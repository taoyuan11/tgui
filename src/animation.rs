use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::color::Color;
use crate::ui::layout::Insets;
use crate::ui::unit::{Dp, Sp};
use crate::ui::widget::Point;

const DEFAULT_DURATION_MS: u64 = 180;
const THEME_DURATION_MS: u64 = 240;
pub(crate) const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationCurve {
    Linear,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

impl AnimationCurve {
    pub fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseInCubic => progress * progress * progress,
            Self::EaseOutCubic => 1.0 - (1.0 - progress).powi(3),
            Self::EaseInOutCubic => {
                if progress < 0.5 {
                    4.0 * progress * progress * progress
                } else {
                    1.0 - ((-2.0 * progress + 2.0).powi(3) / 2.0)
                }
            }
        }
    }
}

pub type Easing = AnimationCurve;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

impl PlaybackDirection {
    fn toggled(self) -> Self {
        match self {
            Self::Normal => Self::Reverse,
            Self::Reverse => Self::Normal,
            Self::Alternate => Self::AlternateReverse,
            Self::AlternateReverse => Self::Alternate,
        }
    }

    fn starts_reversed(self) -> bool {
        matches!(self, Self::Reverse | Self::AlternateReverse)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Count(u32),
    Infinite,
}

impl Repeat {
    fn finite_cycles(self) -> Option<u32> {
        match self {
            Self::Count(count) => Some(count.max(1)),
            Self::Infinite => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    delay: Duration,
    repeat: Repeat,
    direction: PlaybackDirection,
    speed: f32,
    fill_mode: FillMode,
}

impl Playback {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn repeat(mut self, repeat: u32) -> Self {
        self.repeat = Repeat::Count(repeat.max(1));
        self
    }

    pub fn repeat_forever(mut self) -> Self {
        self.repeat = Repeat::Infinite;
        self
    }

    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.0);
        self
    }

    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.fill_mode = fill_mode;
        self
    }

    pub fn delay_duration(self) -> Duration {
        self.delay
    }

    pub fn repeat_mode(self) -> Repeat {
        self.repeat
    }

    pub fn direction_mode(self) -> PlaybackDirection {
        self.direction
    }

    pub fn speed_factor(self) -> f32 {
        self.speed
    }

    pub fn fill(self) -> FillMode {
        self.fill_mode
    }
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            delay: Duration::ZERO,
            repeat: Repeat::Count(1),
            direction: PlaybackDirection::Normal,
            speed: 1.0,
            fill_mode: FillMode::Both,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    duration: Duration,
    curve: AnimationCurve,
    playback: Playback,
}

