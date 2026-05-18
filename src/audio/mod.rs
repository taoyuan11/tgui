mod controller;
mod types;

pub(crate) mod backend;

pub use crate::ui::widget::Audio;
pub use controller::AudioController;
pub(crate) use types::AudioSnapshot;
pub use types::{AudioMetrics, AudioSource, PlaybackState};

#[cfg(feature = "bench-support")]
pub mod bench_support {
    //! Benchmark hooks. Only enabled with the `bench-support` feature.

    use crate::audio::backend::shared::{
        audio_output_bench_support as raw, bench_ffmpeg_http_input_options,
    };
    use crate::foundation::error::TguiError;

    pub use crate::audio::backend::shared::audio_output_bench_support::BenchAudioOutput;

    pub fn make_output(channels: u16, volume: f32, muted: bool, playing: bool) -> BenchAudioOutput {
        raw::make_output(channels, volume, muted, playing)
    }

    pub fn enqueue_chunk(output: &BenchAudioOutput, samples: Vec<f32>, compressed_bytes: u64) {
        raw::enqueue_chunk(output, samples, compressed_bytes);
    }

    pub fn write_f32(buffer: &mut [f32], output: &BenchAudioOutput) {
        raw::write_f32(buffer, output);
    }

    pub fn write_i16(buffer: &mut [i16], output: &BenchAudioOutput) {
        raw::write_i16(buffer, output);
    }

    pub fn played_frames(output: &BenchAudioOutput) -> u64 {
        raw::played_frames(output)
    }

    pub fn build_http_options(headers: &[(String, String)]) -> Result<(), TguiError> {
        bench_ffmpeg_http_input_options("audio", headers).map(|_| ())
    }
}
