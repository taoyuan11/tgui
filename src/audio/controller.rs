use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::foundation::binding::{Signal, ViewModelContext};
use crate::foundation::error::TguiError;

use super::backend::{
    ffmpeg::FfmpegAudioBackend, AudioBackend, BackendSharedState,
    DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES,
};
use super::types::{AudioMetrics, AudioSnapshot, AudioSource, PlaybackState};

#[derive(Clone)]
pub struct AudioController {
    inner: Arc<AudioControllerInner>,
}

struct AudioControllerInner {
    shared: BackendSharedState,
    backend: Arc<dyn AudioBackend>,
}

impl AudioController {
    pub fn new(ctx: &ViewModelContext) -> Self {
        let shared = BackendSharedState {
            playback_state: ctx.state(PlaybackState::Idle),
            metrics: ctx.state(AudioMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES),
            error: ctx.state(None),
            snapshot: ctx.state(AudioSnapshot::default()),
        };
        let backend: Arc<dyn AudioBackend> = Arc::new(FfmpegAudioBackend::new(shared.clone()));
        Self::from_parts(shared, backend)
    }

    pub(crate) fn from_parts(shared: BackendSharedState, backend: Arc<dyn AudioBackend>) -> Self {
        Self {
            inner: Arc::new(AudioControllerInner { shared, backend }),
        }
    }

    pub fn load(&self, source: AudioSource) -> Result<(), TguiError> {
        self.inner.shared.reset_for_load();
        self.inner.backend.load(source)
    }

    pub fn play(&self) {
        if self.inner.shared.playback_state.get() == PlaybackState::Ended {
            self.inner.backend.seek(Duration::ZERO);
        }
        self.inner.backend.play();
    }

    pub fn pause(&self) {
        self.inner.backend.pause();
    }

    pub fn stop(&self) {
        self.inner.backend.stop();
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

    pub fn set_looping(&self, looping: bool) {
        self.inner.shared.looping.set(looping);
        self.inner.backend.set_looping(looping);
    }

    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.inner.shared.buffer_memory_limit_bytes.set(bytes);
        self.inner.backend.set_buffer_memory_limit_bytes(bytes);
    }

    pub fn playback_state(&self) -> Signal<PlaybackState> {
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

    pub fn looping(&self) -> Signal<bool> {
        self.inner.shared.looping.signal()
    }

    pub fn error(&self) -> Signal<Option<String>> {
        self.inner.shared.error.signal()
    }

    pub(crate) fn snapshot(&self) -> AudioSnapshot {
        self.inner.shared.snapshot.get()
    }
}

impl PartialEq for AudioController {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for AudioController {}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};

    use super::super::backend::{AudioBackend, BackendSharedState};
    use super::*;

    #[derive(Default)]
    struct RecordedCommands {
        loads: Vec<AudioSource>,
        commands: Vec<&'static str>,
        seeks: Vec<Duration>,
        volumes: Vec<f32>,
        muteds: Vec<bool>,
        loopings: Vec<bool>,
        buffer_memory_limits: Vec<u64>,
    }

    struct MockBackend {
        commands: Arc<Mutex<RecordedCommands>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                commands: Arc::new(Mutex::new(RecordedCommands::default())),
            }
        }
    }

    impl AudioBackend for MockBackend {
        fn load(&self, source: AudioSource) -> Result<(), TguiError> {
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
                .commands
                .push("play");
        }

        fn pause(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .commands
                .push("pause");
        }

        fn stop(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .commands
                .push("stop");
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

        fn set_looping(&self, looping: bool) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .loopings
                .push(looping);
        }

        fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .buffer_memory_limits
                .push(bytes);
        }

        fn shutdown(&self) {}
    }

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    fn test_shared(ctx: &ViewModelContext) -> BackendSharedState {
        BackendSharedState {
            playback_state: ctx.state(PlaybackState::Idle),
            metrics: ctx.state(AudioMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES),
            error: ctx.state(None),
            snapshot: ctx.state(AudioSnapshot::default()),
        }
    }

    #[test]
    fn controller_forwards_commands_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = AudioController::from_parts(shared, backend);

        controller
            .load(AudioSource::File("demo.mp3".into()))
            .expect("mock load should succeed");
        controller.play();
        controller.pause();
        controller.stop();
        controller.seek(Duration::from_secs(9));
        controller.set_volume(0.25);
        controller.set_muted(true);
        controller.set_looping(true);
        controller.set_buffer_memory_limit_bytes(32 * 1024 * 1024);

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.loads, vec![AudioSource::File("demo.mp3".into())]);
        assert_eq!(commands.commands, vec!["play", "pause", "stop", "seek"]);
        assert_eq!(commands.seeks, vec![Duration::from_secs(9)]);
        assert_eq!(commands.volumes, vec![0.25]);
        assert_eq!(commands.muteds, vec![true]);
        assert_eq!(commands.loopings, vec![true]);
        assert_eq!(commands.buffer_memory_limits, vec![32 * 1024 * 1024]);
    }

    #[test]
    fn controller_bindings_reflect_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let controller = AudioController::from_parts(shared.clone(), backend);

        shared.playback_state.set(PlaybackState::Paused);
        shared.metrics.set(AudioMetrics {
            duration: Some(Duration::from_secs(30)),
            position: Duration::from_secs(12),
            buffered: Some(Duration::from_secs(16)),
        });
        shared.error.set(Some("boom".to_string()));
        shared.looping.set(true);

        assert_eq!(controller.playback_state().get(), PlaybackState::Paused);
        assert_eq!(controller.position().get(), Duration::from_secs(12));
        assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
        assert_eq!(
            controller.buffered_position().get(),
            Some(Duration::from_secs(16))
        );
        assert_eq!(controller.error().get(), Some("boom".to_string()));
        assert!(controller.looping().get());
    }

    #[test]
    fn stop_state_reset_clears_progress_and_error() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(PlaybackState::Playing);
        shared.metrics.set(AudioMetrics {
            duration: Some(Duration::from_secs(10)),
            position: Duration::from_secs(4),
            buffered: Some(Duration::from_secs(6)),
        });
        shared.error.set(Some("boom".to_string()));
        shared.snapshot.set(AudioSnapshot {
            loading: false,
            error: Some("boom".to_string()),
        });

        shared.reset_for_stop();

        assert_eq!(shared.playback_state.get(), PlaybackState::Idle);
        assert_eq!(shared.metrics.get(), AudioMetrics::default());
        assert_eq!(shared.error.get(), None);
        assert_eq!(shared.snapshot.get(), AudioSnapshot::default());
    }
}