impl Transition {
    pub fn linear(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::Linear,
            playback: Playback::default(),
        }
    }

    pub fn ease_in(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseInCubic,
            playback: Playback::default(),
        }
    }

    pub fn ease_out(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseOutCubic,
            playback: Playback::default(),
        }
    }

    pub fn ease_in_out(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseInOutCubic,
            playback: Playback::default(),
        }
    }

    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.playback = self.playback.delay(delay);
        self
    }

    pub fn repeat(mut self, repeat: u32) -> Self {
        self.playback = self.playback.repeat(repeat);
        self
    }

    pub fn repeat_forever(mut self) -> Self {
        self.playback = self.playback.repeat_forever();
        self
    }

    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.playback = self.playback.direction(direction);
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.playback = self.playback.speed(speed);
        self
    }

    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.playback = self.playback.fill_mode(fill_mode);
        self
    }

    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self
    }

    pub(crate) fn duration(self) -> Duration {
        self.duration
    }

    pub(crate) fn curve_mode(self) -> AnimationCurve {
        self.curve
    }

    pub(crate) fn playback_mode(self) -> Playback {
        self.playback
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::ease_out(Duration::from_millis(DEFAULT_DURATION_MS))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe<T> {
    offset: Duration,
    value: T,
}

impl<T> Keyframe<T> {
    pub fn at(offset: Duration, value: T) -> Self {
        Self { offset, value }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn offset(&self) -> Duration {
        self.offset
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes<T> {
    total_duration: Duration,
    frames: Vec<Keyframe<T>>,
    curve: AnimationCurve,
}

impl<T> Keyframes<T> {
    pub fn timed(total_duration: Duration) -> Self {
        Self {
            total_duration,
            frames: Vec::new(),
            curve: AnimationCurve::Linear,
        }
    }

    pub fn percent(total_duration: Duration) -> Self {
        Self::timed(total_duration)
    }

    pub fn at(mut self, offset: Duration, value: T) -> Self {
        self.frames
            .push(Keyframe::at(offset.min(self.total_duration), value));
        self
    }

    pub fn at_percent(mut self, percent: f32, value: T) -> Self {
        let progress = percent.clamp(0.0, 1.0) as f64;
        let offset = Duration::from_secs_f64(self.total_duration.as_secs_f64() * progress);
        self.frames.push(Keyframe::at(offset, value));
        self
    }

    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }

    pub fn total_duration(&self) -> Duration {
        self.total_duration
    }

    pub fn frames(&self) -> &[Keyframe<T>] {
        &self.frames
    }

    pub fn into_spec(self) -> AnimationSpec<T> {
        self.into()
    }

    fn curve_mode(&self) -> AnimationCurve {
        self.curve
    }

    fn sorted_frames(&self) -> Vec<&Keyframe<T>> {
        let mut frames = self.frames.iter().collect::<Vec<_>>();
        frames.sort_by_key(|frame| frame.offset);
        frames
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationSpec<T> {
    keyframes: Keyframes<T>,
    playback: Playback,
}

impl<T> AnimationSpec<T> {
    pub fn new(keyframes: Keyframes<T>) -> Self {
        Self {
            keyframes,
            playback: Playback::default(),
        }
    }

    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.playback = self.playback.delay(delay);
        self
    }

    pub fn repeat(mut self, repeat: u32) -> Self {
        self.playback = self.playback.repeat(repeat);
        self
    }

    pub fn repeat_forever(mut self) -> Self {
        self.playback = self.playback.repeat_forever();
        self
    }

    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.playback = self.playback.direction(direction);
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.playback = self.playback.speed(speed);
        self
    }

    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.playback = self.playback.fill_mode(fill_mode);
        self
    }

    pub fn keyframes(&self) -> &Keyframes<T> {
        &self.keyframes
    }

    pub fn playback_config(&self) -> Playback {
        self.playback
    }
}

impl<T> From<Keyframes<T>> for AnimationSpec<T> {
    fn from(value: Keyframes<T>) -> Self {
        Self::new(value)
    }
}

impl<T> From<AnimationSpec<T>> for Transition {
    fn from(value: AnimationSpec<T>) -> Self {
        Self {
            duration: value.keyframes.total_duration(),
            curve: value.keyframes.curve_mode(),
            playback: value.playback,
        }
    }
}

#[derive(Clone)]
pub struct AnimatedValue<T> {
    value: Arc<Mutex<T>>,
    invalidation: InvalidationSignal,
}

impl<T> AnimatedValue<T> {
    pub(crate) fn new(value: T, invalidation: InvalidationSignal) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            invalidation,
        }
    }

    pub fn set(&self, value: T) {
        *self.value.lock().expect("animated value lock poisoned") = value;
        self.invalidation.mark_dirty();
    }

    pub fn binding(&self) -> Binding<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let animated = self.clone();
        Binding::new(move || animated.get())
    }
}

impl<T: Clone> AnimatedValue<T> {
    pub fn get(&self) -> T {
        self.value
            .lock()
            .expect("animated value lock poisoned")
            .clone()
    }
}

impl<T: PartialEq> AnimatedValue<T> {
    fn set_if_changed(&self, value: T) -> bool {
        let mut current = self.value.lock().expect("animated value lock poisoned");
        if *current == value {
            return false;
        }
        *current = value;
        drop(current);
        self.invalidation.mark_dirty();
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationStatus {
    Idle,
    Running,
    Paused,
    Stopped,
    Completed,
}

type AnimationCallback = Arc<dyn Fn() + Send + Sync>;

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

    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self.playback_overridden = true;
        self
    }

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

