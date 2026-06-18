use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::foundation::binding::InvalidationSignal;

use super::spec::{AnimationSpec, FillMode, Keyframes, Playback, PlaybackDirection};
use super::value::AnimatedValue;
use super::Animatable;

mod timeline;

pub(crate) use self::timeline::{sample_timeline, AnimationCoordinator};

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

/// 表示控制器当前的生命周期状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationStatus {
    Idle,
    Running,
    Paused,
    Stopped,
    Completed,
}

type AnimationCallback = Arc<dyn Fn() + Send + Sync>;

/// 用于构建动画控制器。
pub struct AnimationControllerBuilder {
    coordinator: AnimationCoordinator,
    invalidation: InvalidationSignal,
    playback: Playback,
    playback_overridden: bool,
    tracks: Vec<Box<dyn TrackRunner + Send>>,
    on_start: Option<AnimationCallback>,
    on_repeat: Option<AnimationCallback>,
    on_complete: Option<AnimationCallback>,
    on_stop: Option<AnimationCallback>,
}

impl AnimationControllerBuilder {
    pub(crate) fn new(coordinator: AnimationCoordinator, invalidation: InvalidationSignal) -> Self {
        Self {
            coordinator,
            invalidation,
            playback: Playback::default(),
            playback_overridden: false,
            tracks: Vec::new(),
            on_start: None,
            on_repeat: None,
            on_complete: None,
            on_stop: None,
        }
    }

    /// 设置控制器级播放配置。
    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self.playback_overridden = true;
        self
    }

    /// 添加一个由关键帧驱动的动画轨道。
    ///
    /// 参数：
    /// - `value`：被驱动的目标值。
    /// - `spec`：轨道动画描述。
    ///
    /// 返回值：
    /// - 返回构建器自身，便于链式调用。
    pub fn track<T>(mut self, value: AnimatedValue<T>, spec: AnimationSpec<T>) -> Self
    where
        T: Animatable,
    {
        if self.tracks.is_empty() && !self.playback_overridden {
            self.playback = spec.playback_config();
        }
        self.tracks.push(Box::new(TypedTrack {
            target: value,
            keyframes: spec.keyframes,
        }));
        self
    }

    /// 注册首次进入运行态时的回调。
    pub fn on_start(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_start = Some(Arc::new(callback));
        self
    }

    /// 注册每次进入新周期时的回调。
    pub fn on_repeat(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_repeat = Some(Arc::new(callback));
        self
    }

    /// 注册动画完成后的回调。
    pub fn on_complete(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_complete = Some(Arc::new(callback));
        self
    }

    /// 注册显式停止后的回调。
    pub fn on_stop(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_stop = Some(Arc::new(callback));
        self
    }

    /// 生成控制器句柄并注册到协调器。
    pub fn build(self) -> AnimationControllerHandle {
        let cycle_duration = self
            .tracks
            .iter()
            .map(|track| track.total_duration())
            .max()
            .unwrap_or(Duration::ZERO);
        let state = Arc::new(Mutex::new(AnimationControllerState {
            playback: self.playback,
            status: AnimationStatus::Idle,
            started_at: None,
            accumulated: Duration::ZERO,
            cycle_duration,
            tracks: self.tracks,
            last_cycle_index: None,
            started_once: false,
            on_start: self.on_start,
            on_repeat: self.on_repeat,
            on_complete: self.on_complete,
            on_stop: self.on_stop,
            invalidation: self.invalidation,
        }));
        self.coordinator.register(&state);
        AnimationControllerHandle { state }
    }
}

/// 动画控制器句柄。
#[derive(Clone)]
pub struct AnimationControllerHandle {
    state: Arc<Mutex<AnimationControllerState>>,
}

impl AnimationControllerHandle {
    /// 开始播放动画。
    pub fn play(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.play(Instant::now());
    }

    /// 暂停动画。
    pub fn pause(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.pause(Instant::now());
    }

    /// 恢复已暂停的动画。
    pub fn resume(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.resume(Instant::now());
    }

    /// 停止动画并重置到初始位置。
    pub fn stop(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.stop(Instant::now());
    }

