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
            session: None,
        }
    }

    fn run(&mut self) {
        loop {
            let command_result = if self.session.is_some() {
                match self.command_rx.try_recv() {
                    Ok(command) => Ok(command),
                    Err(TryRecvError::Empty) => Err(RecvTimeoutError::Timeout),
                    Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
                }
            } else {
                self.command_rx.recv_timeout(COMMAND_POLL_INTERVAL)
            };

            match command_result {
                Ok(command) => {
                    if !self.handle_command(command) {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
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
                    thread::sleep(STEP_IDLE_SLEEP);
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
                self.open_session(OpenReason::Seek, generation, source, position);
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
            DecodeCommand::Shutdown => return false,
        }

        true
    }

    fn open_session(
        &mut self,
        reason: OpenReason,
        generation: u64,
        source: VideoSource,
        position: Duration,
    ) {
        self.shared_queue.replace_generation(generation);

        match DecodeSession::open(
            reason,
            generation,
            source,
            position,
            self.volume,
            self.muted,
            self.buffer_memory_limit_bytes,
            self.shared_queue.clone(),
            self.playback_clock.clone(),
        ) {
            Ok((session, stream_opened, first_frame_position)) => {
                let _ = self
                    .event_tx
                    .send(DecodeEvent::StreamOpened(stream_opened.clone()));
                let _ = self.event_tx.send(DecodeEvent::FirstFrameReady {
                    generation,
                    _position: first_frame_position,
                });
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
}
