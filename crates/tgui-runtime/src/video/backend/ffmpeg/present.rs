use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, Receiver, Sender};

use super::*;

mod playback;

pub(super) fn present_main(
    backend_rx: Receiver<BackendCommand>,
    decode_tx: Sender<DecodeCommand>,
    event_rx: Receiver<DecodeEvent>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
) {
    let mut worker = PresentWorker::new(
        backend_rx,
        decode_tx,
        event_rx,
        shared,
        latest_frame,
        shared_queue,
        playback_clock,
    );
    worker.run();
}

struct PresentWorker {
    backend_rx: Receiver<BackendCommand>,
    decode_tx: Sender<DecodeCommand>,
    event_rx: Receiver<DecodeEvent>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    current_source: Option<VideoSource>,
    current_generation: u64,
    current_duration: Option<Duration>,
    current_intrinsic_size: IntrinsicSize,
    current_video_size: VideoSize,
    current_start_position: Duration,
    current_buffering_profile: BufferingProfile,
    current_audio_clock: Option<SharedAudioClock>,
    last_presented_position: Duration,
    software_paused_position: Duration,
    software_play_started_at: Option<Instant>,
    should_play: bool,
    decode_playing: bool,
    playback_ended: bool,
    buffer_snapshot: BufferSnapshot,
    pending_open_reason: Option<OpenReason>,
    stream_opened: bool,
    startup_pending: bool,
}

