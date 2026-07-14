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
    playback_rate: f32,
    audio_track_selection: VideoAudioTrackSelection,
    subtitle_track_selection: VideoSubtitleTrackSelection,
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
            playback_rate: 1.0,
            audio_track_selection: VideoAudioTrackSelection::Auto,
            subtitle_track_selection: VideoSubtitleTrackSelection::Disabled,
            buffer_memory_limit_bytes: DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
            target_raster: None,
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

            let step_result = session.step();
            if step_result.is_ok() {
                let generation = session.generation;
                for cue in session.drain_subtitle_cues() {
                    let _ = self
                        .event_tx
                        .send(DecodeEvent::SubtitleCue(SubtitleCueEvent {
                            generation,
                            cue: cue.cue,
                            placement: cue.placement,
                            style: cue.style,
                        }));
                }
                for cue in session.drain_subtitle_bitmap_cues() {
                    let _ = self.event_tx.send(DecodeEvent::SubtitleBitmapCue(
                        SubtitleBitmapCueEvent { generation, cue },
                    ));
                }
            }

            match step_result {
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
            DecodeCommand::SetPlaybackRate(rate) => {
                self.playback_rate = normalize_playback_rate(rate);
                if let Some(session) = self.session.as_mut() {
                    session.set_playback_rate(self.playback_rate);
                }
            }
            DecodeCommand::SetAudioTrackSelection(selection) => {
                self.audio_track_selection = selection;
            }
            DecodeCommand::SetSubtitleTrackSelection(selection) => {
                self.subtitle_track_selection = selection;
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
            DecodeCommand::Stop => {
                self.session = None;
                self.shared_queue.clear_all();
                self.playback_clock.set_position(Duration::ZERO);
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
            self.playback_rate,
            self.audio_track_selection,
            self.subtitle_track_selection,
            self.buffer_memory_limit_bytes,
            self.target_raster,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crossbeam_channel::unbounded;

    use super::*;

    #[test]
    fn stop_command_clears_queue_and_clock_without_exiting_worker() {
        let (_command_tx, command_rx) = unbounded();
        let (event_tx, _event_rx) = unbounded();
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();
        let mut worker = DecodeWorker::new(
            command_rx,
            event_tx,
            shared_queue.clone(),
            playback_clock.clone(),
        );

        shared_queue.replace_generation(3);
        shared_queue.push_frames(vec![QueuedVideoFrame {
            generation: 3,
            position: Duration::ZERO,
            end_position: Duration::from_millis(33),
            frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
            compressed_bytes: 4,
            decoded_bytes: 4,
        }]);
        playback_clock.set_position(Duration::from_secs(2));

        assert!(worker.handle_command(DecodeCommand::Stop));

        assert_eq!(shared_queue.ready_frame_count(3), 0);
        assert_eq!(playback_clock.position(), Duration::ZERO);
    }

    #[test]
    fn set_playback_rate_updates_decode_worker_state() {
        let (_command_tx, command_rx) = unbounded();
        let (event_tx, _event_rx) = unbounded();
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();
        let mut worker = DecodeWorker::new(command_rx, event_tx, shared_queue, playback_clock);

        assert!(worker.handle_command(DecodeCommand::SetPlaybackRate(2.25)));
        assert_eq!(worker.playback_rate, 2.25);

        assert!(worker.handle_command(DecodeCommand::SetPlaybackRate(99.0)));
        assert_eq!(worker.playback_rate, 4.0);
    }

    #[test]
    fn set_subtitle_track_selection_updates_decode_worker_state() {
        let (_command_tx, command_rx) = unbounded();
        let (event_tx, _event_rx) = unbounded();
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();
        let mut worker = DecodeWorker::new(command_rx, event_tx, shared_queue, playback_clock);

        assert!(
            worker.handle_command(DecodeCommand::SetSubtitleTrackSelection(
                VideoSubtitleTrackSelection::Stream(9),
            ))
        );

        assert_eq!(
            worker.subtitle_track_selection,
            VideoSubtitleTrackSelection::Stream(9)
        );
    }
}
