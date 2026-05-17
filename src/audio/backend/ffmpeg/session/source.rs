use std::time::Duration;

use ffmpeg::format;
use ffmpeg_next as ffmpeg;

use crate::audio::AudioSource;
use crate::foundation::error::TguiError;

use super::super::{LOCAL_AUDIO_QUEUE_HARD_WATER, NETWORK_AUDIO_QUEUE_HARD_WATER};

pub(crate) fn validate_audio_source(source: &AudioSource) -> Result<(), TguiError> {
    match source {
        AudioSource::File(_) => Ok(()),
        AudioSource::Url { headers, .. } => {
            crate::audio::backend::shared::validate_ffmpeg_headers("audio", headers)
        }
    }
}

pub(super) fn open_audio_input(
    source: &AudioSource,
    start_position: Duration,
) -> Result<format::context::Input, TguiError> {
    let source_url = match source {
        AudioSource::File(path) => path
            .to_str()
            .ok_or_else(|| TguiError::Media("audio path is not valid UTF-8".to_string()))?
            .to_string(),
        AudioSource::Url { url, .. } => url.clone(),
    };
    let headers = match source {
        AudioSource::File(_) => None,
        AudioSource::Url { headers, .. } => Some(headers.as_slice()),
    };
    let mut input =
        crate::audio::backend::shared::open_ffmpeg_input("audio", &source_url, headers)?;

    if !start_position.is_zero() {
        let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
        input
            .seek(timestamp, ..timestamp)
            .map_err(|error| TguiError::Media(format!("failed to seek audio source: {error}")))?;
    }

    Ok(input)
}

pub(super) fn queue_hard_water(source: &AudioSource) -> Duration {
    match source {
        AudioSource::File(_) => LOCAL_AUDIO_QUEUE_HARD_WATER,
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
