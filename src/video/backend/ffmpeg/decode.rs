use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError};

use crate::audio::backend::shared::AudioOutput;
use crate::foundation::error::TguiError;

use super::*;

mod buffering;
mod open;
mod worker;

use self::open::{open_audio_pipeline, resolve_source_url, seek_input_to_start_position};
pub(super) use self::worker::decode_main;

enum DecodeStepOutcome {
    Continue { snapshot_changed: bool },
    Idle { snapshot_changed: bool },
    EofDrained,
}

struct DecodeSession {
    generation: u64,
    _reason: OpenReason,
    start_position: Duration,
    video_frame_duration: Duration,
    input: format::context::Input,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_decoder: ffmpeg::decoder::Video,
    video_codec_id: codec::Id,
    video_decoder_name: String,
    audio_decoder: Option<ffmpeg::decoder::Audio>,
    scaler: Scaler,
    resampler: Option<Resampler>,
    video_time_base: ffmpeg::Rational,
    audio_output: Option<AudioOutput>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    pending_video_packets: VecDeque<QueuedVideoPacket>,
    buffering_profile: BufferingProfile,
    buffer_memory_limit_bytes: u64,
    pending_video_compressed_bytes: u64,
    pending_audio_compressed_bytes: u64,
    last_video_position: Duration,
    eof_sent: bool,
    eof_notified: bool,
    last_snapshot: Option<BufferSnapshot>,
}

