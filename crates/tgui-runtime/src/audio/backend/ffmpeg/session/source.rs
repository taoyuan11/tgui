use std::time::Duration;

use ffmpeg::format;
use ffmpeg_next as ffmpeg;

use crate::audio::backend::shared::{
    create_temporary_media_file, media_path_to_url, TemporaryMediaFile,
};
use crate::audio::AudioSource;
use crate::foundation::error::TguiError;

use super::super::{LOCAL_AUDIO_QUEUE_HARD_WATER, NETWORK_AUDIO_QUEUE_HARD_WATER};

pub(crate) fn validate_audio_source(source: &AudioSource) -> Result<(), TguiError> {
    match source {
        AudioSource::File(_) => Ok(()),
        AudioSource::Url { headers, .. } => {
            crate::audio::backend::shared::validate_ffmpeg_headers("audio", headers)
        }
        AudioSource::Bytes { bytes, .. } => {
            if bytes.is_empty() {
                Err(TguiError::Media("audio bytes source is empty".to_string()))
            } else {
                Ok(())
            }
        }
    }
}

pub(super) struct OpenedAudioInput {
    pub(super) input: format::context::Input,
    pub(super) resource: Option<TemporaryMediaFile>,
}

pub(super) fn open_audio_input(
    source: &AudioSource,
    start_position: Duration,
) -> Result<OpenedAudioInput, TguiError> {
    let (source_url, headers, resource) = match source {
        AudioSource::File(path) => (media_path_to_url("audio", path)?, None, None),
        AudioSource::Url { url, headers } => (url.clone(), Some(headers.as_slice()), None),
        AudioSource::Bytes { bytes, extension } => {
            let file = create_temporary_media_file("audio", bytes, extension.as_deref())?;
            let source_url = media_path_to_url("audio", file.path())?;
            (source_url, None, Some(file))
        }
    };
    let mut input =
        crate::audio::backend::shared::open_ffmpeg_input("audio", &source_url, headers)?;

    if !start_position.is_zero() {
        let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
        input
            .seek(timestamp, ..timestamp)
            .map_err(|error| TguiError::Media(format!("failed to seek audio source: {error}")))?;
    }

    Ok(OpenedAudioInput { input, resource })
}

pub(super) fn queue_hard_water(source: &AudioSource) -> Duration {
    match source {
        AudioSource::File(_) | AudioSource::Bytes { .. } => LOCAL_AUDIO_QUEUE_HARD_WATER,
        AudioSource::Url { .. } => NETWORK_AUDIO_QUEUE_HARD_WATER,
    }
}

pub(super) fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

fn pts_to_duration(timestamp: Option<i64>, time_base: ffmpeg::Rational) -> Option<Duration> {
    let timestamp = timestamp?;
    let numerator = time_base.numerator() as f64;
    let denominator = time_base.denominator() as f64;
    if denominator <= 0.0 {
        return None;
    }
    let seconds = timestamp as f64 * numerator / denominator;
    Some(Duration::from_secs_f64(seconds.max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_sources_use_local_queue_limits() {
        assert_eq!(
            queue_hard_water(&AudioSource::bytes(vec![1, 2, 3])),
            LOCAL_AUDIO_QUEUE_HARD_WATER
        );
    }

    #[test]
    fn empty_bytes_source_is_rejected_before_open() {
        assert!(matches!(
            validate_audio_source(&AudioSource::bytes(Vec::<u8>::new())),
            Err(TguiError::Media(message)) if message.contains("bytes source is empty")
        ));
    }
}
