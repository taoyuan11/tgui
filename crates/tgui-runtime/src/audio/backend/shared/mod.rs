mod init;
mod network;
mod output;
mod resample;
mod source;
#[cfg(test)]
mod tests;

pub(crate) use init::ensure_ffmpeg_initialized;
#[cfg(all(feature = "video", test))]
pub(crate) use network::ffmpeg_http_input_options;
pub(crate) use network::{open_ffmpeg_input, validate_ffmpeg_headers};
pub(crate) use output::{normalize_playback_rate, AudioOutput, AudioSampleBatch, SharedAudioClock};
pub(crate) use resample::{
    flush_audio_resampler_with_buffer, receive_audio_frames_with_buffer, ReusableAudioFrame,
};
#[cfg(feature = "video")]
pub(crate) use resample::{
    flush_audio_resampler_with_pending_and_buffer, receive_audio_frames_with_pending_and_buffer,
};
pub(crate) use source::{create_temporary_media_file, media_path_to_url, TemporaryMediaFile};

#[cfg(feature = "bench-support")]
pub(crate) use network::ffmpeg_http_input_options as bench_ffmpeg_http_input_options;
#[cfg(feature = "bench-support")]
pub(crate) use output::bench_support as audio_output_bench_support;
