use super::*;

struct VideoReceiveOutcome {
    decoded_any: bool,
    decoder_drained: bool,
}

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
        self.queued_video_tail_position()
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn pending_video_packet_memory_bytes(&self) -> u64 {
        self.pending_video_packet_bytes
            .saturating_add(self.pending_video_compressed_bytes)
    }

    pub(super) fn ready_video_frame_memory_bytes(&self) -> u64 {
        self.shared_queue.ready_memory_bytes(self.generation)
    }

    pub(super) fn audio_buffered_memory_bytes(&self) -> u64 {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_memory_bytes())
            .unwrap_or(0)
            .saturating_add(self.pending_audio_compressed_bytes)
    }

    pub(super) fn total_buffered_memory_bytes(&self) -> u64 {
        total_buffered_memory_bytes(
            self.pending_video_packet_memory_bytes(),
            self.ready_video_frame_memory_bytes(),
            self.audio_buffered_memory_bytes(),
        )
    }

    pub(super) fn estimated_next_video_frame_memory_bytes(&self) -> u64 {
        let output = self.scaler.output();
        u64::from(output.width)
            .saturating_mul(u64::from(output.height))
            .saturating_mul(4)
    }

    pub(super) fn buffering_constrained_by_memory_limit(&self) -> bool {
        buffering_constrained_by_memory_limit(
            self.total_buffered_memory_bytes(),
            self.buffer_memory_limit_bytes,
            self.estimated_next_video_frame_memory_bytes(),
        )
    }

    pub(super) fn should_throttle_demux(&self) -> bool {
        let audio_buffered = self.audio_buffered_duration();
        let video_buffered = self.ready_video_buffered_duration();
        let has_audio = self.audio_output.is_some();
        let minimum_working_set_ready = self.shared_queue.has_frames(self.generation)
            && (!has_audio || !audio_buffered.is_zero());
        let soft_water_reached = video_buffered >= self.buffering_profile.video_queue_high_water
            && (!has_audio
                || audio_buffered >= self.buffering_profile.audio_queue_high_water);
        should_throttle_demux(
            soft_water_reached
                || (minimum_working_set_ready
                    && self.total_buffered_memory_bytes() >= self.buffer_memory_limit_bytes),
            audio_buffered >= self.buffering_profile.audio_queue_hard_water,
            video_buffered >= self.buffering_profile.video_queue_hard_water,
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
        self.pending_video_packet_bytes = self
            .pending_video_packet_bytes
            .saturating_add(packet.size() as u64);
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
        let ready_count = self.shared_queue.ready_frame_count(self.generation);
        let mut decode_budget = self
            .buffering_profile
            .ready_video_frame_count
            .saturating_sub(ready_count);
        let mut outcome =
            self.receive_ready_video_frames(&mut decode_budget, respect_buffer_memory_limit)?;
        let mut decoded_any = outcome.decoded_any;

        while decode_budget > 0
            && outcome.decoder_drained
            && self.can_admit_video_frame(respect_buffer_memory_limit)
        {
            let Some(queued_packet) = self.pending_video_packets.pop_front() else {
                break;
            };
            let packet_bytes = queued_packet.packet.size() as u64;

            if let Err(error) = self.video_decoder.send_packet(&queued_packet.packet) {
                if is_video_send_would_block(error) {
                    self.pending_video_packets.push_front(queued_packet);
                    outcome = self.receive_ready_video_frames(
                        &mut decode_budget,
                        respect_buffer_memory_limit,
                    )?;
                    decoded_any |= outcome.decoded_any;
                    if outcome.decoder_drained {
                        return Err(self.video_packet_send_error(error));
                    }
                    continue;
                }
                return Err(self.video_packet_send_error(error));
            }
            self.pending_video_compressed_bytes = self
                .pending_video_compressed_bytes
                .saturating_add(packet_bytes);
            self.pending_video_packet_bytes =
                self.pending_video_packet_bytes.saturating_sub(packet_bytes);

            outcome = self
                .receive_ready_video_frames(&mut decode_budget, respect_buffer_memory_limit)?;
            decoded_any |= outcome.decoded_any;
        }

        Ok(decoded_any)
    }

    fn receive_ready_video_frames(
        &mut self,
        decode_budget: &mut usize,
        respect_buffer_memory_limit: bool,
    ) -> Result<VideoReceiveOutcome, TguiError> {
        let mut decoded = VideoFrame::empty();
        let mut decoded_any = false;

        while *decode_budget > 0 && self.can_admit_video_frame(respect_buffer_memory_limit) {
            match self.video_decoder.receive_frame(&mut decoded) {
                Ok(()) => {}
                Err(error) if is_video_receive_drained(error) => {
                    return Ok(VideoReceiveOutcome {
                        decoded_any,
                        decoder_drained: true,
                    })
                }
                Err(error) => return Err(self.video_frame_receive_error(error)),
            }

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
                &mut self.converted_video_frame,
                &decoded,
                self.video_texture_id,
                revision,
            )?);
            let frame = QueuedVideoFrame {
                generation: self.generation,
                position,
                end_position: position.saturating_add(self.video_frame_duration),
                decoded_bytes: texture.pixels().len() as u64,
                texture,
                compressed_bytes: std::mem::take(&mut self.pending_video_compressed_bytes),
            };
            self.last_video_position = position;
            self.shared_queue.push_frames(vec![frame]);
            decoded_any = true;
            *decode_budget = (*decode_budget).saturating_sub(1);
        }

        Ok(VideoReceiveOutcome {
            decoded_any,
            decoder_drained: false,
        })
    }

    fn can_admit_video_frame(&self, respect_buffer_memory_limit: bool) -> bool {
        !respect_buffer_memory_limit
            || !self.buffering_constrained_by_memory_limit()
            || !self.shared_queue.has_frames(self.generation)
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

    pub(super) fn video_frame_receive_error(&self, error: ffmpeg::Error) -> TguiError {
        if self.video_codec_id == codec::Id::AV1 {
            return TguiError::Media(format!(
                "当前 FFmpeg 无法解码这个 AV1 视频。解码器 `{}` 报错: {error}。请安装或构建带 `dav1d` / `aom` 软件 AV1 解码支持的 FFmpeg，或将该视频转码为 H.264/H.265 后再预览。",
                self.video_decoder_name
            ));
        }

        TguiError::Media(format!("failed to receive video frame: {error}"))
    }
}

fn is_video_receive_drained(error: ffmpeg::Error) -> bool {
    matches!(
        error,
        ffmpeg::Error::Eof
            | ffmpeg::Error::Other {
                errno: ffmpeg::error::EAGAIN
            }
    )
}

fn is_video_send_would_block(error: ffmpeg::Error) -> bool {
    matches!(
        error,
        ffmpeg::Error::Other {
            errno: ffmpeg::error::EAGAIN
        }
    )
}
