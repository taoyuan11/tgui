use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, Receiver};

use crate::audio::{AudioMetrics, AudioPlaybackState, AudioSource};

use super::super::BackendSharedState;
use super::session::{AudioSession, SessionStep};
use super::{BackendCommand, COMMAND_POLL_INTERVAL, METRICS_SYNC_INTERVAL, STEP_IDLE_SLEEP};

mod commands;

pub(super) fn worker_main(command_rx: Receiver<BackendCommand>, shared: BackendSharedState) {
    let mut worker = AudioWorker::new(command_rx, shared);
    worker.run();
}

pub(super) struct AudioWorker {
    command_rx: Receiver<BackendCommand>,
    pub(super) shared: BackendSharedState,
    pub(super) current_source: Option<AudioSource>,
    pub(super) current_duration: Option<Duration>,
    pub(super) should_play: bool,
    pub(super) looping: bool,
    pub(super) volume: f32,
    pub(super) muted: bool,
    pub(super) buffer_memory_limit_bytes: u64,
    pub(super) session: Option<AudioSession>,
    pub(super) last_metrics_sync_at: Option<Instant>,
}

impl AudioWorker {
    pub(super) fn new(command_rx: Receiver<BackendCommand>, shared: BackendSharedState) -> Self {
        Self {
            command_rx,
            looping: shared.looping.get(),
            volume: shared.volume.get(),
            muted: shared.muted.get(),
            buffer_memory_limit_bytes: shared.buffer_memory_limit_bytes.get(),
            shared,
            current_source: None,
            current_duration: None,
            should_play: false,
            session: None,
            last_metrics_sync_at: None,
        }
    }

    fn run(&mut self) {
        loop {
            self.sync_metrics_if_due();

            let timeout = after(if self.session.is_some() {
                COMMAND_POLL_INTERVAL
            } else {
                Duration::from_millis(50)
            });

            select! {
                recv(self.command_rx) -> message => {
                    match message {
                        Ok(command) => {
                            if !self.handle_command(command) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                recv(timeout) -> _ => {}
            }

            let outcome = if let Some(session) = self.session.as_mut() {
                session.step(self.buffer_memory_limit_bytes)
            } else {
                continue;
            };

            match outcome {
                Ok(SessionStep::Continue) => {}
                Ok(SessionStep::Idle) => {
                    thread::sleep(STEP_IDLE_SLEEP);
                }
                Ok(SessionStep::EofDrained) => {
                    // 播放到末尾后，loop 模式会按原语义重新打开当前源并从头开始。
                    if self.looping {
                        let should_play = self.should_play;
                        self.reopen_current_source(Duration::ZERO, should_play);
                    } else {
                        self.should_play = false;
                        self.sync_metrics(true);
                        self.shared.playback_state.set(AudioPlaybackState::Ended);
                    }
                }
                Err(error) => {
                    self.session = None;
                    self.last_metrics_sync_at = None;
                    self.shared.set_error(error.to_string());
                }
            }
        }
    }

    fn sync_metrics_if_due(&mut self) {
        let should_sync = self
            .last_metrics_sync_at
            .map(|last| last.elapsed() >= METRICS_SYNC_INTERVAL)
            .unwrap_or(true);
        if should_sync {
            self.sync_metrics(false);
        }
    }

    fn sync_metrics(&mut self, force: bool) {
        if !self.shared.metrics_enabled() {
            return;
        }

        let Some(session) = self.session.as_ref() else {
            return;
        };

        if !force
            && self
                .last_metrics_sync_at
                .is_some_and(|last| last.elapsed() < METRICS_SYNC_INTERVAL)
        {
            return;
        }

        // metrics 只在外部真正订阅后才同步，避免后台线程做无意义状态写入。
        self.shared.metrics.set(AudioMetrics {
            duration: self.current_duration,
            position: session.position(),
            buffered: Some(session.buffered_position()),
        });
        self.last_metrics_sync_at = Some(Instant::now());
    }
}
