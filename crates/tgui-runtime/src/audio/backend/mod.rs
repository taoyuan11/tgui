use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::foundation::binding::State;
use crate::foundation::error::TguiError;

use super::types::{AudioMetrics, AudioPlaybackState, AudioSnapshot, AudioSource};

pub(crate) mod ffmpeg;
pub(crate) mod shared;

pub(crate) const DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct BackendSharedState {
    pub playback_state: State<AudioPlaybackState>,
    pub metrics: State<AudioMetrics>,
    pub volume: State<f32>,
    pub muted: State<bool>,
    pub looping: State<bool>,
    pub playback_rate: State<f32>,
    pub metrics_observed: Arc<AtomicBool>,
    pub buffer_memory_limit_bytes: State<u64>,
    pub error: State<Option<String>>,
    pub snapshot: State<AudioSnapshot>,
}

impl BackendSharedState {
    pub fn enable_metrics(&self) {
        self.metrics_observed.store(true, Ordering::SeqCst);
    }

    pub fn metrics_enabled(&self) -> bool {
        self.metrics_observed.load(Ordering::SeqCst)
    }

    pub fn reset_for_load(&self) {
        self.playback_state.set(AudioPlaybackState::Loading);
        self.metrics.set(AudioMetrics::default());
        self.error.set(None);
        self.snapshot.set(AudioSnapshot {
            loading: true,
            error: None,
        });
    }

    pub fn reset_for_stop(&self) {
        self.playback_state.set(AudioPlaybackState::Idle);
        self.metrics.set(AudioMetrics::default());
        self.error.set(None);
        self.snapshot.set(AudioSnapshot::default());
    }

    pub fn set_ready(&self) {
        self.snapshot.set(AudioSnapshot {
            loading: false,
            error: None,
        });
        self.error.set(None);
        self.playback_state.set(AudioPlaybackState::Ready);
    }

    pub fn set_error(&self, message: String) {
        self.playback_state
            .set(AudioPlaybackState::Error(message.clone()));
        self.error.set(Some(message.clone()));
        self.snapshot.set(AudioSnapshot {
            loading: false,
            error: Some(message),
        });
    }
}

pub(crate) trait AudioBackend: Send + Sync {
    fn load(&self, source: AudioSource) -> Result<(), TguiError>;
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, position: std::time::Duration);
    fn set_volume(&self, volume: f32);
    fn set_muted(&self, muted: bool);
    fn set_looping(&self, looping: bool);
    fn set_playback_rate(&self, rate: f32);
    fn set_buffer_memory_limit_bytes(&self, bytes: u64);
    fn shutdown(&self);
}
