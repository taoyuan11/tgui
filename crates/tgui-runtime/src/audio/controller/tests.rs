use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::animation::AnimationCoordinator;
use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
use crate::media::{MediaPlaybackSource, MediaSource};

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
    playback_rates: Vec<f32>,
    buffer_memory_limits: Vec<u64>,
}

struct MockBackend {
    commands: Arc<Mutex<RecordedCommands>>,
    load_error: Option<&'static str>,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(RecordedCommands::default())),
            load_error: None,
        }
    }

    fn failing_load(message: &'static str) -> Self {
        Self {
            commands: Arc::new(Mutex::new(RecordedCommands::default())),
            load_error: Some(message),
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
        if let Some(message) = self.load_error {
            return Err(TguiError::Media(message.to_string()));
        }
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

    fn set_playback_rate(&self, rate: f32) {
        self.commands
            .lock()
            .expect("commands lock poisoned")
            .playback_rates
            .push(rate);
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
        playback_state: ctx.state(AudioPlaybackState::Idle),
        metrics: ctx.state(AudioMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
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
    controller.set_playback_rate(2.0);
    controller.set_buffer_memory_limit_bytes(32 * 1024 * 1024);

    let commands = commands.lock().expect("commands lock poisoned");
    assert_eq!(commands.loads, vec![AudioSource::File("demo.mp3".into())]);
    assert_eq!(commands.commands, vec!["play", "pause", "stop", "seek"]);
    assert_eq!(commands.seeks, vec![Duration::from_secs(9)]);
    assert_eq!(commands.volumes, vec![0.25]);
    assert_eq!(commands.muteds, vec![true]);
    assert_eq!(commands.loopings, vec![true]);
    assert_eq!(commands.playback_rates, vec![2.0]);
    assert_eq!(commands.buffer_memory_limits, vec![32 * 1024 * 1024]);
}

#[test]
fn controller_load_accepts_media_source() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(MockBackend::new());
    let commands = backend.commands.clone();
    let controller = AudioController::from_parts(shared, backend);

    controller
        .load(MediaSource::url("https://example.com/demo.mp3"))
        .expect("mock load should succeed");

    let commands = commands.lock().expect("commands lock poisoned");
    assert_eq!(
        commands.loads,
        vec![AudioSource::url("https://example.com/demo.mp3")]
    );
}

#[test]
fn controller_load_accepts_media_playback_source() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(MockBackend::new());
    let commands = backend.commands.clone();
    let controller = AudioController::from_parts(shared, backend);

    controller
        .load(
            MediaPlaybackSource::url("https://example.com/demo.mp3").with_header("X-Test", "value"),
        )
        .expect("mock load should succeed");

    let commands = commands.lock().expect("commands lock poisoned");
    assert_eq!(
        commands.loads,
        vec![AudioSource::url("https://example.com/demo.mp3").with_header("X-Test", "value")]
    );
}

#[test]
fn playback_rate_is_clamped_and_exposed_as_signal() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(MockBackend::new());
    let commands = backend.commands.clone();
    let controller = AudioController::from_parts(shared, backend);

    controller.set_playback_rate(8.0);
    controller.set_playback_rate(0.0);
    controller.set_playback_rate(f32::NAN);

    assert_eq!(controller.playback_rate().get(), 1.0);
    assert_eq!(
        commands
            .lock()
            .expect("commands lock poisoned")
            .playback_rates,
        vec![4.0, 0.25, 1.0]
    );
}

#[test]
fn load_failure_sets_error_state_and_snapshot() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(MockBackend::failing_load("invalid audio header"));
    let controller = AudioController::from_parts(shared.clone(), backend);

    let error = controller
        .load(AudioSource::url("https://example.com/demo.mp3"))
        .expect_err("mock load should fail");

    assert_eq!(error.to_string(), "invalid audio header");
    assert_eq!(
        controller.playback_state().get(),
        AudioPlaybackState::Error("invalid audio header".to_string())
    );
    assert_eq!(
        controller.error().get(),
        Some("invalid audio header".to_string())
    );
    assert_eq!(
        controller.snapshot(),
        AudioSnapshot {
            loading: false,
            error: Some("invalid audio header".to_string()),
        }
    );
}

#[test]
fn controller_bindings_reflect_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(MockBackend::new());
    let controller = AudioController::from_parts(shared.clone(), backend);

    shared.playback_state.set(AudioPlaybackState::Paused);
    shared.metrics.set(AudioMetrics {
        duration: Some(Duration::from_secs(30)),
        position: Duration::from_secs(12),
        buffered: Some(Duration::from_secs(16)),
    });
    shared.error.set(Some("boom".to_string()));
    shared.looping.set(true);
    shared.playback_rate.set(1.5);

    assert_eq!(
        controller.playback_state().get(),
        AudioPlaybackState::Paused
    );
    assert_eq!(controller.position().get(), Duration::from_secs(12));
    assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
    assert_eq!(
        controller.buffered_position().get(),
        Some(Duration::from_secs(16))
    );
    assert_eq!(controller.error().get(), Some("boom".to_string()));
    assert!(controller.looping().get());
    assert_eq!(controller.playback_rate().get(), 1.5);
}

#[test]
fn stop_state_reset_clears_progress_and_error() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    shared.playback_state.set(AudioPlaybackState::Playing);
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

    assert_eq!(shared.playback_state.get(), AudioPlaybackState::Idle);
    assert_eq!(shared.metrics.get(), AudioMetrics::default());
    assert_eq!(shared.error.get(), None);
    assert_eq!(shared.snapshot.get(), AudioSnapshot::default());
}