    pub fn on_start(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_start = Some(Arc::new(callback));
        self
    }

    pub fn on_repeat(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_repeat = Some(Arc::new(callback));
        self
    }

    pub fn on_complete(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_complete = Some(Arc::new(callback));
        self
    }

    pub fn on_stop(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_stop = Some(Arc::new(callback));
        self
    }

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

#[derive(Clone)]
pub struct AnimationControllerHandle {
    state: Arc<Mutex<AnimationControllerState>>,
}

impl AnimationControllerHandle {
    pub fn play(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.play(Instant::now());
    }

    pub fn pause(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.pause(Instant::now());
    }

    pub fn resume(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.resume(Instant::now());
    }

    pub fn stop(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.stop(Instant::now());
    }

    pub fn restart(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.restart(Instant::now());
    }

    pub fn seek_time(&self, elapsed: Duration) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.seek_time(Instant::now(), elapsed);
    }

    pub fn seek_percent(&self, percent: f32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.seek_percent(Instant::now(), percent);
    }

    pub fn reverse(&self) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.reverse(Instant::now());
    }

    pub fn set_speed(&self, speed: f32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_speed(Instant::now(), speed);
    }

    pub fn set_iterations(&self, iterations: u32) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_iterations(iterations);
    }

    pub fn set_direction(&self, direction: PlaybackDirection) {
        let mut state = self
            .state
            .lock()
            .expect("animation controller lock poisoned");
        state.set_direction(direction);
    }

    pub fn status(&self) -> AnimationStatus {
        self.state
            .lock()
            .expect("animation controller lock poisoned")
            .status
    }

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

#[derive(Clone, Default)]
pub(crate) struct AnimationCoordinator {
    controllers: Arc<Mutex<Vec<Weak<Mutex<AnimationControllerState>>>>>,
}

impl AnimationCoordinator {
    fn register(&self, controller: &Arc<Mutex<AnimationControllerState>>) {
        self.controllers
            .lock()
            .expect("animation coordinator lock poisoned")
            .push(Arc::downgrade(controller));
    }

    pub(crate) fn refresh(&self, now: Instant) -> bool {
        let mut controllers = self
            .controllers
            .lock()
            .expect("animation coordinator lock poisoned");
        let mut changed = false;
        controllers.retain(|weak| {
            let Some(controller) = weak.upgrade() else {
                return false;
            };
            changed |= controller
                .lock()
                .expect("animation controller lock poisoned")
                .tick(now);
            true
        });
        changed
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        let controllers = self
            .controllers
            .lock()
            .expect("animation coordinator lock poisoned");
        controllers
            .iter()
            .filter_map(|weak| weak.upgrade())
            .any(|controller| {
                controller
                    .lock()
                    .expect("animation controller lock poisoned")
                    .is_running()
            })
            .then_some(now + FRAME_INTERVAL)
    }
}

#[derive(Clone, Copy, Debug)]
struct TimelineSample {
    active: bool,
    completed: bool,
    cycle_index: u32,
    local_time: Duration,
    reversed: bool,
}