    /// 从头重新开始播放。
    pub fn restart(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.restart(Instant::now());
    }

    /// 按时间偏移跳转到指定位置。
    pub fn seek_time(&self, elapsed: Duration) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.seek_time(Instant::now(), elapsed);
    }

    /// 按百分比跳转到指定位置。
    pub fn seek_percent(&self, percent: f32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.seek_percent(Instant::now(), percent);
    }

    /// 反转当前播放方向。
    pub fn reverse(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.reverse(Instant::now());
    }

    /// 设置播放速度。
    pub fn set_speed(&self, speed: f32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_speed(Instant::now(), speed);
    }

    /// 设置有限循环次数。
    pub fn set_iterations(&self, iterations: u32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_iterations(iterations);
    }

    /// 设置播放方向。
    pub fn set_direction(&self, direction: PlaybackDirection) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_direction(direction);
    }

    /// 查询当前状态。
    pub fn status(&self) -> AnimationStatus {
        self.state
            .lock()
            .expect("animation controller lock poisoned")
            .status
    }

    /// 查询当前周期内的进度百分比。
    pub fn progress(&self) -> f32 {
        self.state
            .lock()
            .expect("animation controller lock poisoned")
            .progress(Instant::now())
    }
}

trait TrackRunner {
    fn total_duration(&self) -> Duration;
    fn apply_sample(&mut self, cycle_time: Duration, reversed: bool) -> bool;
}

struct TypedTrack<T> {
    target: AnimatedValue<T>,
    keyframes: Keyframes<T>,
}

impl<T: Animatable> TrackRunner for TypedTrack<T> {
    fn total_duration(&self) -> Duration {
        self.keyframes.total_duration()
    }

    fn apply_sample(&mut self, cycle_time: Duration, reversed: bool) -> bool {
        let value = self.keyframes.sample_at(if reversed {
            self.keyframes
                .total_duration()
                .checked_sub(cycle_time.min(self.keyframes.total_duration()))
                .unwrap_or(Duration::ZERO)
        } else {
            cycle_time.min(self.keyframes.total_duration())
        });
        value
            .map(|value| self.target.set_if_changed(value))
            .unwrap_or(false)
    }
}

struct AnimationControllerState {
    playback: Playback,
    status: AnimationStatus,
    started_at: Option<Instant>,
    accumulated: Duration,
    cycle_duration: Duration,
    tracks: Vec<Box<dyn TrackRunner + Send>>,
    last_cycle_index: Option<u32>,
    started_once: bool,
    on_start: Option<AnimationCallback>,
    on_repeat: Option<AnimationCallback>,
    on_complete: Option<AnimationCallback>,
    on_stop: Option<AnimationCallback>,
    invalidation: InvalidationSignal,
}

impl AnimationControllerState {
    fn play(&mut self, now: Instant) {
        if matches!(self.status, AnimationStatus::Running) {
            return;
        }
        if matches!(
            self.status,
            AnimationStatus::Idle | AnimationStatus::Stopped | AnimationStatus::Completed
        ) {
            self.accumulated = Duration::ZERO;
            self.last_cycle_index = None;
            self.started_once = false;
        }
        self.started_at = Some(now);
        self.status = AnimationStatus::Running;
        self.invalidation.mark_dirty();
    }

    fn pause(&mut self, now: Instant) {
        if !matches!(self.status, AnimationStatus::Running) {
            return;
        }
        self.accumulated = self.elapsed_at(now);
        self.started_at = None;
        self.status = AnimationStatus::Paused;
        self.invalidation.mark_dirty();
    }

    fn resume(&mut self, now: Instant) {
        if matches!(self.status, AnimationStatus::Paused) {
            self.started_at = Some(now);
            self.status = AnimationStatus::Running;
            self.invalidation.mark_dirty();
        }
    }

    fn stop(&mut self, now: Instant) {
        self.started_at = None;
        self.accumulated = Duration::ZERO;
        self.status = AnimationStatus::Stopped;
        self.last_cycle_index = None;
        self.started_once = false;
        self.apply_sample(now);
        if let Some(callback) = self.on_stop.clone() {
            callback();
        }
        self.invalidation.mark_dirty();
    }

