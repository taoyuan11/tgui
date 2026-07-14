use std::time::Duration;

use ffmpeg::codec;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg_next as ffmpeg;

use crate::audio::AudioSource;
use crate::foundation::error::TguiError;

use super::super::shared::{
    flush_audio_resampler_with_buffer, receive_audio_frames_with_buffer, AudioOutput,
    ReusableAudioFrame, SharedAudioClock, TemporaryMediaFile,
};

mod source;

pub(crate) use source::validate_audio_source;
use source::{open_audio_input, queue_hard_water, stream_duration};

pub(super) enum SessionStep {
    Continue,
    Idle,
    EofDrained,
}

pub(super) struct AudioSession {
    input: ffmpeg::format::context::Input,
    _input_resource: Option<TemporaryMediaFile>,
    start_position: Duration,
    duration: Option<Duration>,
    audio_stream_index: usize,
    audio_decoder: ffmpeg::decoder::Audio,
    resampler: Resampler,
    resample_frame: ReusableAudioFrame,
    audio_output: AudioOutput,
    audio_clock: SharedAudioClock,
    eof_sent: bool,
    eof_drained: bool,
    queue_hard_water: Duration,
}

impl AudioSession {
    pub(super) fn open(
        source: AudioSource,
        start_position: Duration,
        volume: f32,
        muted: bool,
        playback_rate: f32,
        playing: bool,
    ) -> Result<Self, TguiError> {
        let opened_input = open_audio_input(&source, start_position)?;
        let input = opened_input.input;
        let input_resource = opened_input.resource;

        let audio_stream = input
            .streams()
            .best(media::Type::Audio)
            .ok_or_else(|| TguiError::Media("audio stream not found".to_string()))?;
        let audio_stream_index = audio_stream.index();
        let duration = stream_duration(audio_stream.duration(), audio_stream.time_base());
        let audio_context = codec::context::Context::from_parameters(audio_stream.parameters())
            .map_err(|error| TguiError::Media(format!("failed to open audio codec: {error}")))?;
        let mut audio_decoder = audio_context.decoder().audio().map_err(|error| {
            TguiError::Media(format!("failed to create audio decoder: {error}"))
        })?;
        audio_decoder
            .set_parameters(audio_stream.parameters())
            .map_err(|error| {
                TguiError::Media(format!("failed to configure audio decoder: {error}"))
            })?;
        if audio_decoder.channel_layout().is_empty() {
            audio_decoder.set_channel_layout(ffmpeg::ChannelLayout::default(
                audio_decoder.channels().into(),
            ));
        }

        let audio_output = AudioOutput::new(volume, muted, "tgui-audio")
            .map_err(|error| TguiError::Media(format!("failed to create audio output: {error}")))?;
        audio_output.set_playback_rate(playback_rate);
        let audio_clock = audio_output.clock_handle();
        let resampler = Resampler::get(
            audio_decoder.format(),
            audio_decoder.channel_layout(),
            audio_decoder.rate(),
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::default(audio_output.channels().into()),
            audio_output.sample_rate(),
        )
        .map_err(|error| TguiError::Media(format!("failed to create audio resampler: {error}")))?;

        let mut session = Self {
            input,
            _input_resource: input_resource,
            start_position,
            duration,
            audio_stream_index,
            audio_decoder,
            resampler,
            resample_frame: ReusableAudioFrame::new(),
            audio_output,
            audio_clock,
            eof_sent: false,
            eof_drained: false,
            queue_hard_water: queue_hard_water(&source),
        };
        session.audio_output.set_playing(playing);
        session.prime_initial_audio()?;
        Ok(session)
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub(super) fn position(&self) -> Duration {
        self.start_position
            .saturating_add(self.audio_clock.position())
    }

    pub(super) fn buffered_position(&self) -> Duration {
        self.position()
            .saturating_add(self.audio_clock.buffered_duration())
    }

    pub(super) fn set_playing(&self, playing: bool) {
        self.audio_output.set_playing(playing);
    }

    pub(super) fn set_volume(&self, volume: f32) {
        self.audio_output.set_volume(volume);
    }

    pub(super) fn set_muted(&self, muted: bool) {
        self.audio_output.set_muted(muted);
    }

    pub(super) fn set_playback_rate(&self, rate: f32) {
        self.audio_output.set_playback_rate(rate);
    }

    fn prime_initial_audio(&mut self) -> Result<(), TguiError> {
        // 首次打开后先预灌一小段音频，避免一按播放就因为缓冲为零而立刻饿死。
        while !self.eof_sent && self.audio_clock.buffered_duration() < Duration::from_millis(200) {
            match self.step(u64::MAX)? {
                SessionStep::Continue => {}
                SessionStep::Idle | SessionStep::EofDrained => break,
            }
        }
        Ok(())
    }

    pub(super) fn step(
        &mut self,
        buffer_memory_limit_bytes: u64,
    ) -> Result<SessionStep, TguiError> {
        if self.eof_sent && self.audio_clock.buffered_duration().is_zero() {
            if self.eof_drained {
                return Ok(SessionStep::Idle);
            }
            self.eof_drained = true;
            return Ok(SessionStep::EofDrained);
        }

        if self.audio_clock.buffered_duration() >= self.queue_hard_water
            || self.audio_clock.buffered_memory_bytes() >= buffer_memory_limit_bytes
        {
            return Ok(SessionStep::Idle);
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets
                .next()
                .map(|(stream, packet)| (stream.index(), packet))
        };

        match next_packet {
            Some((stream_index, packet)) => {
                if stream_index == self.audio_stream_index {
                    self.audio_decoder.send_packet(&packet).map_err(|error| {
                        TguiError::Media(format!("failed to send audio packet: {error}"))
                    })?;
                    receive_audio_frames_with_buffer(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        &mut self.resample_frame,
                        packet.size() as u64,
                    )?;
                }
                Ok(SessionStep::Continue)
            }
            None => {
                if !self.eof_sent {
                    self.eof_sent = true;
                    let _ = self.audio_decoder.send_eof();
                    receive_audio_frames_with_buffer(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        &mut self.resample_frame,
                        0,
                    )?;
                    flush_audio_resampler_with_buffer(
                        &mut self.resampler,
                        &self.audio_output,
                        &mut self.resample_frame,
                    )?;
                }
                Ok(if self.audio_clock.buffered_duration().is_zero() {
                    SessionStep::EofDrained
                } else {
                    SessionStep::Idle
                })
            }
        }
    }
}