fn sample_timeline(
    total_duration: Duration,
    playback: Playback,
    elapsed: Duration,
) -> Option<TimelineSample> {
    let start_reversed = playback.direction_mode().starts_reversed();

    if total_duration.is_zero() {
        return Some(TimelineSample {
            active: true,
            completed: playback.repeat_mode().finite_cycles().is_some(),
            cycle_index: 0,
            local_time: Duration::ZERO,
            reversed: start_reversed,
        });
    }

    let scaled_elapsed =
        Duration::from_secs_f64(elapsed.as_secs_f64() * playback.speed_factor().max(0.0) as f64);

    if scaled_elapsed < playback.delay_duration() {
        return match playback.fill() {
            FillMode::Backwards | FillMode::Both => Some(TimelineSample {
                active: false,
                completed: false,
                cycle_index: 0,
                local_time: if start_reversed {
                    total_duration
                } else {
                    Duration::ZERO
                },
                reversed: start_reversed,
            }),
            FillMode::None | FillMode::Forwards => None,
        };
    }

    let active_elapsed = scaled_elapsed.saturating_sub(playback.delay_duration());
    let cycle_secs = total_duration.as_secs_f64();
    let elapsed_secs = active_elapsed.as_secs_f64();
    let cycles = playback.repeat_mode().finite_cycles();

    if let Some(cycle_count) = cycles {
        let total_secs = cycle_secs * cycle_count as f64;
        if elapsed_secs >= total_secs {
            return match playback.fill() {
                FillMode::Forwards | FillMode::Both => {
                    let cycle_index = cycle_count.saturating_sub(1);
                    let reversed = is_cycle_reversed(playback.direction_mode(), cycle_index);
                    Some(TimelineSample {
                        active: false,
                        completed: true,
                        cycle_index,
                        local_time: if reversed {
                            Duration::ZERO
                        } else {
                            total_duration
                        },
                        reversed,
                    })
                }
                FillMode::None | FillMode::Backwards => None,
            };
        }
    }

    let mut cycle_index = (elapsed_secs / cycle_secs).floor() as u32;
    let mut cycle_time = Duration::from_secs_f64(elapsed_secs % cycle_secs);
    if !active_elapsed.is_zero() && cycle_time.is_zero() {
        cycle_index = cycle_index.saturating_sub(1);
        cycle_time = total_duration;
    }

    Some(TimelineSample {
        active: true,
        completed: false,
        cycle_index,
        local_time: cycle_time,
        reversed: is_cycle_reversed(playback.direction_mode(), cycle_index),
    })
}

fn is_cycle_reversed(direction: PlaybackDirection, cycle_index: u32) -> bool {
    match direction {
        PlaybackDirection::Normal => false,
        PlaybackDirection::Reverse => true,
        PlaybackDirection::Alternate => cycle_index % 2 == 1,
        PlaybackDirection::AlternateReverse => cycle_index % 2 == 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WidgetProperty {
    Background,
    BackgroundAlt,
    BackgroundBlur,
    BorderColor,
    BorderRadius,
    BorderWidth,
    TextColor,
    Opacity,
    Offset,
    SwitchThumbColor,
    SwitchThumbOffset,
    Width,
    Height,
    Margin,
    Padding,
    Gap,
    Grow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WindowProperty {
    ClearColor,
    ThemeBackground,
    ThemeSurface,
    ThemeSurfaceLow,
    ThemeSurfaceHigh,
    ThemePrimary,
    ThemeOnSurface,
    ThemeOnSurfaceMuted,
    ThemePrimaryContainer,
    ThemeFocusRing,
    ThemeSelection,
    ThemeInputBackground,
    ThemeInputBorder,
    ThemeButtonPrimary,
    ThemeButtonSecondary,
    ThemeScrollbarThumb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AnimationKey {
    Widget { id: u64, property: WidgetProperty },
    Window(WindowProperty),
}

pub trait Animatable: Clone + PartialEq + Send + Sync + 'static {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self;
}

impl Animatable for Color {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        fn lerp_channel(from: u8, to: u8, progress: f32) -> u8 {
            (from as f32 + (to as f32 - from as f32) * progress)
                .round()
                .clamp(0.0, 255.0) as u8
        }

        Self::rgba(
            lerp_channel(from.r, to.r, progress),
            lerp_channel(from.g, to.g, progress),
            lerp_channel(from.b, to.b, progress),
            lerp_channel(from.a, to.a, progress),
        )
    }
}

impl Animatable for f32 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        from + (to - from) * progress
    }
}

impl Animatable for Dp {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self(f32::interpolate(&from.0, &to.0, progress))
    }
}

impl Animatable for Sp {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self(f32::interpolate(&from.0, &to.0, progress))
    }
}

impl Animatable for Point {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self {
            x: Dp::interpolate(&from.x, &to.x, progress),
            y: Dp::interpolate(&from.y, &to.y, progress),
        }
    }
}

