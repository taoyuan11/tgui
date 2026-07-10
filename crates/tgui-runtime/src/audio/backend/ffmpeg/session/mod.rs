use std::time::Duration;

use ffmpeg::codec;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg_next as ffmpeg;

use crate::audio::AudioSource;
use crate::foundation::error::TguiError;

use super::super::shared::{
    read_ffmpeg_packet, AudioOutput, PacketRead, SharedAudioClock,
};

mod decode;
mod source;

use decode::{flush_audio_resampler, receive_audio_frames};
pub(crate) use source::validate_audio_source;
use source::{open_audio_input, queue_hard_water, seek_audio_input, stream_duration};

pub(super) enum SessionStep {
    Continue,
    Idle,
    EofDrained,
}

pub(super) struct AudioSession {
    input: ffmpeg::format::context::Input,
    start_position: Duration,
    duration: Option<Duration>,
    audio_stream_index: usize,
    audio_decoder: ffmpeg::decoder::Audio,
    resampler: Resampler,
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
        playing: bool,
    ) -> Result<Self, TguiError> {
        let input = open_audio_input(&source, start_position)?;

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
        let audio_clock = audio_output.clock_handle();
        let resampler = create_audio_resampler(&audio_decoder, &audio_output)?;

        let mut session = Self {
            input,
            start_position,
            duration,
            audio_stream_index,
            audio_decoder,
            resampler,
            audio_output,
            audio_clock,
            eof_sent: false,
            eof_drained: false,
            queue_hard_water: queue_hard_water(&source),
        };
        session.audio_output.set_playing(false);
        session.prime_initial_audio()?;
        session.audio_output.set_playing(playing);
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

    pub(super) fn seek(
        &mut self,
        position: Duration,
        playing: bool,
    ) -> Result<(), TguiError> {
        self.audio_output.set_playing(false);
        seek_audio_input(&mut self.input, position)?;
        self.audio_decoder.flush();
        let replacement_resampler = create_audio_resampler(&self.audio_decoder, &self.audio_output)?;
        self.audio_output.reset();

        self.start_position = position;
        self.resampler = replacement_resampler;
        self.eof_sent = false;
        self.eof_drained = false;
        self.prime_initial_audio()?;
        self.audio_output.set_playing(playing);
        Ok(())
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

        let buffered_duration = self.audio_clock.buffered_duration();
        if buffered_duration >= self.queue_hard_water
            || (!buffered_duration.is_zero()
                && self.audio_clock.buffered_memory_bytes() >= buffer_memory_limit_bytes)
        {
            return Ok(SessionStep::Idle);
        }

        match read_ffmpeg_packet("audio", &mut self.input)? {
            PacketRead::Packet(packet) => {
                let stream_index = packet.stream();
                if stream_index == self.audio_stream_index {
                    self.audio_decoder.send_packet(&packet).map_err(|error| {
                        TguiError::Media(format!("failed to send audio packet: {error}"))
                    })?;
                    receive_audio_frames(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        packet.size() as u64,
                    )?;
                }
                Ok(SessionStep::Continue)
            }
            PacketRead::Retry => Ok(SessionStep::Idle),
            PacketRead::Eof => {
                if !self.eof_sent {
                    self.eof_sent = true;
                    let _ = self.audio_decoder.send_eof();
                    receive_audio_frames(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        0,
                    )?;
                    flush_audio_resampler(&mut self.resampler, &self.audio_output)?;
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

fn create_audio_resampler(
    decoder: &ffmpeg::decoder::Audio,
    output: &AudioOutput,
) -> Result<Resampler, TguiError> {
    Resampler::get(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
        ffmpeg::ChannelLayout::default(output.channels().into()),
        output.sample_rate(),
    )
    .map_err(|error| TguiError::Media(format!("failed to create audio resampler: {error}")))
}
