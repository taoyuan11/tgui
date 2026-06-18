use std::time::Duration;

use crate::audio::{AudioPlaybackState, AudioSnapshot, AudioSource};

use super::AudioWorker;
use crate::audio::backend::ffmpeg::session::AudioSession;
use crate::audio::backend::ffmpeg::BackendCommand;

impl AudioWorker {
    pub(in crate::audio::backend::ffmpeg) fn handle_command(
        &mut self,
        command: BackendCommand,
    ) -> bool {
        match command {
            BackendCommand::Load(source) => {
                self.current_source = Some(source.clone());
                self.should_play = false;
                self.reopen_source(source, Duration::ZERO, false);
            }
            BackendCommand::Play => {
                if self.shared.playback_state.get() == AudioPlaybackState::Ended {
                    self.reopen_current_source(Duration::ZERO, true);
                    return true;
                }
                self.should_play = true;
                if let Some(session) = self.session.as_ref() {
                    session.set_playing(true);
                }
                if self.session.is_some() {
                    self.sync_metrics(true);
                    self.shared.playback_state.set(AudioPlaybackState::Playing);
                }
            }
            BackendCommand::Pause => {
                self.should_play = false;
                if let Some(session) = self.session.as_ref() {
                    session.set_playing(false);
                    self.sync_metrics(true);
                    self.shared.playback_state.set(AudioPlaybackState::Paused);
                }
            }
            BackendCommand::Stop => {
                self.should_play = false;
                self.current_duration = None;
                self.session = None;
                self.last_metrics_sync_at = None;
                self.shared.reset_for_stop();
            }
            BackendCommand::Seek(position) => {
                self.reopen_current_source(position, self.should_play);
            }
            BackendCommand::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(session) = self.session.as_ref() {
                    session.set_volume(self.volume);
                }
            }
            BackendCommand::SetMuted(muted) => {
                self.muted = muted;
                if let Some(session) = self.session.as_ref() {
                    session.set_muted(muted);
                }
            }
            BackendCommand::SetLooping(looping) => {
                self.looping = looping;
            }
            BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.buffer_memory_limit_bytes = bytes;
            }
            BackendCommand::Shutdown => return false,
        }

        true
    }

    pub(super) fn reopen_current_source(&mut self, position: Duration, should_play: bool) {
        let Some(source) = self.current_source.clone() else {
            return;
        };
        self.reopen_source(source, position, should_play);
    }

    fn reopen_source(&mut self, source: AudioSource, position: Duration, should_play: bool) {
        self.should_play = should_play;
        match AudioSession::open(
            source.clone(),
            position,
            self.volume,
            self.muted,
            should_play,
        ) {
            Ok(session) => {
                self.current_duration = session.duration();
                self.session = Some(session);
                if should_play {
                    self.shared.playback_state.set(AudioPlaybackState::Playing);
                } else if position.is_zero() {
                    self.shared.set_ready();
                } else {
                    self.shared.playback_state.set(AudioPlaybackState::Paused);
                    self.shared.snapshot.set(AudioSnapshot {
                        loading: false,
                        error: None,
                    });
                    self.shared.error.set(None);
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
