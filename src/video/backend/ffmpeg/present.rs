use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, Receiver, Sender};

use super::*;

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
                self.playback_clock.set_position(Duration::ZERO);
                self.shared_queue
                    .replace_generation(self.current_generation);
                self.set_decode_playing(false);
                clear_latest_frame(&self.latest_frame);
                self.shared.reset_for_load();
                let _ = self.decode_tx.send(DecodeCommand::Load {
                    generation: self.current_generation,
                    source,
                });
            }
            BackendCommand::Play => {
                if self.playback_ended {
                    self.shared.playback_state.set(PlaybackState::Ended);
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
                    let mut metrics = self.shared.metrics.get();
                    metrics.position = position;
                    self.shared.metrics.set(metrics);
                    self.shared.playback_state.set(PlaybackState::Paused);
                }
            }
            BackendCommand::Seek(position) => {
                let Some(source) = self.current_source.clone() else {
                    return true;
                };
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
                self.playback_clock.set_position(position);
                self.shared_queue
                    .replace_generation(self.current_generation);
                self.set_decode_playing(false);
                self.shared.playback_state.set(PlaybackState::Loading);
                self.shared.error.set(None);
                let current_texture = self
                    .latest_frame
                    .lock()
                    .expect("video frame lock poisoned")
                    .clone();
                self.shared.surface.set(VideoSurfaceSnapshot {
                    intrinsic_size: self.current_intrinsic_size,
                    texture: current_texture,
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
                        OpenReason::Load => self.shared.playback_state.set(PlaybackState::Ready),
                        OpenReason::Seek => self.shared.playback_state.set(PlaybackState::Paused),
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
                    let mut metrics = self.shared.metrics.get();
                    metrics.position = position;
                    self.shared.metrics.set(metrics);
                    self.shared.playback_state.set(PlaybackState::Ended);
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

    fn next_wait_duration(&self) -> Duration {
        if !self.decode_playing {
            return COMMAND_POLL_INTERVAL;
        }

        let Some(next_frame) = self.shared_queue.front(self.current_generation) else {
            return STEP_IDLE_SLEEP;
        };
        let playback = self.playback_position();
        let due_position = playback.saturating_add(VIDEO_PRESENT_TOLERANCE);
        if due_position >= next_frame.position {
            return Duration::ZERO;
        }
        next_frame
            .position
            .saturating_sub(due_position)
            .min(COMMAND_POLL_INTERVAL)
    }

    fn playback_position(&self) -> Duration {
        if let Some(audio_clock) = self.current_audio_clock.as_ref() {
            return self
                .current_start_position
                .saturating_add(audio_clock.position());
        }

        match self.software_play_started_at {
            Some(started_at) => self
                .software_paused_position
                .saturating_add(started_at.elapsed()),
            None => self.software_paused_position,
        }
    }

    fn sync_metrics(&mut self) {
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

    fn buffered_position(&self, current_position: Duration) -> Option<Duration> {
        let audio_buffer_end = self
            .current_audio_clock
            .as_ref()
            .map(|clock| current_position.saturating_add(clock.buffered_duration()));
        let video_buffer_end = self.shared_queue.tail_end_position(self.current_generation);

        match (audio_buffer_end, video_buffer_end) {
            (Some(a), Some(v)) => Some(a.min(v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    fn present_due_frames(&mut self) {
        loop {
            let Some(next_frame) = self.shared_queue.front(self.current_generation) else {
                break;
            };
            if !self.is_frame_due(next_frame.position) {
                break;
            }
            let _ = self.present_next_frame();
        }
    }

    fn present_next_frame(&mut self) -> Option<Duration> {
        let frame = self
            .shared_queue
            .pop_front_matching(self.current_generation)?;
        let position = frame.position;
        let texture = frame.texture;
        *self.latest_frame.lock().expect("video frame lock poisoned") = Some(texture.clone());
        self.shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.current_intrinsic_size,
            texture: Some(texture),
            loading: false,
            error: None,
        });

        self.last_presented_position = position;
        if self.current_audio_clock.is_none() {
            self.software_paused_position = position;
            if self.decode_playing {
                self.software_play_started_at = Some(Instant::now());
            }
        }

        self.playback_clock.set_position(position);

        let mut metrics = self.shared.metrics.get();
        metrics.duration = self.current_duration;
        metrics.position = position;
        metrics.buffered = self.buffered_position(position);
        metrics.video_width = self.current_video_size.width;
        metrics.video_height = self.current_video_size.height;
        self.shared.metrics.set(metrics);
        Some(position)
    }

    fn is_frame_due(&self, position: Duration) -> bool {
        if let Some(audio_clock) = self.current_audio_clock.as_ref() {
            if !audio_clock.has_started_clock() {
                return false;
            }
            let playback = self
                .current_start_position
                .saturating_add(audio_clock.position());
            return playback.saturating_add(VIDEO_PRESENT_TOLERANCE) >= position;
        }

        self.playback_position()
            .saturating_add(VIDEO_PRESENT_TOLERANCE)
            >= position
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

    fn evaluate_playback_state(&mut self) {
        if !self.stream_opened {
            return;
        }

        if self.playback_ended {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Ended);
            return;
        }

        if !self.should_play {
            return;
        }

        if self.should_buffer() && !self.should_keep_draining_eof() {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Buffering);
            return;
        }

        let can_start = if self.startup_pending {
            self.can_start_playback()
        } else {
            self.can_resume_playback()
        };
        if can_start || self.should_keep_draining_eof() {
            self.startup_pending = false;
            self.pending_open_reason = None;
            self.set_decode_playing(true);
            self.shared.playback_state.set(PlaybackState::Playing);
        } else {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Buffering);
        }
    }

    fn remaining_duration(&self) -> Option<Duration> {
        self.current_duration
            .map(|duration| duration.saturating_sub(self.playback_position()))
    }

    fn audio_buffered_duration(&self) -> Duration {
        self.current_audio_clock
            .as_ref()
            .map(|clock| clock.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    fn video_buffered_duration(&self) -> Duration {
        let baseline = self.last_presented_position.max(self.playback_position());
        self.shared_queue
            .tail_end_position(self.current_generation)
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    fn can_start_playback(&self) -> bool {
        let audio_ok = self.current_audio_clock.is_none()
            || self.audio_buffered_duration() >= self.current_buffering_profile.start_buffer_target;
        let video_ok = video_buffer_target_satisfied(
            self.video_buffered_duration(),
            self.current_buffering_profile.video_start_buffer_target,
            self.remaining_duration(),
            self.shared_queue.ready_frame_count(self.current_generation)
                >= self.current_buffering_profile.video_max_packet_count,
        );
        (audio_ok && video_ok)
            || startup_playback_blocked_by_memory_limit(
                self.buffer_snapshot.buffering_constrained_by_memory_limit,
                self.shared_queue.has_frames(self.current_generation),
                self.current_audio_clock.is_some(),
                self.audio_buffered_duration(),
            )
    }

    fn can_resume_playback(&self) -> bool {
        let audio_ok = self.current_audio_clock.is_none()
            || self.audio_buffered_duration() >= self.current_buffering_profile.rebuffer_target;
        let video_ok = video_buffer_target_satisfied(
            self.video_buffered_duration(),
            self.current_buffering_profile.video_resume_buffer_target,
            self.remaining_duration(),
            self.shared_queue.ready_frame_count(self.current_generation)
                >= self.current_buffering_profile.video_max_packet_count,
        );
        (audio_ok && video_ok)
            || startup_playback_blocked_by_memory_limit(
                self.buffer_snapshot.buffering_constrained_by_memory_limit,
                self.shared_queue.has_frames(self.current_generation),
                self.current_audio_clock.is_some(),
                self.audio_buffered_duration(),
            )
    }

    fn should_buffer(&self) -> bool {
        let audio_starving = self
            .current_audio_clock
            .as_ref()
            .map(|clock| {
                clock.buffered_duration() < self.current_buffering_profile.audio_starving_threshold
            })
            .unwrap_or(false);
        let video_starving = should_buffer_video(
            self.video_buffered_duration(),
            VIDEO_REBUFFER_ENTER_THRESHOLD,
            self.remaining_duration(),
        );
        should_buffer_for_rebuffer(
            audio_starving,
            video_starving,
            self.buffer_snapshot.buffering_constrained_by_memory_limit,
        )
    }

    fn should_keep_draining_eof(&self) -> bool {
        self.buffer_snapshot.eof_sent
            && (self.shared_queue.has_frames(self.current_generation)
                || !self.audio_buffered_duration().is_zero())
    }
}
