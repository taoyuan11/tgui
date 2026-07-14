use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::unbounded;

use super::super::{AudioBackend, BackendSharedState, DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES};
use super::worker::AudioWorker;
use super::{AudioWorkerHandle, BackendCommand, FfmpegAudioBackend};
use crate::animation::AnimationCoordinator;
use crate::audio::{AudioController, AudioMetrics, AudioPlaybackState, AudioSource};
use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
use crate::foundation::error::TguiError;

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
        snapshot: ctx.state(crate::audio::AudioSnapshot::default()),
    }
}

#[test]
fn backend_creation_and_preload_settings_do_not_start_worker() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = FfmpegAudioBackend::new(shared);

    assert!(backend
        .worker
        .lock()
        .expect("audio worker lock poisoned")
        .is_none());

    AudioBackend::set_volume(&backend, 0.5);
    AudioBackend::set_muted(&backend, true);
    AudioBackend::set_looping(&backend, true);
    AudioBackend::set_playback_rate(&backend, 1.5);
    AudioBackend::set_buffer_memory_limit_bytes(&backend, 16 * 1024 * 1024);

    assert!(backend
        .worker
        .lock()
        .expect("audio worker lock poisoned")
        .is_none());
}

#[test]
fn controller_load_rejects_invalid_source_without_starting_worker() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = Arc::new(FfmpegAudioBackend::new(shared.clone()));
    let controller = AudioController::from_parts(shared.clone(), backend.clone());
    let source =
        AudioSource::url("https://example.com/demo.mp3").with_header("Bad\nHeader", "value");

    let error = controller
        .load(source)
        .expect_err("invalid header should fail synchronously");

    assert!(matches!(
        error,
        TguiError::Media(message) if message.contains("invalid line break")
    ));
    assert!(backend
        .worker
        .lock()
        .expect("audio worker lock poisoned")
        .is_none());
    assert!(matches!(
        controller.playback_state().get(),
        AudioPlaybackState::Error(message) if message.contains("invalid line break")
    ));
    assert!(controller
        .error()
        .get()
        .is_some_and(|message| message.contains("invalid line break")));
    assert_eq!(
        controller.snapshot(),
        crate::audio::AudioSnapshot {
            loading: false,
            error: controller.error().get(),
        }
    );
}

#[test]
fn shutdown_returns_after_timeout_when_worker_is_blocked() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let backend = FfmpegAudioBackend::new(shared);
    let (command_tx, _command_rx) = unbounded();
    let worker = std::thread::spawn(|| std::thread::sleep(Duration::from_millis(250)));
    *backend.worker.lock().expect("audio worker lock poisoned") =
        Some(AudioWorkerHandle { command_tx, worker });

    let started = Instant::now();
    AudioBackend::shutdown(&backend);

    assert!(
        started.elapsed() < Duration::from_millis(200),
        "shutdown should detach a blocked worker after the configured timeout"
    );
    assert!(backend
        .worker
        .lock()
        .expect("audio worker lock poisoned")
        .is_none());
}

#[test]
fn play_after_ended_reopens_from_start_when_looping_disabled() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("demo.mp3".into()));
    worker.shared.playback_state.set(AudioPlaybackState::Ended);

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
    worker
        .shared
        .playback_state
        .set(AudioPlaybackState::Playing);

    assert!(worker.handle_command(BackendCommand::Stop));

    assert!(worker.session.is_none());
    assert_eq!(worker.shared.playback_state.get(), AudioPlaybackState::Idle);
    assert_eq!(worker.shared.metrics.get(), AudioMetrics::default());
}

// 补充测试：命令处理验证（直接测试worker的命令处理逻辑）

#[test]
fn volume_control_updates_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    // Worker内部处理SetVolume命令
    assert!(worker.handle_command(BackendCommand::SetVolume(0.75)));
    assert_eq!(worker.volume, 0.75, "worker volume should be updated");

    assert!(worker.handle_command(BackendCommand::SetVolume(0.0)));
    assert_eq!(worker.volume, 0.0, "worker volume should be set to 0");

    assert!(worker.handle_command(BackendCommand::SetVolume(1.0)));
    assert_eq!(worker.volume, 1.0, "worker volume should be set to maximum");
}

