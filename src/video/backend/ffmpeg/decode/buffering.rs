use super::*;

impl DecodeSession {
    pub(super) fn playback_position(&self) -> Duration {
        self.playback_clock.position()
    }

    pub(super) fn audio_buffered_duration(&self) -> Duration {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn ready_video_buffered_duration(&self) -> Duration {
        let baseline = self.playback_position();
        self.shared_queue
            .tail_end_position(self.generation)
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn pending_video_packet_memory_bytes(&self) -> u64 {
        self.pending_video_packets
            .iter()
            .map(|packet| packet.packet.size() as u64)
            .sum()
    }

    pub(super) fn ready_video_frame_memory_bytes(&self) -> u64 {
        self.shared_queue.ready_memory_bytes(self.generation)
    }

    pub(super) fn audio_buffered_memory_bytes(&self) -> u64 {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_memory_bytes())
            .unwrap_or(0)
    }

    pub(super) fn total_buffered_memory_bytes(&self) -> u64 {
        total_buffered_memory_bytes(
            self.pending_video_packet_memory_bytes(),
            self.ready_video_frame_memory_bytes(),
            self.audio_buffered_memory_bytes(),
        )
    }

    pub(super) fn estimated_next_video_frame_memory_bytes(&self) -> u64 {
        self.shared_queue
            .head_frame_memory_bytes(self.generation)
            .or_else(|| {
                let frame_bytes = self
                    .shared_queue
                    .state
                    .lock()
                    .expect("video queue lock poisoned")
                    .frames
                    .iter()
                    .filter(|frame| frame.generation == self.generation)
                    .map(|frame| frame.compressed_bytes)
                    .collect::<Vec<_>>();
                average_non_zero_bytes(&frame_bytes)
            })
            .or_else(|| {
                self.pending_video_packets
                    .front()
                    .map(|packet| packet.packet.size() as u64)
            })
            .unwrap_or(self.pending_video_compressed_bytes)
    }

    pub(super) fn buffering_constrained_by_memory_limit(&self) -> bool {
        buffering_constrained_by_memory_limit(
            self.total_buffered_memory_bytes(),
            self.buffer_memory_limit_bytes,
            self.estimated_next_video_frame_memory_bytes(),
        )
    }

    pub(super) fn should_throttle_demux(&self) -> bool {
        should_throttle_demux(
            self.total_buffered_memory_bytes() >= self.buffer_memory_limit_bytes,
            self.audio_buffered_duration() >= self.buffering_profile.audio_queue_hard_water,
            self.ready_video_buffered_duration() >= self.buffering_profile.video_queue_hard_water,
            self.pending_video_packets.len() >= self.buffering_profile.video_max_packet_count,
        )
    }

    pub(super) fn queued_video_tail_position(&self) -> Option<Duration> {
        self.pending_video_packets
            .back()
            .map(|packet| packet.end_position)
            .or_else(|| self.shared_queue.tail_end_position(self.generation))
            .or(Some(self.last_video_position))
    }

    pub(super) fn queue_video_packet(&mut self, packet: ffmpeg::Packet) {
        let position = packet
            .pts()
            .or_else(|| packet.dts())
            .and_then(|timestamp| pts_to_duration(Some(timestamp), self.video_time_base))
            .unwrap_or_else(|| {
                self.queued_video_tail_position()
                    .unwrap_or(self.start_position)
            });
        let duration = packet_duration(packet.duration(), self.video_time_base)
            .unwrap_or(self.video_frame_duration);
        self.pending_video_packets.push_back(QueuedVideoPacket {
            packet,
            end_position: position.saturating_add(duration),
        });
    }

    pub(super) fn fill_ready_video_frames(
        &mut self,
        respect_buffer_memory_limit: bool,
    ) -> Result<bool, TguiError> {
        let mut decoded_any = false;
        let mut decode_budget = self.buffering_profile.ready_video_frame_count;

        while decode_budget > 0
            && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
        {
            let Some(queued_packet) = self.pending_video_packets.pop_front() else {
                break;
            };

            self.video_decoder
                .send_packet(&queued_packet.packet)
                .map_err(|error| self.video_packet_send_error(error))?;
            self.pending_video_compressed_bytes = self
                .pending_video_compressed_bytes
                .saturating_add(queued_packet.packet.size() as u64);

            let mut decoded = VideoFrame::empty();
            let mut newly_decoded = Vec::new();

            while decode_budget > 0
                && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
                && self.video_decoder.receive_frame(&mut decoded).is_ok()
            {
                let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                    .unwrap_or_else(|| {
                        self.queued_video_tail_position()
                            .unwrap_or(self.start_position)
                    });

                if self.should_drop_video_preroll_frame(position) {
                    continue;
                }

                let revision = self.next_video_texture_revision();
                let texture = Arc::new(video_frame_to_texture(
                    &mut self.scaler,
                    &decoded,
                    self.video_texture_id,
                    revision,
                )?);
                let frame = QueuedVideoFrame {
                    generation: self.generation,
                    position,
                    end_position: position.saturating_add(self.video_frame_duration),
                    texture,
                    compressed_bytes: 0,
                };
                self.last_video_position = position;
                newly_decoded.push(frame);
                decode_budget = decode_budget.saturating_sub(1);
            }

            if !newly_decoded.is_empty() {
                let compressed_bytes = std::mem::take(&mut self.pending_video_compressed_bytes);
                distribute_video_compressed_bytes(&mut newly_decoded, compressed_bytes);
                self.shared_queue.push_frames(newly_decoded);
                decoded_any = true;
            }
        }

        if decode_budget > 0
            && self.pending_video_packets.is_empty()
            && self.eof_sent
            && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
        {
            let mut decoded = VideoFrame::empty();
            let mut flushed_frames = Vec::new();
            while decode_budget > 0
                && self.video_decoder.receive_frame(&mut decoded).is_ok()
                && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
            {
                let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                    .unwrap_or_else(|| {
                        self.queued_video_tail_position()
                            .unwrap_or(self.start_position)
                    });
                if self.should_drop_video_preroll_frame(position) {
                    continue;
                }

                let revision = self.next_video_texture_revision();
                let texture = Arc::new(video_frame_to_texture(
                    &mut self.scaler,
                    &decoded,
                    self.video_texture_id,
                    revision,
                )?);
                flushed_frames.push(QueuedVideoFrame {
                    generation: self.generation,
                    position,
                    end_position: position.saturating_add(self.video_frame_duration),
                    texture,
                    compressed_bytes: 0,
                });
                self.last_video_position = position;
                decode_budget = decode_budget.saturating_sub(1);
            }

            if !flushed_frames.is_empty() {
                let compressed_bytes = std::mem::take(&mut self.pending_video_compressed_bytes);
                distribute_video_compressed_bytes(&mut flushed_frames, compressed_bytes);
                self.shared_queue.push_frames(flushed_frames);
                decoded_any = true;
            }
        }

        Ok(decoded_any)
    }

    pub(super) fn should_drop_video_preroll_frame(&self, position: Duration) -> bool {
        !self.start_position.is_zero()
            && position.saturating_add(VIDEO_SEEK_PREROLL_TOLERANCE) < self.start_position
    }

    pub(super) fn video_packet_send_error(&self, error: ffmpeg::Error) -> TguiError {
        if self.video_codec_id == codec::Id::AV1
            && matches!(
                error,
                ffmpeg::Error::Other {
                    errno: ffmpeg::error::ENOSYS
                }
            )
        {
            return TguiError::Media(format!(
                "AV1 is not supported by the linked FFmpeg build. The current decoder `{}` cannot decode this file on this platform. Rebuild/install FFmpeg with the `dav1d` or `aom` feature enabled in vcpkg.",
                self.video_decoder_name
            ));
        }

        TguiError::Media(format!("failed to send video packet: {error}"))
    }
}

fn average_non_zero_bytes(bytes: &[u64]) -> Option<u64> {
    let (sum, count) = bytes
        .iter()
        .copied()
        .filter(|bytes| *bytes > 0)
        .fold((0u64, 0u64), |(sum, count), bytes| {
            (sum.saturating_add(bytes), count + 1)
        });
    (count > 0).then(|| sum / count)
}

#[cfg(test)]
mod tests {
    use super::average_non_zero_bytes;

    #[test]
    fn average_non_zero_bytes_returns_none_for_empty_or_zero_only_input() {
        assert_eq!(average_non_zero_bytes(&[]), None);
        assert_eq!(average_non_zero_bytes(&[0, 0, 0]), None);
    }

    #[test]
    fn average_non_zero_bytes_ignores_zero_entries() {
        assert_eq!(average_non_zero_bytes(&[0, 10, 20]), Some(15));
    }
}
