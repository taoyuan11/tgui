use std::sync::Arc;
use std::time::Duration;

use crate::foundation::binding::{Binding, ViewModelContext};
use crate::TguiError;

use super::backend::{
    ffmpeg::FfmpegVideoBackend, BackendSharedState, VideoBackend,
    DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
use super::types::{PlaybackState, VideoMetrics, VideoSize, VideoSource, VideoSurfaceSnapshot};

#[derive(Clone)]
pub struct VideoController {
    inner: Arc<VideoControllerInner>,
}

struct VideoControllerInner {
    shared: BackendSharedState,
    backend: Arc<dyn VideoBackend>,
}

impl VideoController {
    pub fn new(ctx: &ViewModelContext) -> Self {
        let shared = BackendSharedState {
            playback_state: ctx.observable(PlaybackState::Idle),
            metrics: ctx.observable(VideoMetrics::default()),
            volume: ctx.observable(1.0),
            muted: ctx.observable(false),
            buffer_memory_limit_bytes: ctx.observable(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.observable(VideoSize::default()),
            error: ctx.observable(None),
            surface: ctx.observable(VideoSurfaceSnapshot::default()),
        };
        let backend: Arc<dyn VideoBackend> = Arc::new(FfmpegVideoBackend::new(shared.clone()));
        Self::from_parts(shared, backend)
    }

    pub(crate) fn from_parts(shared: BackendSharedState, backend: Arc<dyn VideoBackend>) -> Self {
        Self {
            inner: Arc::new(VideoControllerInner { shared, backend }),
        }
    }

    pub fn load(&self, source: VideoSource) -> Result<(), TguiError> {
        self.inner.shared.reset_for_load();
        self.inner.backend.load(source)
    }

    pub fn play(&self) {
        self.inner.backend.play();
    }

    pub fn pause(&self) {
        self.inner.backend.pause();
    }

    pub fn seek(&self, position: Duration) {
        self.inner.backend.seek(position);
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.inner.shared.volume.set(volume);
        self.inner.backend.set_volume(volume);
    }

    pub fn set_muted(&self, muted: bool) {
        self.inner.shared.muted.set(muted);
        self.inner.backend.set_muted(muted);
    }

    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.inner.shared.buffer_memory_limit_bytes.set(bytes);
        self.inner.backend.set_buffer_memory_limit_bytes(bytes);
    }

    pub fn playback_state(&self) -> Binding<PlaybackState> {
        self.inner.shared.playback_state.binding()
    }

    pub fn position(&self) -> Binding<Duration> {
        self.inner
            .shared
            .metrics
            .binding()
            .map(|metrics| metrics.position)
    }

    pub fn duration(&self) -> Binding<Option<Duration>> {
        self.inner
            .shared
            .metrics
            .binding()
            .map(|metrics| metrics.duration)
    }

    pub fn buffered_position(&self) -> Binding<Option<Duration>> {
        self.inner
            .shared
            .metrics
            .binding()
            .map(|metrics| metrics.buffered)
    }

    pub fn volume(&self) -> Binding<f32> {
        self.inner.shared.volume.binding()
    }

    pub fn muted(&self) -> Binding<bool> {
        self.inner.shared.muted.binding()
    }

    pub fn video_size(&self) -> Binding<VideoSize> {
        self.inner.shared.video_size.binding()
    }

    pub fn error(&self) -> Binding<Option<String>> {
        self.inner.shared.error.binding()
    }

    pub(crate) fn surface_snapshot(&self) -> VideoSurfaceSnapshot {
        let mut snapshot = self.inner.shared.surface.get();
        if snapshot.texture.is_none() {
            snapshot.texture = self.inner.backend.current_frame();
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::media::{IntrinsicSize, TextureFrame};

    use super::super::backend::{BackendSharedState, VideoBackend};
    use super::*;

    #[derive(Default)]
    struct RecordedCommands {
        loads: Vec<VideoSource>,
        play_count: usize,
        pause_count: usize,
        seeks: Vec<Duration>,
        volumes: Vec<f32>,
        muteds: Vec<bool>,
        buffer_memory_limits: Vec<u64>,
    }

    struct MockBackend {
        commands: Arc<Mutex<RecordedCommands>>,
        frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                commands: Arc::new(Mutex::new(RecordedCommands::default())),
                frame: Arc::new(Mutex::new(None)),
            }
        }
    }

    impl VideoBackend for MockBackend {
        fn load(&self, source: VideoSource) -> Result<(), TguiError> {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .loads
                .push(source);
            Ok(())
        }

        fn play(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .play_count += 1;
        }

        fn pause(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .pause_count += 1;
        }

        fn seek(&self, position: Duration) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .seeks
                .push(position);
        }

        fn set_volume(&self, volume: f32) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .volumes
                .push(volume);
        }

        fn set_muted(&self, muted: bool) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .muteds
                .push(muted);
        }

        fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .buffer_memory_limits
                .push(bytes);
        }

        fn current_frame(&self) -> Option<Arc<TextureFrame>> {
            self.frame.lock().expect("frame lock poisoned").clone()
        }

        fn shutdown(&self) {}
    }

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    fn test_shared(ctx: &ViewModelContext) -> BackendSharedState {
        BackendSharedState {
            playback_state: ctx.observable(PlaybackState::Idle),
            metrics: ctx.observable(VideoMetrics::default()),
            volume: ctx.observable(1.0),
            muted: ctx.observable(false),
            buffer_memory_limit_bytes: ctx.observable(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.observable(VideoSize::default()),
            error: ctx.observable(None),
            surface: ctx.observable(VideoSurfaceSnapshot::default()),
        }
    }

    #[test]
    fn controller_forwards_commands_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(VideoSource::File("demo.mp4".into()))
            .expect("mock load should succeed");
        controller.play();
        controller.pause();
        controller.seek(Duration::from_secs(9));
        controller.set_volume(0.25);
        controller.set_muted(true);
        controller.set_buffer_memory_limit_bytes(32 * 1024 * 1024);

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.loads, vec![VideoSource::File("demo.mp4".into())]);
        assert_eq!(commands.play_count, 1);
        assert_eq!(commands.pause_count, 1);
        assert_eq!(commands.seeks, vec![Duration::from_secs(9)]);
        assert_eq!(commands.volumes, vec![0.25]);
        assert_eq!(commands.muteds, vec![true]);
        assert_eq!(commands.buffer_memory_limits, vec![32 * 1024 * 1024]);
    }

    #[test]
    fn controller_bindings_reflect_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let frame = Arc::new(TextureFrame::new(8, 4, vec![255; 8 * 4 * 4]));
        *backend.frame.lock().expect("frame lock poisoned") = Some(frame.clone());
        let controller = VideoController::from_parts(shared.clone(), backend);

        shared.playback_state.set(PlaybackState::Paused);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(30)),
            position: Duration::from_secs(12),
            buffered: Some(Duration::from_secs(16)),
            video_width: 8,
            video_height: 4,
        });
        shared.video_size.set(VideoSize {
            width: 8,
            height: 4,
        });
        shared.error.set(Some("boom".to_string()));
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: IntrinsicSize::from_pixels(8, 4),
            texture: None,
            loading: false,
            error: None,
        });

        assert_eq!(controller.playback_state().get(), PlaybackState::Paused);
        assert_eq!(controller.position().get(), Duration::from_secs(12));
        assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
        assert_eq!(
            controller.buffered_position().get(),
            Some(Duration::from_secs(16))
        );
        assert_eq!(controller.video_size().get().width, 8);
        assert_eq!(controller.error().get(), Some("boom".to_string()));
        assert_eq!(
            controller
                .surface_snapshot()
                .texture
                .expect("backend frame should backfill snapshot")
                .size(),
            (8, 4)
        );
    }

    #[test]
    fn controller_defaults_buffer_memory_limit_to_100_mib() {
        let ctx = test_context();
        let controller = VideoController::new(&ctx);

        assert_eq!(
            controller.inner.shared.buffer_memory_limit_bytes.get(),
            DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES
        );
    }
}