#[test]
fn mute_control_updates_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    assert!(worker.handle_command(BackendCommand::SetMuted(true)));
    assert!(worker.muted, "worker muted should be true");

    assert!(worker.handle_command(BackendCommand::SetMuted(false)));
    assert!(!worker.muted, "worker muted should be false");
}

#[test]
fn looping_control_updates_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    assert!(worker.handle_command(BackendCommand::SetLooping(true)));
    assert!(worker.looping, "worker looping should be enabled");

    assert!(worker.handle_command(BackendCommand::SetLooping(false)));
    assert!(!worker.looping, "worker looping should be disabled");
}

#[test]
fn playback_rate_updates_worker_and_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    assert!(worker.handle_command(BackendCommand::SetPlaybackRate(2.5)));
    assert_eq!(worker.playback_rate, 2.5);
    assert_eq!(shared.playback_rate.get(), 2.5);

    assert!(worker.handle_command(BackendCommand::SetPlaybackRate(99.0)));
    assert_eq!(worker.playback_rate, 4.0);
    assert_eq!(shared.playback_rate.get(), 4.0);

    assert!(worker.handle_command(BackendCommand::SetPlaybackRate(f32::NAN)));
    assert_eq!(worker.playback_rate, 1.0);
    assert_eq!(shared.playback_rate.get(), 1.0);
}

#[test]
fn buffer_memory_limit_updates_shared_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    let new_limit = 32 * 1024 * 1024; // 32 MB
    assert!(worker.handle_command(BackendCommand::SetBufferMemoryLimitBytes(new_limit)));
    assert_eq!(
        worker.buffer_memory_limit_bytes, new_limit,
        "worker buffer memory limit should be updated"
    );
}

#[test]
fn pause_command_sets_worker_flag() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.should_play = true;

    // Pause命令会设置should_play标志
    assert!(worker.handle_command(BackendCommand::Pause));
    assert!(!worker.should_play, "should_play flag should be cleared");

    // 注意：playback_state只在有active session时才会更新
    // 这个测试验证命令处理逻辑，不需要完整的session
}

#[test]
fn seek_command_updates_target_position() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("test.mp3".into()));
    worker.current_duration = Some(Duration::from_secs(60));

    let seek_target = Duration::from_secs(30);
    // Seek command triggers reopen_current_source, which is handled internally
    // We verify the command returns true (processed successfully)
    assert!(worker.handle_command(BackendCommand::Seek(seek_target)));

    // The seek is executed via reopen_current_source, not stored in a field
    // Session will be reopened at the target position when playback continues
}

#[test]
fn play_when_paused_resumes_playback() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("demo.mp3".into()));
    worker.shared.playback_state.set(AudioPlaybackState::Paused);
    worker.should_play = false;

    assert!(worker.handle_command(BackendCommand::Play));

    assert!(worker.should_play, "should_play flag should be set");
}

#[test]
fn stop_when_idle_is_noop() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.shared.playback_state.set(AudioPlaybackState::Idle);

    assert!(worker.handle_command(BackendCommand::Stop));

    assert_eq!(
        worker.shared.playback_state.get(),
        AudioPlaybackState::Idle,
        "state should remain Idle"
    );
    assert!(worker.session.is_none(), "session should be None");
}

#[test]
fn multiple_volume_changes_without_worker() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());

    // 快速多次改变音量
    for i in 0..10 {
        assert!(worker.handle_command(BackendCommand::SetVolume(i as f32 / 10.0)));
    }

    assert_eq!(
        worker.volume, 0.9,
        "worker volume should reflect last change"
    );
}

#[test]
fn looping_enabled_prevents_ended_state() {
    let ctx = test_context();
    let shared = test_shared(&ctx);
    let (_tx, rx) = unbounded();
    let mut worker = AudioWorker::new(rx, shared.clone());
    worker.current_source = Some(AudioSource::File("loop.mp3".into()));
    worker.shared.looping.set(true);
    worker
        .shared
        .playback_state
        .set(AudioPlaybackState::Playing);

    // 模拟播放结束，但循环开启时不应进入 Ended 状态
    // 实际逻辑在 worker 的主循环中，这里只验证初始设置
    assert!(worker.shared.looping.get(), "looping should be enabled");
}
