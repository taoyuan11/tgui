mod controller;
mod types;

pub(crate) mod backend;

pub use crate::ui::widget::VideoSurface;
pub use controller::VideoController;
pub(crate) use types::VideoSurfaceSnapshot;
pub use types::{VideoMetrics, VideoPlaybackState, VideoSize, VideoSource};

#[cfg(feature = "bench-support")]
pub mod bench_support {
    //! Benchmark hooks. Only enabled with the `bench-support` feature.

    pub use crate::video::backend::ffmpeg::bench_support::{
        bench_buffering_constrained_by_memory_limit, bench_distribute_video_compressed_bytes,
        bench_pts_to_duration, bench_should_buffer_for_rebuffer, bench_should_buffer_video,
        bench_should_throttle_demux, bench_total_buffered_memory_bytes,
        bench_video_buffer_target_satisfied, BenchPlaybackClock, BenchVideoQueue,
    };
}
