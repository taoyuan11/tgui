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

pub(super) struct OpenedSubtitlePipeline {
    pub(super) stream_index: usize,
    pub(super) decoder: ffmpeg::decoder::Subtitle,
    pub(super) time_base: ffmpeg::Rational,
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

pub(super) fn collect_audio_tracks(input: &format::context::Input) -> Vec<VideoAudioTrack> {
    input
        .streams()
        .filter_map(|stream| {
            let parameters = stream.parameters();
            (parameters.medium() == media::Type::Audio).then(|| {
                let metadata = stream.metadata();
                let (channels, sample_rate) = audio_stream_shape(parameters);
                VideoAudioTrack {
                    stream_index: stream.index(),
                    title: metadata.get("title").map(ToString::to_string),
                    language: metadata.get("language").map(ToString::to_string),
                    channels,
                    sample_rate,
                }
            })
        })
        .collect()
}

pub(super) fn select_audio_stream<'a>(
    input: &'a format::context::Input,
    selection: VideoAudioTrackSelection,
) -> Result<Option<format::stream::Stream<'a>>, TguiError> {
    match selection {
        VideoAudioTrackSelection::Auto => Ok(input.streams().best(media::Type::Audio)),
        VideoAudioTrackSelection::Disabled => Ok(None),
        VideoAudioTrackSelection::Stream(index) => input
            .streams()
            .find(|stream| {
                stream.index() == index && stream.parameters().medium() == media::Type::Audio
            })
            .map(Some)
            .ok_or_else(|| TguiError::Media(format!("audio track stream {index} not found"))),
    }
}

pub(super) fn collect_subtitle_tracks(input: &format::context::Input) -> Vec<VideoSubtitleTrack> {
    input
        .streams()
        .filter_map(|stream| {
            let parameters = stream.parameters();
            (parameters.medium() == media::Type::Subtitle).then(|| {
                let metadata = stream.metadata();
                VideoSubtitleTrack {
                    stream_index: stream.index(),
                    title: metadata.get("title").map(ToString::to_string),
                    language: metadata.get("language").map(ToString::to_string),
                    codec: Some(parameters.id().name().to_string()),
                }
            })
        })
        .collect()
}

pub(super) fn validate_subtitle_selection(
    input: &format::context::Input,
    selection: VideoSubtitleTrackSelection,
) -> Result<(), TguiError> {
    match selection {
        VideoSubtitleTrackSelection::Disabled => Ok(()),
        VideoSubtitleTrackSelection::Stream(index) => input
            .streams()
            .find(|stream| {
                stream.index() == index && stream.parameters().medium() == media::Type::Subtitle
            })
            .map(|_| ())
            .ok_or_else(|| TguiError::Media(format!("subtitle track stream {index} not found"))),
    }
}

pub(super) fn select_subtitle_stream<'a>(
    input: &'a format::context::Input,
    selection: VideoSubtitleTrackSelection,
) -> Result<Option<format::stream::Stream<'a>>, TguiError> {
    match selection {
        VideoSubtitleTrackSelection::Disabled => Ok(None),
        VideoSubtitleTrackSelection::Stream(index) => input
            .streams()
            .find(|stream| {
                stream.index() == index && stream.parameters().medium() == media::Type::Subtitle
            })
            .map(Some)
            .ok_or_else(|| TguiError::Media(format!("subtitle track stream {index} not found"))),
    }
}

fn audio_stream_shape(parameters: codec::Parameters) -> (u16, u32) {
    codec::context::Context::from_parameters(parameters)
        .ok()
        .and_then(|context| context.decoder().audio().ok())
        .map(|decoder| (decoder.channels(), decoder.rate()))
        .unwrap_or((0, 0))
}

pub(super) fn open_audio_pipeline(
    audio_stream: &format::stream::Stream<'_>,
    volume: f32,
    muted: bool,
    playback_rate: f32,
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
    output.set_playback_rate(playback_rate);
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

pub(super) fn open_subtitle_pipeline(
    subtitle_stream: &format::stream::Stream<'_>,
) -> Result<OpenedSubtitlePipeline, TguiError> {
    let stream_index = subtitle_stream.index();
    let subtitle_context =
        codec::context::Context::from_parameters(subtitle_stream.parameters())
            .map_err(|error| TguiError::Media(format!("failed to open subtitle codec: {error}")))?;
    let mut decoder = subtitle_context
        .decoder()
        .subtitle()
        .map_err(|error| TguiError::Media(format!("failed to create subtitle decoder: {error}")))?;
    decoder
        .set_parameters(subtitle_stream.parameters())
        .map_err(|error| {
            TguiError::Media(format!("failed to configure subtitle decoder: {error}"))
        })?;

    Ok(OpenedSubtitlePipeline {
        stream_index,
        decoder,
        time_base: subtitle_stream.time_base(),
    })
}
