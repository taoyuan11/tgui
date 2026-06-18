use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::foundation::binding::{Signal, ViewModelContext};
use crate::foundation::error::TguiError;

use super::backend::{
    ffmpeg::FfmpegAudioBackend, AudioBackend, BackendSharedState,
    DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES,
};
use super::types::{AudioMetrics, AudioPlaybackState, AudioSnapshot, AudioSource};

#[cfg(test)]
mod tests;

#[derive(Clone)]
/// 音频播放控制器。
///
/// 该类型负责把 ViewModel 层的播放命令转发到后端，并暴露可绑定的播放状态信号。
pub struct AudioController {
    inner: Arc<AudioControllerInner>,
}

struct AudioControllerInner {
    shared: BackendSharedState,
    backend: Arc<dyn AudioBackend>,
}

impl AudioController {
    /// 创建一个新的音频控制器。
    ///
    /// # 参数
    /// - `ctx`：用于创建内部状态和信号的 ViewModel 上下文。
    ///
    /// # 返回值
    /// 返回绑定到当前上下文的音频控制器。
    pub fn new(ctx: &ViewModelContext) -> Self {
        let shared = BackendSharedState {
            playback_state: ctx.state(AudioPlaybackState::Idle),
            metrics: ctx.state(AudioMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES),
            error: ctx.state(None),
            snapshot: ctx.state(AudioSnapshot::default()),
        };
        let backend: Arc<dyn AudioBackend> = Arc::new(FfmpegAudioBackend::new(shared.clone()));
        Self::from_parts(shared, backend)
    }

    pub(crate) fn from_parts(shared: BackendSharedState, backend: Arc<dyn AudioBackend>) -> Self {
        Self {
            inner: Arc::new(AudioControllerInner { shared, backend }),
        }
    }

    /// 加载新的音频源。
    ///
    /// # 参数
    /// - `source`：待加载的音频来源。
    ///
    /// # 返回值
    /// 成功时返回 `Ok(())`；如果后端无法接受该音频源则返回错误。
    pub fn load(&self, source: AudioSource) -> Result<(), TguiError> {
        self.inner.shared.reset_for_load();
        self.inner.backend.load(source)
    }

    /// 开始或继续播放当前音频。
    pub fn play(&self) {
        if self.inner.shared.playback_state.get() == AudioPlaybackState::Ended {
            self.inner.backend.seek(Duration::ZERO);
        }
        self.inner.backend.play();
    }

    /// 暂停当前音频播放。
    pub fn pause(&self) {
        self.inner.backend.pause();
    }

    /// 停止当前音频播放并重置进度状态。
    pub fn stop(&self) {
        self.inner.backend.stop();
    }

    /// 跳转到指定播放位置。
    ///
    /// # 参数
    /// - `position`：目标时间点。
    pub fn seek(&self, position: Duration) {
        self.inner.backend.seek(position);
    }

    /// 设置播放音量。
    ///
    /// # 参数
    /// - `volume`：目标音量，超出 `0.0..=1.0` 范围时会被钳制。
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.inner.shared.volume.set(volume);
        self.inner.backend.set_volume(volume);
    }

    /// 设置是否静音。
    ///
    /// # 参数
    /// - `muted`：是否静音。
    pub fn set_muted(&self, muted: bool) {
        self.inner.shared.muted.set(muted);
        self.inner.backend.set_muted(muted);
    }

    /// 设置是否循环播放。
    ///
    /// # 参数
    /// - `looping`：是否启用循环。
    pub fn set_looping(&self, looping: bool) {
        self.inner.shared.looping.set(looping);
        self.inner.backend.set_looping(looping);
    }

    /// 设置音频缓存允许占用的最大内存。
    ///
    /// # 参数
    /// - `bytes`：允许的最大缓存字节数。
    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.inner.shared.buffer_memory_limit_bytes.set(bytes);
        self.inner.backend.set_buffer_memory_limit_bytes(bytes);
    }

    /// 获取可绑定的播放状态信号。
    ///
    /// # 返回值
    /// 返回当前音频播放状态的响应式信号。
    pub fn playback_state(&self) -> Signal<AudioPlaybackState> {
        self.inner.shared.playback_state.signal()
    }

    /// 获取当前播放位置的响应式信号。
    ///
    /// # 返回值
    /// 返回当前播放时间位置。
    pub fn position(&self) -> Signal<Duration> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.position)
    }

    /// 获取总时长的响应式信号。
    ///
    /// # 返回值
    /// 返回音频总时长；未知时为 `None`。
    pub fn duration(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.duration)
    }

    /// 获取已缓冲位置的响应式信号。
    ///
    /// # 返回值
    /// 返回当前后端认为已经缓冲到的位置；未知时为 `None`。
    pub fn buffered_position(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.buffered)
    }

    /// 获取当前音量的响应式信号。
    ///
    /// # 返回值
    /// 返回音量值信号。
    pub fn volume(&self) -> Signal<f32> {
        self.inner.shared.volume.signal()
    }

    /// 获取当前静音状态的响应式信号。
    ///
    /// # 返回值
    /// 返回静音状态信号。
    pub fn muted(&self) -> Signal<bool> {
        self.inner.shared.muted.signal()
    }

    /// 获取当前循环播放状态的响应式信号。
    ///
    /// # 返回值
    /// 返回循环状态信号。
    pub fn looping(&self) -> Signal<bool> {
        self.inner.shared.looping.signal()
    }

    /// 获取最近一次播放错误的响应式信号。
    ///
    /// # 返回值
    /// 返回错误信息；无错误时为 `None`。
    pub fn error(&self) -> Signal<Option<String>> {
        self.inner.shared.error.signal()
    }

    pub(crate) fn snapshot(&self) -> AudioSnapshot {
        self.inner.shared.snapshot.get()
    }
}

impl PartialEq for AudioController {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for AudioController {}