impl DecodeSession {
    #[allow(clippy::too_many_arguments)]
    fn open(
        reason: OpenReason,
        generation: u64,
        source: VideoSource,
        start_position: Duration,
        volume: f32,
        muted: bool,
        buffer_memory_limit_bytes: u64,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Result<(Self, StreamOpenedEvent, Duration), TguiError> {
        let source_url = resolve_source_url(&source)?;
        let buffering_profile = buffering_profile_for_source(&source);
        let mut input = open_input(&source, &source_url)?;
        seek_input_to_start_position(&mut input, start_position)?;

        let video_stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| TguiError::Media("video stream not found".to_string()))?;
        let video_stream_index = video_stream.index();
        let video_time_base = video_stream.time_base();
        let opened_video_decoder = open_video_decoder(&video_stream)?;
        let video_decoder = opened_video_decoder.decoder;
        let scaler = Scaler::get(
            video_decoder.format(),
            video_decoder.width(),
            video_decoder.height(),
            Pixel::RGBA,
            video_decoder.width(),
            video_decoder.height(),
            ScalingFlags::BILINEAR,
        )
        .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))?;

        let intrinsic_size =
            IntrinsicSize::from_pixels(video_decoder.width(), video_decoder.height());
        let duration = stream_duration(video_stream.duration(), video_time_base);
        let video_frame_duration =
            stream_frame_duration(&video_stream).unwrap_or(Duration::from_millis(33));

        let audio_stream = input.streams().best(media::Type::Audio);
        let (audio_stream_index, audio_decoder, resampler, audio_output, audio_clock) =
            if let Some(audio_stream) = audio_stream {
                let opened_audio = open_audio_pipeline(&audio_stream, volume, muted)?;
                (
                    Some(opened_audio.stream_index),
                    Some(opened_audio.decoder),
                    Some(opened_audio.resampler),
                    Some(opened_audio.output),
                    Some(opened_audio.clock),
                )
            } else {
                (None, None, None, None, None)
            };

        let mut session = Self {
            generation,
            _reason: reason,
            start_position,
            video_frame_duration,
            input,
            video_stream_index,
            audio_stream_index,
            video_decoder,
            video_codec_id: opened_video_decoder.codec_id,
            video_decoder_name: opened_video_decoder.decoder_name,
            audio_decoder,
            scaler,
            resampler,
            video_time_base,
            audio_output,
            shared_queue,
            playback_clock,
            pending_video_packets: VecDeque::new(),
            buffering_profile,
            buffer_memory_limit_bytes,
            pending_video_compressed_bytes: 0,
            pending_audio_compressed_bytes: 0,
            last_video_position: start_position,
            eof_sent: false,
            eof_notified: false,
            last_snapshot: None,
        };

        session.playback_clock.set_position(start_position);
        let first_frame_position = session.prime_first_frame()?;

        let opened = StreamOpenedEvent {
            generation,
            start_position,
            duration,
            intrinsic_size,
            video_size: VideoSize {
                width: session.video_decoder.width(),
                height: session.video_decoder.height(),
            },
            buffering_profile,
            audio_clock,
        };

        Ok((session, opened, first_frame_position))
    }

    fn prime_first_frame(&mut self) -> Result<Duration, TguiError> {
        loop {
            if self.shared_queue.has_frames(self.generation) {
                return Ok(self
                    .shared_queue
                    .front(self.generation)
                    .map(|frame| frame.position)
                    .unwrap_or(self.start_position));
            }

            let next_packet = {
                let mut packets = self.input.packets();
                packets
                    .next()
                    .map(|(stream, packet)| (stream.index(), packet))
            };
            let Some((stream_index, packet)) = next_packet else {
                self.eof_sent = true;
                self.video_decoder.send_eof().map_err(|error| {
                    TguiError::Media(format!("failed to flush preview decoder: {error}"))
                })?;
                self.fill_ready_video_frames(false)?;
                if let Some(frame) = self.shared_queue.front(self.generation) {
                    return Ok(frame.position);
                }
                break;
            };

            if stream_index != self.video_stream_index {
                continue;
            }

            self.queue_video_packet(packet);
            self.fill_ready_video_frames(false)?;
        }

        Err(TguiError::Media(
            "video source does not contain a decodable frame".to_string(),
        ))
    }

    fn step(&mut self) -> Result<DecodeStepOutcome, TguiError> {
        let mut snapshot_changed = false;

        if self.fill_ready_video_frames(true)? {
            snapshot_changed = true;
        }

        if self.should_throttle_demux() {
            snapshot_changed |= self.update_snapshot_cache();
            return Ok(DecodeStepOutcome::Idle { snapshot_changed });
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets
                .next()
                .map(|(stream, packet)| (stream.index(), packet))
        };

        match next_packet {
            Some((stream_index, packet)) => {
                if stream_index == self.video_stream_index {
                    self.queue_video_packet(packet);
                    snapshot_changed = true;
                    if self.fill_ready_video_frames(true)? {
                        snapshot_changed = true;
                    }
                } else if Some(stream_index) == self.audio_stream_index {
                    if let (Some(audio_decoder), Some(resampler), Some(audio_output)) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_output.as_ref(),
                    ) {
                        audio_decoder.send_packet(&packet).map_err(|error| {
                            TguiError::Media(format!("failed to send audio packet: {error}"))
                        })?;
                        self.pending_audio_compressed_bytes = self
                            .pending_audio_compressed_bytes
                            .saturating_add(packet.size() as u64);
                        receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        snapshot_changed = true;
                    }
                }
            }
            None => {
                if !self.eof_sent {
                    self.eof_sent = true;
                    self.video_decoder.send_eof().map_err(|error| {
                        TguiError::Media(format!("failed to flush video decoder: {error}"))
                    })?;

                    if let (Some(audio_decoder), Some(resampler), Some(audio_output)) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_output.as_ref(),
                    ) {
                        let _ = audio_decoder.send_eof();
                        receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        flush_audio_resampler(
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                    }
                    snapshot_changed = true;
                }

                if self.fill_ready_video_frames(true)? {
                    snapshot_changed = true;
                }

                if self.eof_sent
                    && self.pending_video_packets.is_empty()
                    && !self.shared_queue.has_frames(self.generation)
                    && self.audio_buffered_duration().is_zero()
                {
                    if !self.eof_notified {
                        self.eof_notified = true;
                        self.update_snapshot_cache();
                        return Ok(DecodeStepOutcome::EofDrained);
                    }
                    return Ok(DecodeStepOutcome::Idle {
                        snapshot_changed: false,
                    });
                }
            }
        }

        snapshot_changed |= self.update_snapshot_cache();
        Ok(DecodeStepOutcome::Continue { snapshot_changed })
    }

    fn snapshot(&self) -> BufferSnapshot {
        BufferSnapshot {
            generation: self.generation,
            eof_sent: self.eof_sent,
            total_buffered_memory_bytes: self.total_buffered_memory_bytes(),
            buffering_constrained_by_memory_limit: self.buffering_constrained_by_memory_limit(),
        }
    }

    fn update_snapshot_cache(&mut self) -> bool {
        let snapshot = self.snapshot();
        let changed = self
            .last_snapshot
            .as_ref()
            .map(|previous| {
                previous.eof_sent != snapshot.eof_sent
                    || previous.total_buffered_memory_bytes != snapshot.total_buffered_memory_bytes
                    || previous.buffering_constrained_by_memory_limit
                        != snapshot.buffering_constrained_by_memory_limit
            })
            .unwrap_or(true);
        if changed {
            self.last_snapshot = Some(snapshot);
        }
        changed
    }

    fn set_playing(&mut self, playing: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_playing(playing);
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_volume(volume);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_muted(muted);
        }
    }

    fn set_buffer_memory_limit_bytes(&mut self, bytes: u64) {
        self.buffer_memory_limit_bytes = bytes;
    }
}
