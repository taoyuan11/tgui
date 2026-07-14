//! Video playback controllers, state types, and widgets.
//!
//! This module is available with the `video` feature. It provides a
//! [`VideoController`] for loading and controlling media, [`VideoSurface`] for
//! rendering frames, and [`Video`] for a ready-made player with controls.

mod controller;
mod types;

pub(crate) mod backend;

pub use crate::ui::widget::{Video, VideoStyle, VideoSurface};
pub use controller::VideoController;
pub use types::{
    VideoAudioTrack, VideoAudioTrackSelection, VideoMetrics, VideoPlaybackState, VideoSize,
    VideoSource, VideoSubtitleBitmapCue, VideoSubtitleCue, VideoSubtitleTrack,
    VideoSubtitleTrackSelection,
};
pub(crate) use types::{
    VideoSubtitleCuePlacement, VideoSubtitleCueStyle, VideoSubtitleHorizontalAlign,
    VideoSubtitleVerticalAlign, VideoSurfaceSnapshot,
};

#[cfg(feature = "bench-support")]
pub mod bench_support {
    //! Benchmark hooks. Only enabled with the `bench-support` feature.

    pub use crate::rendering::renderer::{
        renderer_texture_diagnostics, reset_renderer_texture_diagnostics,
        RendererTextureDiagnostics,
    };
    pub use crate::video::backend::ffmpeg::bench_support::{
        bench_buffering_constrained_by_memory_limit, bench_distribute_video_compressed_bytes,
        bench_pts_to_duration, bench_should_buffer_for_rebuffer, bench_should_buffer_video,
        bench_should_throttle_demux, bench_total_buffered_memory_bytes,
        bench_video_buffer_target_satisfied, BenchConvertedVideoFrame, BenchPlaybackClock,
        BenchVideoFrameConverter, BenchVideoFrameKind, BenchVideoQueue,
    };
}
