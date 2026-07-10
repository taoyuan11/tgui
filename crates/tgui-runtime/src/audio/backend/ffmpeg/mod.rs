use std::sync::{Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};

use crate::foundation::error::TguiError;

use super::{AudioBackend, BackendSharedState};
use crate::audio::AudioSource;

mod session;
#[cfg(test)]
mod tests;
mod worker;

use session::validate_audio_source;
use worker::worker_main;

const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);
const METRICS_SYNC_INTERVAL: Duration = Duration::from_millis(100);

static FFMPEG_INIT: Once = Once::new();

pub(crate) struct FfmpegAudioBackend {
    shared: BackendSharedState,
    worker: Mutex<Option<AudioWorkerHandle>>,
}

struct AudioWorkerHandle {
    command_tx: Sender<BackendCommand>,
    worker: JoinHandle<()>,
}

impl FfmpegAudioBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        Self {
            shared,
            worker: Mutex::new(None),
        }
    }

    fn ensure_worker(&self) -> Result<Sender<BackendCommand>, TguiError> {
        let mut guard = self.worker.lock().expect("audio worker lock poisoned");
        if let Some(handle) = guard.as_ref() {
            return Ok(handle.command_tx.clone());
        }

        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg_next::init();
        });

        let (command_tx, command_rx) = unbounded();
        let shared = self.shared.clone();
        let worker = thread::spawn(move || {
            worker_main(command_rx, shared);
        });

        *guard = Some(AudioWorkerHandle {
            command_tx: command_tx.clone(),
            worker,
        });

        Ok(command_tx)
    }

    fn active_command_tx(&self) -> Option<Sender<BackendCommand>> {
        self.worker
            .lock()
            .expect("audio worker lock poisoned")
            .as_ref()
            .map(|handle| handle.command_tx.clone())
    }

    fn send_if_active(&self, command: BackendCommand) {
        if let Some(command_tx) = self.active_command_tx() {
            let _ = command_tx.send(command);
        }
    }
}

impl Drop for FfmpegAudioBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl AudioBackend for FfmpegAudioBackend {
    fn load(&self, source: AudioSource) -> Result<(), TguiError> {
        validate_audio_source(&source)?;
        self.ensure_worker()?
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("audio backend is unavailable".to_string()))
    }

    fn play(&self) {
        self.send_if_active(BackendCommand::Play);
    }

    fn pause(&self) {
        self.send_if_active(BackendCommand::Pause);
    }

    fn stop(&self) {
        if self.active_command_tx().is_some() {
            self.send_if_active(BackendCommand::Stop);
        } else {
            self.shared.reset_for_stop();
        }
    }

    fn seek(&self, position: Duration) {
        self.send_if_active(BackendCommand::Seek(position));
    }

    fn set_volume(&self, volume: f32) {
        self.send_if_active(BackendCommand::SetVolume(volume));
    }

    fn set_muted(&self, muted: bool) {
        self.send_if_active(BackendCommand::SetMuted(muted));
    }

    fn set_looping(&self, looping: bool) {
        self.send_if_active(BackendCommand::SetLooping(looping));
    }

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.send_if_active(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn shutdown(&self) {
        let Some(handle) = self
            .worker
            .lock()
            .expect("audio worker lock poisoned")
            .take()
        else {
            return;
        };

        let _ = handle.command_tx.send(BackendCommand::Shutdown);
        let _ = handle.worker.join();
    }
}

#[derive(Clone, Debug)]
pub(super) enum BackendCommand {
    Load(AudioSource),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    SetLooping(bool),
    SetBufferMemoryLimitBytes(u64),
    Shutdown,
}
