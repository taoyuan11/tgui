use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::unbounded;

use super::super::{BackendSharedState, DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES};
use super::worker::AudioWorker;
use super::BackendCommand;
use crate::animation::AnimationCoordinator;
use crate::audio::{AudioMetrics, AudioSource, PlaybackState};
use crate::foundation::binding::{InvalidationSignal, ViewModelContext};

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
        snapshot: ctx.state(crate::audio::AudioSnapshot::default()),
    }
}

#[test]
fn play_after_ended_reopens_from_start_when_looping_disabled() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("demo.mp3".into()));
    worker.shared.playback_state.set(PlaybackState::Ended);

    assert!(worker.handle_command(BackendCommand::Play));

    assert!(worker.should_play);
}

#[test]
fn stop_clears_session_and_resets_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("demo.mp3".into()));
    worker.current_duration = Some(Duration::from_secs(30));
    worker.should_play = true;
    worker.shared.playback_state.set(PlaybackState::Playing);

    assert!(worker.handle_command(BackendCommand::Stop));

    assert!(worker.session.is_none());
    assert_eq!(worker.shared.playback_state.get(), PlaybackState::Idle);
    assert_eq!(worker.shared.metrics.get(), AudioMetrics::default());
}