    fn restart(&mut self, now: Instant) {
        self.accumulated = Duration::ZERO;
        self.started_at = Some(now);
        self.status = AnimationStatus::Running;
        self.last_cycle_index = None;
        self.started_once = false;
        self.apply_sample(now);
        self.invalidation.mark_dirty();
    }

    fn seek_time(&mut self, now: Instant, elapsed: Duration) {
        self.accumulated = elapsed;
        if matches!(self.status, AnimationStatus::Running) {
            self.started_at = Some(now);
        }
        self.apply_sample(now);
        self.invalidation.mark_dirty();
    }

    fn seek_percent(&mut self, now: Instant, percent: f32) {
        let target = Duration::from_secs_f64(
            self.cycle_duration.as_secs_f64() * percent.clamp(0.0, 1.0) as f64,
        );
        self.seek_time(now, target);
    }

    fn reverse(&mut self, now: Instant) {
        self.accumulated = self.elapsed_at(now);
        self.started_at = if matches!(self.status, AnimationStatus::Running) {
            Some(now)
        } else {
            None
        };
        self.playback = self
            .playback
            .direction(self.playback.direction_mode().toggled());
        self.apply_sample(now);
        self.invalidation.mark_dirty();
    }

    fn set_speed(&mut self, now: Instant, speed: f32) {
        self.accumulated = self.elapsed_at(now);
        self.started_at = if matches!(self.status, AnimationStatus::Running) {
            Some(now)
        } else {
            None
        };
        self.playback = self.playback.speed(speed);
        self.invalidation.mark_dirty();
    }

    fn set_iterations(&mut self, iterations: u32) {
        self.playback = self.playback.repeat(iterations);
        self.invalidation.mark_dirty();
    }

    fn set_direction(&mut self, direction: PlaybackDirection) {
        self.playback = self.playback.direction(direction);
        self.invalidation.mark_dirty();
    }

    fn elapsed_at(&self, now: Instant) -> Duration {
        let running = self
            .started_at
            .map(|started_at| now.saturating_duration_since(started_at))
            .unwrap_or(Duration::ZERO);
        let scaled =
            Duration::from_secs_f64(running.as_secs_f64() * self.playback.speed_factor() as f64);
        self.accumulated.saturating_add(scaled)
    }

    fn progress(&self, now: Instant) -> f32 {
        let Some(sample) =
            sample_timeline(self.cycle_duration, self.playback, self.elapsed_at(now))
        else {
            return 0.0;
        };
        if self.cycle_duration.is_zero() {
            return if sample.completed { 1.0 } else { 0.0 };
        }
        (sample.local_time.as_secs_f32() / self.cycle_duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    fn apply_sample(&mut self, now: Instant) -> bool {
        let mut changed = false;
        let elapsed = self.elapsed_at(now);
        let Some(sample) = sample_timeline(self.cycle_duration, self.playback, elapsed) else {
            return false;
        };

        if sample.active && !self.started_once {
            self.started_once = true;
            if let Some(callback) = self.on_start.clone() {
                callback();
            }
        }

        if let Some(previous_cycle) = self.last_cycle_index {
            if sample.cycle_index > previous_cycle {
                if let Some(callback) = self.on_repeat.clone() {
                    callback();
                }
            }
        }
        self.last_cycle_index = Some(sample.cycle_index);

        for track in &mut self.tracks {
            changed |= track.apply_sample(sample.local_time, sample.reversed);
        }

        if sample.completed && matches!(self.status, AnimationStatus::Running) {
            self.status = AnimationStatus::Completed;
            self.started_at = None;
            if let Some(callback) = self.on_complete.clone() {
                callback();
            }
        }

        changed
    }

    fn tick(&mut self, now: Instant) -> bool {
        if !matches!(self.status, AnimationStatus::Running) {
            return false;
        }
        let changed = self.apply_sample(now);
        if changed {
            self.invalidation.mark_dirty();
        }
        changed
    }

    fn is_running(&self) -> bool {
        matches!(self.status, AnimationStatus::Running)
    }
}
