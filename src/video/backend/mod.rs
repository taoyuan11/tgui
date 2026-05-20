use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::foundation::binding::State;
use crate::foundation::error::TguiError;
use crate::media::{RasterRequest, TextureFrame};

use super::types::{VideoMetrics, VideoPlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot};

pub(crate) mod ffmpeg;

pub(crate) const DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct BackendSharedState {
    pub playback_state: State<VideoPlaybackState>,
    pub metrics: State<VideoMetrics>,
    pub volume: State<f32>,
    pub muted: State<bool>,
    pub metrics_observed: Arc<AtomicBool>,
    pub buffer_memory_limit_bytes: State<u64>,
    pub video_size: State<VideoSize>,
    pub error: State<Option<String>>,
    pub surface: State<VideoSurfaceSnapshot>,
}

impl BackendSharedState {
    pub fn enable_metrics(&self) {
        self.metrics_observed.store(true, Ordering::SeqCst);
    }

    pub fn metrics_enabled(&self) -> bool {
        self.metrics_observed.load(Ordering::SeqCst)
    }

    pub fn publish_frame(&self) {
        self.surface.invalidation().request_redraw();
    }

    pub fn reset_for_load(&self) {
        self.playback_state.set(VideoPlaybackState::Loading);
        self.metrics.set(VideoMetrics::default());
        self.video_size.set(VideoSize::default());
        self.error.set(None);
        self.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::ZERO,
            texture: None,
            loading: true,
            error: None,
        });
    }

    pub fn set_error(&self, message: String) {
        self.playback_state
            .set(VideoPlaybackState::Error(message.clone()));
        self.error.set(Some(message.clone()));
        self.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.video_size.get().intrinsic_size(),
            texture: None,
            loading: false,
            error: Some(message),
        });
    }
}

#[allow(dead_code)]
pub(crate) trait VideoBackend: Send + Sync {
    fn load(&self, source: VideoSource) -> Result<(), TguiError>;
    fn play(&self);
    fn pause(&self);
    fn seek(&self, position: std::time::Duration);
    fn set_volume(&self, volume: f32);
    fn set_muted(&self, muted: bool);
    fn set_buffer_memory_limit_bytes(&self, bytes: u64);
    fn set_target_raster(&self, raster: Option<RasterRequest>);
    fn current_frame(&self) -> Option<Arc<TextureFrame>>;
    fn shutdown(&self);

    fn on_surface_lost(&self) {}
    fn on_surface_restored(&self) {}
    fn on_app_background(&self) {}
    fn on_app_foreground(&self) {}
}