impl Animatable for Insets {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self {
            left: Dp::interpolate(&from.left, &to.left, progress),
            top: Dp::interpolate(&from.top, &to.top, progress),
            right: Dp::interpolate(&from.right, &to.right, progress),
            bottom: Dp::interpolate(&from.bottom, &to.bottom, progress),
        }
    }
}

impl<T: Animatable> Keyframes<T> {
    pub fn sample_at(&self, time: Duration) -> Option<T> {
        let frames = self.sorted_frames();
        let first = frames.first()?;
        let last = frames.last()?;
        if frames.len() == 1 || time <= first.offset() {
            return Some(first.value().clone());
        }
        if time >= last.offset() {
            return Some(last.value().clone());
        }

        for window in frames.windows(2) {
            let from = window[0];
            let to = window[1];
            if time >= from.offset() && time <= to.offset() {
                let span = to.offset().saturating_sub(from.offset());
                if span.is_zero() {
                    return Some(to.value().clone());
                }
                let elapsed = time.saturating_sub(from.offset());
                let progress = (elapsed.as_secs_f32() / span.as_secs_f32()).clamp(0.0, 1.0);
                let eased = self.curve_mode().sample(progress);
                return Some(T::interpolate(from.value(), to.value(), eased));
            }
        }

        Some(last.value().clone())
    }
}

#[derive(Clone)]
struct ActiveAnimation<T> {
    from: T,
    to: T,
    transition: Transition,
    started_at: Instant,
}

struct SlotState<T> {
    displayed: T,
    target: T,
    animation: Option<ActiveAnimation<T>>,
}

impl<T: Animatable> SlotState<T> {
    fn settled(value: T) -> Self {
        Self {
            displayed: value.clone(),
            target: value,
            animation: None,
        }
    }

    fn sample(&mut self, now: Instant) -> T {
        let Some(animation) = self.animation.as_ref() else {
            return self.displayed.clone();
        };

        let Some(sample) = sample_timeline(
            animation.transition.duration(),
            animation.transition.playback_mode(),
            now.saturating_duration_since(animation.started_at),
        ) else {
            self.displayed = animation.from.clone();
            return self.displayed.clone();
        };

        let progress = if animation.transition.duration().is_zero() {
            1.0
        } else {
            animation.transition.curve_mode().sample(
                sample.local_time.as_secs_f32() / animation.transition.duration().as_secs_f32(),
            )
        };
        self.displayed = if sample.reversed {
            T::interpolate(&animation.to, &animation.from, progress)
        } else {
            T::interpolate(&animation.from, &animation.to, progress)
        };
        if sample.completed {
            self.displayed = if sample.reversed {
                animation.from.clone()
            } else {
                animation.to.clone()
            };
            self.target = self.displayed.clone();
            self.animation = None;
        }
        self.displayed.clone()
    }
}

struct AnimationStore<T> {
    slots: HashMap<AnimationKey, SlotState<T>>,
}

impl<T> Default for AnimationStore<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }
}

impl<T: Animatable> AnimationStore<T> {
    fn resolve(
        &mut self,
        key: AnimationKey,
        target: T,
        transition: Option<Transition>,
        now: Instant,
    ) -> T {
        let Some(transition) = transition.filter(|transition| !transition.duration().is_zero())
        else {
            self.slots.insert(key, SlotState::settled(target.clone()));
            return target;
        };

        let state = self
            .slots
            .entry(key)
            .or_insert_with(|| SlotState::settled(target.clone()));

        let current = state.sample(now);
        if state.target != target {
            state.target = target.clone();
            if current != target {
                state.displayed = current.clone();
                state.animation = Some(ActiveAnimation {
                    from: current,
                    to: target,
                    transition,
                    started_at: now,
                });
            } else {
                state.displayed = target.clone();
                state.animation = None;
            }
        }

        state.sample(now)
    }

    fn refresh(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for state in self.slots.values_mut() {
            let before = state.displayed.clone();
            if state.sample(now) != before {
                changed = true;
            }
        }
        changed
    }

    fn has_active(&self) -> bool {
        self.slots.values().any(|state| state.animation.is_some())
    }
}

