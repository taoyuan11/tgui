use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::foundation::binding::{Signal, ViewModelContext};
use crate::foundation::error::TguiError;
use crate::media::RasterRequest;

use super::backend::{
    ffmpeg::FfmpegVideoBackend, BackendSharedState, VideoBackend,
    DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
use super::types::{
    VideoMetrics, VideoPlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot,
};

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
            playback_state: ctx.state(VideoPlaybackState::Idle),
            metrics: ctx.state(VideoMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.state(VideoSize::default()),
            error: ctx.state(None),
            surface: ctx.state(VideoSurfaceSnapshot::default()),
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
        if self.inner.shared.playback_state.get() == VideoPlaybackState::Ended {
            self.replay();
            return;
        }
        self.inner.backend.play();
    }

    pub fn replay(&self) {
        self.seek(Duration::ZERO);
        self.inner.backend.play();
    }

    pub fn pause(&self) {
        self.inner.backend.pause();
    }

    pub fn seek(&self, position: Duration) {
        let mut metrics = self.inner.shared.metrics.get();
        metrics.position = position;
        self.inner.shared.metrics.set(metrics);
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

    pub fn playback_state(&self) -> Signal<VideoPlaybackState> {
        self.inner.shared.playback_state.signal()
    }

    pub fn position(&self) -> Signal<Duration> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.position)
    }

    pub fn duration(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.duration)
    }

    pub fn buffered_position(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.buffered)
    }

    pub fn volume(&self) -> Signal<f32> {
        self.inner.shared.volume.signal()
    }

    pub fn muted(&self) -> Signal<bool> {
        self.inner.shared.muted.signal()
    }

    pub fn video_size(&self) -> Signal<VideoSize> {
        self.inner.shared.video_size.signal()
    }

    pub fn error(&self) -> Signal<Option<String>> {
        self.inner.shared.error.signal()
    }

    pub(crate) fn set_target_raster(&self, raster: Option<RasterRequest>) {
        self.inner.backend.set_target_raster(raster);
    }

    pub(crate) fn surface_metadata(&self) -> VideoSurfaceSnapshot {
        self.inner.shared.surface.get()
    }

    pub(crate) fn current_frame(&self) -> Option<Arc<crate::media::TextureFrame>> {
        self.inner.backend.current_frame()
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
        commands: Vec<&'static str>,
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
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("play");
        }

        fn pause(&self) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("pause");
            commands.pause_count += 1;
        }

        fn seek(&self, position: Duration) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("seek");
            commands.seeks.push(position);
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

        fn set_target_raster(&self, _raster: Option<RasterRequest>) {}

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
            playback_state: ctx.state(VideoPlaybackState::Idle),
            metrics: ctx.state(VideoMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.state(VideoSize::default()),
            error: ctx.state(None),
            surface: ctx.state(VideoSurfaceSnapshot::default()),
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
        assert_eq!(commands.commands, vec!["play", "pause", "seek"]);
        assert_eq!(commands.pause_count, 1);
        assert_eq!(commands.seeks, vec![Duration::from_secs(9)]);
        assert_eq!(commands.volumes, vec![0.25]);
        assert_eq!(commands.muteds, vec![true]);
        assert_eq!(commands.buffer_memory_limits, vec![32 * 1024 * 1024]);
    }

    #[test]
    fn seek_updates_position_signal_immediately() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(30)),
            position: Duration::from_secs(3),
            buffered: Some(Duration::from_secs(12)),
            video_width: 16,
            video_height: 9,
        });
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::from_secs(9));

        assert_eq!(controller.position().get(), Duration::from_secs(9));
        assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
        assert_eq!(
            controller.buffered_position().get(),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            commands.lock().expect("commands lock poisoned").seeks,
            vec![Duration::from_secs(9)]
        );
    }

    #[test]
    fn controller_forwards_url_sources_with_headers_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(
                VideoSource::url("https://example.com/demo.mp4")
                    .with_header("Authorization", "Bearer token")
                    .with_header("Referer", "https://example.com/app"),
            )
            .expect("mock load should succeed");

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            commands.loads,
            vec![VideoSource::Url {
                url: "https://example.com/demo.mp4".to_string(),
                headers: vec![
                    ("Authorization".to_string(), "Bearer token".to_string()),
                    ("Referer".to_string(), "https://example.com/app".to_string()),
                ],
            }]
        );
    }

    #[test]
    fn controller_bindings_reflect_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let frame = Arc::new(TextureFrame::new(8, 4, vec![255; 8 * 4 * 4]));
        *backend.frame.lock().expect("frame lock poisoned") = Some(frame.clone());
        let controller = VideoController::from_parts(shared.clone(), backend);

        shared.playback_state.set(VideoPlaybackState::Paused);
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

        assert_eq!(
            controller.playback_state().get(),
            VideoPlaybackState::Paused
        );
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
                .current_frame()
                .expect("backend frame should be available")
                .size(),
            (8, 4)
        );
    }

    #[test]
    fn play_restarts_from_beginning_after_playback_ended() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Ended);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.play();

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.commands, vec!["seek", "play"]);
        assert_eq!(commands.seeks, vec![Duration::ZERO]);
    }

    #[test]
    fn replay_seeks_to_start_then_plays() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.replay();

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.commands, vec!["seek", "play"]);
        assert_eq!(commands.seeks, vec![Duration::ZERO]);
    }

    // 补充测试：更多边界情况和错误处理

    #[test]
    fn volume_clamps_to_valid_range() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared.clone(), backend);

        // 超出范围的值应被 clamp
        controller.set_volume(1.5);
        controller.set_volume(-0.5);
        controller.set_volume(0.5);

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            recorded.volumes,
            vec![1.0, 0.0, 0.5],
            "volumes should be clamped to [0.0, 1.0]"
        );
    }

    #[test]
    fn multiple_pause_calls_are_idempotent() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.pause();
        controller.pause();
        controller.pause();

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            recorded.pause_count, 3,
            "all pause calls should be forwarded"
        );
    }

    #[test]
    fn seek_then_play_maintains_position() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::from_secs(15));
        controller.play();

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.commands, vec!["seek", "play"]);
        assert_eq!(recorded.seeks, vec![Duration::from_secs(15)]);
    }

    #[test]
    fn buffer_memory_limit_propagates_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        let limits = vec![16 * 1024 * 1024, 64 * 1024 * 1024, 128 * 1024 * 1024];
        for limit in &limits {
            controller.set_buffer_memory_limit_bytes(*limit);
        }

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.buffer_memory_limits, limits);
    }

    #[test]
    fn mute_unmute_sequence() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller.set_muted(true);
        assert!(shared.muted.get(), "should be muted");

        controller.set_muted(false);
        assert!(!shared.muted.get(), "should be unmuted");

        controller.set_muted(true);
        assert!(shared.muted.get(), "should be muted again");

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.muteds, vec![true, false, true]);
    }

    #[test]
    fn load_resets_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Playing);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(60)),
            position: Duration::from_secs(30),
            buffered: Some(Duration::from_secs(45)),
            video_width: 1920,
            video_height: 1080,
        });

        let backend = Arc::new(MockBackend::new());
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller
            .load(VideoSource::File("new.mp4".into()))
            .expect("load should succeed");

        // reset_for_load 设置状态为 Loading（正在加载新源）
        assert_eq!(shared.playback_state.get(), VideoPlaybackState::Loading);
        assert_eq!(shared.metrics.get().position, Duration::ZERO);
    }

    #[test]
    fn play_when_idle_does_not_seek() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Idle);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.play();

        let recorded = commands.lock().expect("commands lock poisoned");
        // Idle 状态下 play() 不应触发 seek，只调用 play
        assert_eq!(recorded.commands, vec!["play"]);
        assert!(recorded.seeks.is_empty());
    }

    #[test]
    fn seek_to_zero_is_valid() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::ZERO);

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.seeks, vec![Duration::ZERO]);
        assert_eq!(controller.position().get(), Duration::ZERO);
    }

    #[test]
    fn volume_and_mute_are_independent() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller.set_volume(0.5);
        controller.set_muted(true);

        assert_eq!(
            shared.volume.get(),
            0.5,
            "volume setting independent of mute"
        );
        assert!(shared.muted.get(), "mute flag should be set");

        controller.set_muted(false);
        assert_eq!(
            shared.volume.get(),
            0.5,
            "unmuting should not change volume"
        );
    }
}