impl PresentWorker {
    fn new(
        backend_rx: Receiver<BackendCommand>,
        decode_tx: Sender<DecodeCommand>,
        event_rx: Receiver<DecodeEvent>,
        shared: BackendSharedState,
        latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Self {
        Self {
            backend_rx,
            decode_tx,
            event_rx,
            shared,
            latest_frame,
            shared_queue,
            playback_clock,
            current_source: None,
            current_generation: 0,
            current_duration: None,
            current_intrinsic_size: IntrinsicSize::ZERO,
            current_video_size: VideoSize::default(),
            current_start_position: Duration::ZERO,
            current_buffering_profile: LOCAL_BUFFERING_PROFILE,
            current_audio_clock: None,
            last_presented_position: Duration::ZERO,
            software_paused_position: Duration::ZERO,
            software_play_started_at: None,
            should_play: false,
            decode_playing: false,
            playback_ended: false,
            buffer_snapshot: BufferSnapshot::default(),
            pending_open_reason: None,
            stream_opened: false,
            startup_pending: false,
        }
    }

    fn run(&mut self) {
        loop {
            self.present_due_frames();
            self.sync_metrics();
            self.evaluate_playback_state();

            let wait = self.next_wait_duration();
            let timeout = after(wait);

            select! {
                recv(self.backend_rx) -> message => {
                    match message {
                        Ok(command) => {
                            if !self.handle_backend_command(command) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                recv(self.event_rx) -> message => {
                    match message {
                        Ok(event) => self.handle_decode_event(event),
                        Err(_) => break,
                    }
                }
                recv(timeout) -> _ => {}
            }
        }

        let _ = self.decode_tx.send(DecodeCommand::Shutdown);
    }

    fn handle_backend_command(&mut self, command: BackendCommand) -> bool {
        match command {
            BackendCommand::Load(source) => {
                self.set_decode_playing(false);
                self.current_source = Some(source.clone());
                self.current_generation = self.current_generation.saturating_add(1);
                self.pending_open_reason = Some(OpenReason::Load);
                self.stream_opened = false;
                self.startup_pending = true;
                self.current_start_position = Duration::ZERO;
                self.current_duration = None;
                self.current_audio_clock = None;
                self.last_presented_position = Duration::ZERO;
                self.software_paused_position = Duration::ZERO;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.buffer_snapshot = BufferSnapshot {
                    generation: self.current_generation,
                    ..BufferSnapshot::default()
                };
                self.playback_clock.set_position(Duration::ZERO);
                self.shared_queue
                    .replace_generation(self.current_generation);
                clear_latest_frame(&self.latest_frame);
                self.shared.reset_for_load();
                let _ = self.decode_tx.send(DecodeCommand::Load {
                    generation: self.current_generation,
                    source,
                });
            }
            BackendCommand::Play => {
                if self.playback_ended {
                    self.shared.playback_state.set(VideoPlaybackState::Ended);
                    return true;
                }
                self.should_play = true;
                self.evaluate_playback_state();
            }
            BackendCommand::Pause => {
                self.should_play = false;
                let position = self.playback_position();
                self.pause_software_clock(position);
                self.set_decode_playing(false);
                if self.stream_opened {
                    if self.shared.metrics_enabled() {
                        let mut metrics = self.shared.metrics.get();
                        metrics.position = position;
                        self.shared.metrics.set(metrics);
                    }
                    self.shared.playback_state.set(VideoPlaybackState::Paused);
                }
            }
            BackendCommand::Stop => {
                self.set_decode_playing(false);
                self.current_generation = self.current_generation.saturating_add(1);
                self.current_source = None;
                self.current_duration = None;
                self.current_intrinsic_size = IntrinsicSize::ZERO;
                self.current_video_size = VideoSize::default();
                self.current_start_position = Duration::ZERO;
                self.current_audio_clock = None;
                self.last_presented_position = Duration::ZERO;
                self.software_paused_position = Duration::ZERO;
                self.software_play_started_at = None;
                self.should_play = false;
                self.decode_playing = false;
                self.playback_ended = false;
                self.pending_open_reason = None;
                self.stream_opened = false;
                self.startup_pending = false;
                self.buffer_snapshot = BufferSnapshot {
                    generation: self.current_generation,
                    ..BufferSnapshot::default()
                };
                self.playback_clock.set_position(Duration::ZERO);
                self.shared_queue
                    .replace_generation(self.current_generation);
                clear_latest_frame(&self.latest_frame);
                self.shared.reset_for_stop();
                let _ = self.decode_tx.send(DecodeCommand::Stop {
                    generation: self.current_generation,
                });
            }
            BackendCommand::Seek(position) => {
                let Some(source) = self.current_source.clone() else {
                    return true;
                };
                self.set_decode_playing(false);
                self.current_generation = self.current_generation.saturating_add(1);
                self.pending_open_reason = Some(OpenReason::Seek);
                self.stream_opened = false;
                self.startup_pending = true;
                self.current_start_position = position;
                self.current_duration = None;
                self.current_audio_clock = None;
                self.software_paused_position = position;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.buffer_snapshot = BufferSnapshot {
                    generation: self.current_generation,
                    ..BufferSnapshot::default()
                };
                self.playback_clock.set_position(position);
                self.shared_queue
                    .replace_generation(self.current_generation);
                clear_latest_frame(&self.latest_frame);
                self.shared.playback_state.set(VideoPlaybackState::Loading);
                self.shared.error.set(None);
                self.shared.surface.set(VideoSurfaceSnapshot {
                    intrinsic_size: self.current_intrinsic_size,
                    texture: None,
                    loading: true,
                    error: None,
                });
                let _ = self.decode_tx.send(DecodeCommand::Seek {
                    generation: self.current_generation,
                    source,
                    position,
                });
            }
            BackendCommand::SetVolume(volume) => {
                let volume = volume.clamp(0.0, 1.0);
                self.shared.volume.set(volume);
                let _ = self.decode_tx.send(DecodeCommand::SetVolume(volume));
            }
            BackendCommand::SetMuted(muted) => {
                self.shared.muted.set(muted);
                let _ = self.decode_tx.send(DecodeCommand::SetMuted(muted));
            }
            BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.shared.buffer_memory_limit_bytes.set(bytes);
                let _ = self
                    .decode_tx
                    .send(DecodeCommand::SetBufferMemoryLimitBytes(bytes));
            }
            BackendCommand::SetTargetRaster(raster) => {
                let _ = self.decode_tx.send(DecodeCommand::SetTargetRaster(raster));
            }
            BackendCommand::Shutdown => return false,
        }

        true
    }

    fn handle_decode_event(&mut self, event: DecodeEvent) {
        match event {
            DecodeEvent::StreamOpened(opened) => {
                if opened.generation != self.current_generation {
                    return;
                }
                self.stream_opened = true;
                self.current_start_position = opened.start_position;
                self.current_duration = opened.duration;
                self.current_intrinsic_size = opened.intrinsic_size;
                self.current_video_size = opened.video_size;
                self.current_buffering_profile = opened.buffering_profile;
                self.current_audio_clock = opened.audio_clock;
                self.last_presented_position = opened.start_position;
                self.software_paused_position = opened.start_position;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.playback_clock.set_position(opened.start_position);
                self.shared.video_size.set(opened.video_size);
                self.shared.error.set(None);
                self.shared.surface.set(VideoSurfaceSnapshot {
                    intrinsic_size: opened.intrinsic_size,
                    texture: None,
                    loading: true,
                    error: None,
                });
            }
            DecodeEvent::FirstFrameReady { generation, .. } => {
                if generation != self.current_generation {
                    return;
                }
                let _ = self.present_next_frame();
                if self.should_play {
                    self.evaluate_playback_state();
                } else {
                    match self.pending_open_reason.take().unwrap_or(OpenReason::Load) {
                        OpenReason::Load => {
                            self.shared.playback_state.set(VideoPlaybackState::Ready)
                        }
                        OpenReason::Seek => {
                            self.shared.playback_state.set(VideoPlaybackState::Paused)
                        }
                    }
                }
            }
            DecodeEvent::BufferSnapshot(snapshot) => {
                if snapshot.generation != self.current_generation {
                    return;
                }
                self.buffer_snapshot = snapshot;
            }
            DecodeEvent::EofDrained { generation } => {
                if generation != self.current_generation {
                    return;
                }
                self.buffer_snapshot.eof_sent = true;
                if self.shared_queue.ready_frame_count(generation) == 0
                    && self.audio_buffered_duration().is_zero()
                {
                    let position = self.playback_position();
                    self.set_decode_playing(false);
                    self.playback_ended = true;
                    self.should_play = false;
                    self.startup_pending = false;
                    if self.shared.metrics_enabled() {
                        let mut metrics = self.shared.metrics.get();
                        metrics.position = position;
                        self.shared.metrics.set(metrics);
                    }
                    self.shared.playback_state.set(VideoPlaybackState::Ended);
                }
            }
            DecodeEvent::FatalError {
                generation,
                message,
            } => {
                if generation != self.current_generation {
                    return;
                }
                self.set_decode_playing(false);
                self.shared.set_error(message);
            }
        }
    }

    fn sync_metrics(&mut self) {
        if !self.shared.metrics_enabled() {
            return;
        }

        if !self.stream_opened {
            return;
        }

        let position = self.playback_position();
        self.playback_clock.set_position(position);

        let mut metrics = self.shared.metrics.get();
        metrics.duration = self.current_duration;
        metrics.position = position;
        metrics.buffered = self.buffered_position(position);
        metrics.video_width = self.current_video_size.width;
        metrics.video_height = self.current_video_size.height;
        self.shared.metrics.set(metrics);
    }

    fn set_decode_playing(&mut self, playing: bool) {
        if self.decode_playing == playing {
            return;
        }

        self.decode_playing = playing;
        if self.current_audio_clock.is_none() {
            if playing {
                self.software_play_started_at = Some(Instant::now());
            } else {
                let position = self.playback_position();
                self.software_play_started_at = None;
                self.software_paused_position = position;
            }
        }

        let _ = self.decode_tx.send(DecodeCommand::SetPlaying {
            generation: self.current_generation,
            playing,
        });
    }

    fn pause_software_clock(&mut self, position: Duration) {
        self.software_play_started_at = None;
        self.software_paused_position = position;
        self.playback_clock.set_position(position);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    use parking_lot::Mutex;

    use crossbeam_channel::unbounded;

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::video::VideoMetrics;

    use super::super::super::{BackendSharedState, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES};
    use super::*;

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

    fn test_worker() -> (PresentWorker, crossbeam_channel::Receiver<DecodeCommand>) {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let (backend_tx, backend_rx) = unbounded();
        drop(backend_tx);
        let (decode_tx, decode_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        drop(event_tx);

        (
            PresentWorker::new(
                backend_rx,
                decode_tx,
                event_rx,
                shared,
                Arc::new(Mutex::new(None)),
                Arc::new(SharedVideoQueue::new()),
                SharedPlaybackClock::default(),
            ),
            decode_rx,
        )
    }

    #[test]
    fn seek_pauses_previous_generation_before_reopening() {
        let (mut worker, decode_rx) = test_worker();
        worker.current_source = Some(VideoSource::File("demo.mp4".into()));
        worker.current_generation = 7;
        worker.decode_playing = true;

        assert!(worker.handle_backend_command(BackendCommand::Seek(Duration::from_secs(5))));

        assert!(matches!(
            decode_rx
                .recv()
                .expect("pause command should be sent first"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("seek command should follow pause"),
            DecodeCommand::Seek {
                generation: 8,
                position,
                ..
            } if position == Duration::from_secs(5)
        ));
    }

    #[test]
    fn load_pauses_previous_generation_before_reopening() {
        let (mut worker, decode_rx) = test_worker();
        worker.current_generation = 3;
        worker.decode_playing = true;
        let source = VideoSource::File("demo.mp4".into());

        assert!(worker.handle_backend_command(BackendCommand::Load(source.clone())));

        assert!(matches!(
            decode_rx
                .recv()
                .expect("pause command should be sent first"),
            DecodeCommand::SetPlaying {
                generation: 3,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("load command should follow pause"),
            DecodeCommand::Load {
                generation: 4,
                source: queued_source,
            } if queued_source == source
        ));
    }
}
