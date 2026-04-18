use std::sync::Arc;

use crate::foundation::binding::Observable;
use crate::media::TextureFrame;
use crate::TguiError;

use super::types::{PlaybackState, VideoMetrics, VideoSize, VideoSource, VideoSurfaceSnapshot};

pub(crate) mod ffmpeg;

#[derive(Clone)]
pub(crate) struct BackendSharedState {
    pub playback_state: Observable<PlaybackState>,
    pub metrics: Observable<VideoMetrics>,
    pub volume: Observable<f32>,
    pub muted: Observable<bool>,
    pub video_size: Observable<VideoSize>,
    pub error: Observable<Option<String>>,
    pub surface: Observable<VideoSurfaceSnapshot>,
}

impl BackendSharedState {
    pub fn reset_for_load(&self) {
        self.playback_state.set(PlaybackState::Loading);
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
            .set(PlaybackState::Error(message.clone()));
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
    fn current_frame(&self) -> Option<Arc<TextureFrame>>;
    fn shutdown(&self);

    fn on_surface_lost(&self) {}
    fn on_surface_restored(&self) {}
    fn on_app_background(&self) {}
    fn on_app_foreground(&self) {}
}
