use std::time::Duration;

use crate::audio::backend::shared::AudioOutput;
use crate::foundation::error::TguiError;

use super::*;

pub(super) struct OpenedAudioPipeline {
    pub(super) stream_index: usize,
    pub(super) decoder: ffmpeg::decoder::Audio,
    pub(super) resampler: Resampler,
    pub(super) output: AudioOutput,
    pub(super) clock: SharedAudioClock,
}

pub(super) fn resolve_source_url(source: &VideoSource) -> Result<String, TguiError> {
    match source {
        VideoSource::File(path) => path
            .to_str()
            .ok_or_else(|| TguiError::Media("video path is not valid UTF-8".to_string()))
            .map(ToString::to_string),
        VideoSource::Url { url, .. } => Ok(url.clone()),
    }
}

pub(super) fn seek_input_to_start_position(
    input: &mut format::context::Input,
    start_position: Duration,
) -> Result<(), TguiError> {
    if start_position.is_zero() {
        return Ok(());
    }

    let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
    input
        .seek(timestamp, ..timestamp)
        .map_err(|error| TguiError::Media(format!("failed to seek video source: {error}")))
}

pub(super) fn open_audio_pipeline(
    audio_stream: &format::stream::Stream<'_>,
    volume: f32,
    muted: bool,
) -> Result<OpenedAudioPipeline, TguiError> {
    let stream_index = audio_stream.index();
    let audio_context = codec::context::Context::from_parameters(audio_stream.parameters())
        .map_err(|error| TguiError::Media(format!("failed to open audio codec: {error}")))?;
    let mut decoder = audio_context
        .decoder()
        .audio()
        .map_err(|error| TguiError::Media(format!("failed to create audio decoder: {error}")))?;
    decoder
        .set_parameters(audio_stream.parameters())
        .map_err(|error| TguiError::Media(format!("failed to configure audio decoder: {error}")))?;
    if decoder.channel_layout().is_empty() {
        decoder.set_channel_layout(ffmpeg::ChannelLayout::default(decoder.channels().into()));
    }

    let output = AudioOutput::new(volume, muted, "tgui-video")
        .map_err(|error| TguiError::Media(format!("failed to create audio output: {error}")))?;
    let clock = output.clock_handle();
    let resampler = Resampler::get(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
        ffmpeg::ChannelLayout::default(output.channels().into()),
        output.sample_rate(),
    )
    .map_err(|error| TguiError::Media(format!("failed to create audio resampler: {error}")))?;

    Ok(OpenedAudioPipeline {
        stream_index,
        decoder,
        resampler,
        output,
        clock,
    })
}
