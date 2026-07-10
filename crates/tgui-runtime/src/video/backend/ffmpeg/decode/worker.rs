use super::*;
use crate::video::backend::DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES;

pub(in super::super) fn decode_main(
    command_rx: Receiver<DecodeCommand>,
    event_tx: Sender<DecodeEvent>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
) {
    let mut worker = DecodeWorker::new(command_rx, event_tx, shared_queue, playback_clock);
    worker.run();
}

struct DecodeWorker {
    command_rx: Receiver<DecodeCommand>,
    event_tx: Sender<DecodeEvent>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    volume: f32,
    muted: bool,
    buffer_memory_limit_bytes: u64,
    target_raster: Option<RasterRequest>,
    session: Option<DecodeSession>,
}

impl DecodeWorker {
    fn new(
        command_rx: Receiver<DecodeCommand>,
        event_tx: Sender<DecodeEvent>,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Self {
        Self {
            command_rx,
            event_tx,
            shared_queue,
            playback_clock,
            volume: 1.0,
            muted: false,
            buffer_memory_limit_bytes: DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
            target_raster: None,
            session: None,
        }
    }

    fn run(&mut self) {
        loop {
            let can_step = self
                .session
                .as_ref()
                .is_some_and(|session| !session.eof_notified);
            let command = if can_step {
                match self.command_rx.try_recv() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                }
            } else {
                match self.command_rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };

            if let Some(command) = command {
                if !self.handle_command_batch(command) {
                    break;
                }
            }

            let Some(session) = self.session.as_mut() else {
                continue;
            };

            match session.step() {
                Ok(DecodeStepOutcome::Continue { snapshot_changed }) => {
                    if snapshot_changed {
                        let _ = self
                            .event_tx
                            .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    }
                }
                Ok(DecodeStepOutcome::Idle { snapshot_changed }) => {
                    if snapshot_changed {
                        let _ = self
                            .event_tx
                            .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    }
                    thread::sleep(STEP_IDLE_SLEEP);
                }
                Ok(DecodeStepOutcome::EofDrained) => {
                    let generation = session.generation;
                    let _ = self
                        .event_tx
                        .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    let _ = self.event_tx.send(DecodeEvent::EofDrained { generation });
                }
                Err(error) => {
                    let generation = session.generation;
                    let _ = self.event_tx.send(DecodeEvent::FatalError {
                        generation,
                        message: error.to_string(),
                    });
                    self.session = None;
                }
            }
        }
    }

    fn handle_command_batch(&mut self, first: DecodeCommand) -> bool {
        let mut pending_open = None;
        let mut next = Some(first);

        loop {
            if let Some(command) = next.take() {
                match command {
                    command @ (DecodeCommand::Load { .. }
                    | DecodeCommand::Seek { .. }
                    | DecodeCommand::Stop { .. }) => {
                        pending_open = Some(command);
                    }
                    DecodeCommand::Shutdown => return false,
                    command => {
                        if !self.handle_command(command) {
                            return false;
                        }
                    }
                }
            }

            next = match self.command_rx.try_recv() {
                Ok(command) => Some(command),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            };
        }

        pending_open
            .map(|command| self.handle_command(command))
            .unwrap_or(true)
    }

    fn handle_command(&mut self, command: DecodeCommand) -> bool {
        match command {
            DecodeCommand::Load { generation, source } => {
                self.open_session(OpenReason::Load, generation, source, Duration::ZERO);
            }
            DecodeCommand::Seek {
                generation,
                source,
                position,
            } => {
                self.seek_or_open_session(generation, source, position);
            }
            DecodeCommand::SetPlaying {
                generation,
                playing,
            } => {
                if let Some(session) = self.session.as_mut() {
                    if session.generation == generation {
                        session.set_playing(playing);
                    }
                }
            }
            DecodeCommand::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(session) = self.session.as_mut() {
                    session.set_volume(self.volume);
                }
            }
            DecodeCommand::SetMuted(muted) => {
                self.muted = muted;
                if let Some(session) = self.session.as_mut() {
                    session.set_muted(muted);
                }
            }
            DecodeCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.buffer_memory_limit_bytes = bytes;
                if let Some(session) = self.session.as_mut() {
                    session.set_buffer_memory_limit_bytes(bytes);
                }
            }
            DecodeCommand::SetTargetRaster(raster) => {
                self.target_raster = raster;
                if let Some(session) = self.session.as_mut() {
                    session.set_target_raster(raster);
                }
            }
            DecodeCommand::Stop { generation } => {
                if self.shared_queue.accepted_generation() == generation {
                    self.session = None;
                    self.playback_clock.set_position(Duration::ZERO);
                }
            }
            DecodeCommand::Shutdown => return false,
        }

        true
    }

    fn seek_or_open_session(
        &mut self,
        generation: u64,
        source: VideoSource,
        position: Duration,
    ) {
        if self.shared_queue.accepted_generation() != generation {
            return;
        }

        let reuse_result = self
            .session
            .as_mut()
            .filter(|session| session.source() == &source)
            .map(|session| session.seek_in_place(generation, position));

        if let Some(Ok((stream_opened, first_frame_position))) = reuse_result {
            self.publish_opened_session(stream_opened, first_frame_position);
            if let Some(session) = self.session.as_ref() {
                let _ = self
                    .event_tx
                    .send(DecodeEvent::BufferSnapshot(session.snapshot()));
            }
            return;
        }

        self.session = None;
        self.open_session(OpenReason::Seek, generation, source, position);
    }

    fn open_session(
        &mut self,
        reason: OpenReason,
        generation: u64,
        source: VideoSource,
        position: Duration,
    ) {
        if self.shared_queue.accepted_generation() != generation {
            return;
        }

        self.session = None;

        match DecodeSession::open(
            reason,
            generation,
            source,
            position,
            self.volume,
            self.muted,
            self.buffer_memory_limit_bytes,
            self.target_raster,
            self.shared_queue.clone(),
            self.playback_clock.clone(),
        ) {
            Ok((session, stream_opened, first_frame_position)) => {
                self.publish_opened_session(stream_opened, first_frame_position);
                let _ = self
                    .event_tx
                    .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                self.session = Some(session);
            }
            Err(error) => {
                let _ = self.event_tx.send(DecodeEvent::FatalError {
                    generation,
                    message: error.to_string(),
                });
                self.session = None;
            }
        }
    }

    fn publish_opened_session(
        &self,
        stream_opened: StreamOpenedEvent,
        first_frame_position: Duration,
    ) {
        let generation = stream_opened.generation;
        let _ = self
            .event_tx
            .send(DecodeEvent::StreamOpened(stream_opened));
        let _ = self.event_tx.send(DecodeEvent::FirstFrameReady {
            generation,
            _position: first_frame_position,
        });
    }
}
