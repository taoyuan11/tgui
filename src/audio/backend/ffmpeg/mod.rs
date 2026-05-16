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

const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);
const METRICS_SYNC_INTERVAL: Duration = Duration::from_millis(100);

static FFMPEG_INIT: Once = Once::new();

pub(crate) struct FfmpegAudioBackend {
    command_tx: Sender<BackendCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegAudioBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg_next::init();
        });

        let (command_tx, command_rx) = unbounded();
        let worker = thread::spawn(move || {
            worker_main(command_rx, shared);
        });

        Self {
            command_tx,
            worker: Mutex::new(Some(worker)),
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
        self.command_tx
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("audio backend is unavailable".to_string()))
    }

    fn play(&self) {
        let _ = self.command_tx.send(BackendCommand::Play);
    }

    fn pause(&self) {
        let _ = self.command_tx.send(BackendCommand::Pause);
    }

    fn stop(&self) {
        let _ = self.command_tx.send(BackendCommand::Stop);
    }

    fn seek(&self, position: Duration) {
        let _ = self.command_tx.send(BackendCommand::Seek(position));
    }

    fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(BackendCommand::SetVolume(volume));
    }

    fn set_muted(&self, muted: bool) {
        let _ = self.command_tx.send(BackendCommand::SetMuted(muted));
    }

    fn set_looping(&self, looping: bool) {
        let _ = self.command_tx.send(BackendCommand::SetLooping(looping));
    }

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        let _ = self
            .command_tx
            .send(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);

        if let Some(worker) = self
            .worker
            .lock()
            .expect("audio worker lock poisoned")
            .take()
        {
            let _ = worker.join();
        }
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
