mod network;
mod output;
#[cfg(test)]
mod tests;

#[cfg(all(feature = "video", test))]
pub(crate) use network::ffmpeg_http_input_options;
pub(crate) use network::{
    open_ffmpeg_input, read_ffmpeg_packet, validate_ffmpeg_headers, PacketRead,
};
pub(crate) use output::{AudioOutput, SharedAudioClock};

#[cfg(feature = "bench-support")]
pub(crate) use network::ffmpeg_http_input_options as bench_ffmpeg_http_input_options;
#[cfg(feature = "bench-support")]
pub(crate) use output::bench_support as audio_output_bench_support;