#[derive(Default)]
pub(crate) struct AnimationEngine {
    colors: AnimationStore<Color>,
    floats: AnimationStore<f32>,
    dps: AnimationStore<Dp>,
    points: AnimationStore<Point>,
    insets: AnimationStore<Insets>,
}

impl AnimationEngine {
    pub(crate) fn resolve_color(
        &mut self,
        key: AnimationKey,
        target: Color,
        transition: Option<Transition>,
        now: Instant,
    ) -> Color {
        self.colors.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_f32(
        &mut self,
        key: AnimationKey,
        target: f32,
        transition: Option<Transition>,
        now: Instant,
    ) -> f32 {
        self.floats.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_dp(
        &mut self,
        key: AnimationKey,
        target: Dp,
        transition: Option<Transition>,
        now: Instant,
    ) -> Dp {
        self.dps.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_point(
        &mut self,
        key: AnimationKey,
        target: Point,
        transition: Option<Transition>,
        now: Instant,
    ) -> Point {
        self.points.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_insets(
        &mut self,
        key: AnimationKey,
        target: Insets,
        transition: Option<Transition>,
        now: Instant,
    ) -> Insets {
        self.insets.resolve(key, target, transition, now)
    }

    pub(crate) fn refresh(&mut self, now: Instant) -> bool {
        self.colors.refresh(now)
            || self.floats.refresh(now)
            || self.dps.refresh(now)
            || self.points.refresh(now)
            || self.insets.refresh(now)
    }

    pub(crate) fn has_active_animations(&self) -> bool {
        self.colors.has_active()
            || self.floats.has_active()
            || self.dps.has_active()
            || self.points.has_active()
            || self.insets.has_active()
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        self.has_active_animations().then_some(now + FRAME_INTERVAL)
    }
}

pub(crate) fn default_theme_transition() -> Transition {
    Transition::ease_in_out(Duration::from_millis(THEME_DURATION_MS))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        default_theme_transition, sample_timeline, Animatable, AnimationControllerBuilder,
        AnimationCoordinator, AnimationCurve, AnimationEngine, AnimationKey, AnimationSpec,
        AnimationStatus, FillMode, Keyframes, Playback, PlaybackDirection, Transition,
        WidgetProperty,
    };
    use crate::foundation::binding::InvalidationSignal;
    use crate::foundation::color::Color;
    use crate::ui::layout::Insets;
    use crate::ui::unit::dp;
    use crate::ui::widget::Point;

    fn key(property: WidgetProperty) -> AnimationKey {
        AnimationKey::Widget { id: 1, property }
    }

    #[test]
    fn color_interpolation_blends_channels() {
        let start = Color::rgba(0, 20, 40, 60);
        let end = Color::rgba(100, 120, 140, 160);
        assert_eq!(
            Color::interpolate(&start, &end, 0.5),
            Color::rgba(50, 70, 90, 110)
        );
    }

    #[test]
    fn point_interpolation_blends_coordinates() {
        let interpolated = Point::interpolate(
            &Point::new(dp(0.0), dp(10.0)),
            &Point::new(dp(20.0), dp(30.0)),
            0.25,
        );
        assert_eq!(interpolated, Point::new(dp(5.0), dp(15.0)));
    }

    #[test]
    fn insets_interpolation_blends_edges() {
        let interpolated = Insets::interpolate(&Insets::all(dp(0.0)), &Insets::all(dp(20.0)), 0.5);
        assert_eq!(interpolated, Insets::all(dp(10.0)));
    }

    #[test]
    fn unchanged_target_does_not_restart_animation() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::ease_out(Duration::from_millis(100));
        let start = Instant::now();

        assert_eq!(
            engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start),
            0.0
        );
        let mid = start + Duration::from_millis(50);
        let animated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);
        let repeated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);

        assert_eq!(animated, repeated);
        assert!(engine.has_active_animations());
    }

    #[test]
    fn target_change_continues_from_current_value() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start);
        engine.resolve_f32(
            key(WidgetProperty::Opacity),
            10.0,
            Some(transition),
            start + Duration::from_millis(1),
        );
        let mid = start + Duration::from_millis(51);
        let current = engine.resolve_f32(key(WidgetProperty::Opacity), 10.0, Some(transition), mid);
        let redirected =
            engine.resolve_f32(key(WidgetProperty::Opacity), 20.0, Some(transition), mid);

        assert_eq!(current, redirected);
    }

    #[test]
    fn finished_animation_lands_exactly_on_target() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_color(
            key(WidgetProperty::Background),
            Color::BLACK,
            Some(transition),
            start,
        );
        engine.resolve_color(
            key(WidgetProperty::Background),
            Color::WHITE,
            Some(transition),
            start + Duration::from_millis(1),
        );

        let end = start + Duration::from_millis(200);
        assert_eq!(
            engine.resolve_color(
                key(WidgetProperty::Background),
                Color::WHITE,
                Some(transition),
                end,
            ),
            Color::WHITE
        );
        assert!(!engine.has_active_animations());
    }

    #[test]
    fn refresh_reports_when_animated_values_advance() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_point(
            key(WidgetProperty::Offset),
            Point::new(dp(0.0), dp(0.0)),
            Some(transition),
            start,
        );
        engine.resolve_point(
            key(WidgetProperty::Offset),
            Point::new(dp(20.0), dp(0.0)),
            Some(transition),
            start + Duration::from_millis(1),
        );

        assert!(engine.refresh(start + Duration::from_millis(50)));
    }

    #[test]
    fn timed_and_percent_keyframes_land_on_same_value() {
        let timed = Keyframes::timed(Duration::from_millis(200))
            .at(Duration::ZERO, 0.0)
            .at(Duration::from_millis(100), 50.0)
            .at(Duration::from_millis(200), 100.0);
        let percent = Keyframes::percent(Duration::from_millis(200))
            .at_percent(0.0, 0.0)
            .at_percent(0.5, 50.0)
            .at_percent(1.0, 100.0);

        assert_eq!(
            timed.sample_at(Duration::from_millis(100)),
            percent.sample_at(Duration::from_millis(100))
        );
    }

    #[test]
    fn timeline_sampling_respects_alternate_direction() {
        let sample = sample_timeline(
            Duration::from_millis(100),
            Playback::default()
                .repeat(2)
                .direction(PlaybackDirection::Alternate),
            Duration::from_millis(150),
        )
        .expect("sample should exist");

        assert_eq!(sample.cycle_index, 1);
        assert!(sample.reversed);
    }

    #[test]
    fn controller_updates_animated_value_and_completes() {
        let invalidation = InvalidationSignal::new();
        let coordinator = AnimationCoordinator::default();
        let value = super::AnimatedValue::new(0.0f32, invalidation.clone());
        let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation.clone())
            .track(
                value.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(100))
                        .curve(AnimationCurve::Linear)
                        .at(Duration::ZERO, 0.0)
                        .at(Duration::from_millis(100), 10.0),
                ),
            )
            .build();

        handle.play();
        assert!(coordinator.refresh(Instant::now() + Duration::from_millis(50)));
        assert!(value.get() > 0.0);
        coordinator.refresh(Instant::now() + Duration::from_millis(150));
        assert_eq!(handle.status(), AnimationStatus::Completed);
    }

    #[test]
    fn reverse_toggles_playback_direction() {
        let transition = Transition::default().direction(PlaybackDirection::AlternateReverse);
        assert_eq!(
            transition.playback_mode().direction_mode(),
            PlaybackDirection::AlternateReverse
        );
        let reversed = transition.playback_mode().direction_mode().toggled();
        assert_eq!(reversed, PlaybackDirection::Alternate);
    }

    #[test]
    fn theme_transition_uses_non_zero_duration() {
        assert!(default_theme_transition().duration() > Duration::ZERO);
    }

    #[test]
    fn fill_mode_none_hides_values_outside_range() {
        assert!(sample_timeline(
            Duration::from_millis(100),
            Playback::default()
                .delay(Duration::from_millis(10))
                .fill_mode(FillMode::None),
            Duration::ZERO,
        )
        .is_none());
    }
}
