//! UI-thread animation scheduling and deterministic time contracts.
//!
//! A [`Timeline`] is the only timer-like object in the animation system. It is
//! driven by one shared [`FrameClock`], owns all active tracks, and tells the
//! host whether another frame is required. [`Animated`] values are presentation
//! values: sampling a track never writes the application State that supplied a
//! new target.

use crate::core::{AnimationId, Color, ElementId, Point, PropertyId, Size};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

thread_local! {
    static NEXT_TIMELINE_ID: Cell<u64> = const { Cell::new(1) };
}

/// Monotonic time source shared by all animations in a UI runtime.
pub trait FrameClock {
    fn now(&self) -> Duration;
}

impl<T: FrameClock + ?Sized> FrameClock for Rc<T> {
    fn now(&self) -> Duration {
        (**self).now()
    }
}

#[derive(Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// Stable identity of one animated retained property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnimationKey {
    element: ElementId,
    property: PropertyId,
}

impl AnimationKey {
    pub const fn new(element: ElementId, property: PropertyId) -> Self {
        Self { element, property }
    }

    pub const fn element(self) -> ElementId {
        self.element
    }

    pub const fn property(self) -> PropertyId {
        self.property
    }
}

impl From<(ElementId, PropertyId)> for AnimationKey {
    fn from((element, property): (ElementId, PropertyId)) -> Self {
        Self::new(element, property)
    }
}

/// The precise retained-tree work caused by an animated property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnimationImpact {
    Paint,
    Layout,
}

/// One property whose sampled presentation value changed during a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationInvalidation {
    key: AnimationKey,
    impact: AnimationImpact,
}

impl AnimationInvalidation {
    pub const fn key(self) -> AnimationKey {
        self.key
    }

    pub const fn element(self) -> ElementId {
        self.key.element
    }

    pub const fn property(self) -> PropertyId {
        self.key.property
    }

    pub const fn impact(self) -> AnimationImpact {
        self.impact
    }
}

/// Interpolation supported by a value stored in [`Animated`].
pub trait Interpolate: Clone + PartialEq + 'static {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        from + (to - from) * progress
    }
}

impl Interpolate for f64 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        from + (to - from) * f64::from(progress)
    }
}

impl Interpolate for Point {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self::new(
            f32::interpolate(&from.x, &to.x, progress),
            f32::interpolate(&from.y, &to.y, progress),
        )
    }
}

impl Interpolate for Size {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self::new(
            f32::interpolate(&from.width, &to.width, progress),
            f32::interpolate(&from.height, &to.height, progress),
        )
    }
}

impl Interpolate for Color {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        fn channel(from: u8, to: u8, progress: f32) -> u8 {
            (f32::from(from) + (f32::from(to) - f32::from(from)) * progress).round() as u8
        }

        Self::rgba8(
            channel(from.red, to.red, progress),
            channel(from.green, to.green, progress),
            channel(from.blue, to.blue, progress),
            channel(from.alpha, to.alpha, progress),
        )
    }
}

/// A cheaply cloneable presentation value updated only by a [`Timeline`].
///
/// The value deliberately has no connection to [`crate::state::State`]. A
/// component may read its base State during build and animate a separate copy,
/// preserving transactional State semantics while frames are sampled.
#[derive(Clone)]
pub struct Animated<T> {
    inner: Rc<AnimatedInner<T>>,
}

struct AnimatedInner<T> {
    value: RefCell<T>,
    revision: Cell<u64>,
}

impl<T> Animated<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(AnimatedInner {
                value: RefCell::new(value),
                revision: Cell::new(0),
            }),
        }
    }

    pub fn value(&self) -> T
    where
        T: Clone,
    {
        self.inner.value.borrow().clone()
    }

    /// Increases only when the sampled presentation value actually changes.
    pub fn revision(&self) -> u64 {
        self.inner.revision.get()
    }

    fn replace(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        let mut current = self.inner.value.borrow_mut();
        if *current == value {
            return false;
        }
        *current = value;
        self.inner
            .revision
            .set(self.inner.revision.get().saturating_add(1));
        true
    }
}

impl<T: fmt::Debug> fmt::Debug for Animated<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Animated")
            .field("value", &*self.inner.value.borrow())
            .field("revision", &self.revision())
            .finish()
    }
}

/// Curve applied to normalized elapsed time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    pub fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseIn => progress * progress,
            Self::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
            Self::EaseInOut if progress < 0.5 => 2.0 * progress * progress,
            Self::EaseInOut => 1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0,
        }
    }
}

/// How a new track handles an existing animation with the same key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationConflictPolicy {
    /// Replace the old track and use the new request's explicit start value.
    #[default]
    Replace,
    /// Sample the old track now and continue smoothly from that presentation value.
    Continue,
    /// Cancel the old track, then start the request from its explicit start value.
    Cancel,
}

/// Immutable parameters of an animation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationSpec {
    duration: Duration,
    impact: AnimationImpact,
    easing: Easing,
    conflict: AnimationConflictPolicy,
}

impl AnimationSpec {
    pub const fn new(duration: Duration, impact: AnimationImpact) -> Self {
        Self {
            duration,
            impact,
            easing: Easing::Linear,
            conflict: AnimationConflictPolicy::Replace,
        }
    }

    pub const fn duration(self) -> Duration {
        self.duration
    }

    pub const fn impact(self) -> AnimationImpact {
        self.impact
    }

    pub const fn easing(self) -> Easing {
        self.easing
    }

    pub const fn conflict(self) -> AnimationConflictPolicy {
        self.conflict
    }

    pub const fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub const fn with_conflict(mut self, conflict: AnimationConflictPolicy) -> Self {
        self.conflict = conflict;
        self
    }
}

/// Current or terminal state observed through an [`AnimationHandle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
    Replaced,
}

/// Stable token for controlling one particular animation request.
#[derive(Clone)]
pub struct AnimationHandle {
    inner: Rc<HandleInner>,
}

struct HandleInner {
    timeline_id: u64,
    id: AnimationId,
    key: AnimationKey,
    status: Cell<AnimationStatus>,
}

impl AnimationHandle {
    pub fn id(&self) -> AnimationId {
        self.inner.id
    }

    pub fn key(&self) -> AnimationKey {
        self.inner.key
    }

    pub fn status(&self) -> AnimationStatus {
        self.inner.status.get()
    }

    pub fn is_running(&self) -> bool {
        self.status() == AnimationStatus::Running
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            self.status(),
            AnimationStatus::Completed | AnimationStatus::Cancelled | AnimationStatus::Replaced
        )
    }
}

impl fmt::Debug for AnimationHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnimationHandle")
            .field("timeline_id", &self.inner.timeline_id)
            .field("id", &self.id())
            .field("key", &self.key())
            .field("status", &self.status())
            .finish()
    }
}

impl PartialEq for AnimationHandle {
    fn eq(&self, other: &Self) -> bool {
        self.inner.timeline_id == other.inner.timeline_id && self.id() == other.id()
    }
}

impl Eq for AnimationHandle {}

impl Hash for AnimationHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.timeline_id.hash(state);
        self.id().hash(state);
    }
}

/// Observable result of sampling the unified timeline once.
#[derive(Clone, Debug)]
pub struct AnimationFrame {
    timestamp: Duration,
    invalidations: Vec<AnimationInvalidation>,
    completed: Vec<AnimationHandle>,
    cancelled: Vec<AnimationHandle>,
    sampled: usize,
    active: usize,
    needs_next_frame: bool,
}

impl AnimationFrame {
    pub const fn timestamp(&self) -> Duration {
        self.timestamp
    }

    pub fn invalidations(&self) -> &[AnimationInvalidation] {
        &self.invalidations
    }

    pub fn completed(&self) -> &[AnimationHandle] {
        &self.completed
    }

    pub fn cancelled(&self) -> &[AnimationHandle] {
        &self.cancelled
    }

    pub const fn sampled(&self) -> usize {
        self.sampled
    }

    /// Includes running and paused tracks that are still owned by the timeline.
    pub const fn active(&self) -> usize {
        self.active
    }

    /// The event loop should request another frame only when this is true.
    pub const fn needs_next_frame(&self) -> bool {
        self.needs_next_frame
    }

    pub fn is_idle(&self) -> bool {
        !self.needs_next_frame
            && self.invalidations.is_empty()
            && self.completed.is_empty()
            && self.cancelled.is_empty()
    }
}

struct TrackSample {
    changed: bool,
    completed: bool,
}

trait AnimationTrack {
    fn sample(&mut self, now: Duration) -> TrackSample;
    fn pause(&mut self, now: Duration) -> TrackSample;
    fn resume(&mut self, now: Duration);
    fn finish(&mut self) -> bool;
    fn clone_presentation(&self) -> Box<dyn Any>;
    fn take_completion(&mut self) -> Option<Box<dyn FnOnce()>>;
}

struct TypedTrack<T> {
    value: Animated<T>,
    from: T,
    to: T,
    started_at: Duration,
    duration: Duration,
    easing: Easing,
    paused_at: Option<Duration>,
    completion: Option<Box<dyn FnOnce()>>,
}

impl<T: Interpolate> TypedTrack<T> {
    fn progress(&self, now: Duration) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let effective_now = self.paused_at.unwrap_or(now);
        let elapsed = effective_now.saturating_sub(self.started_at);
        (elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0) as f32
    }
}

impl<T: Interpolate> AnimationTrack for TypedTrack<T> {
    fn sample(&mut self, now: Duration) -> TrackSample {
        let progress = self.progress(now);
        let value = T::interpolate(&self.from, &self.to, self.easing.sample(progress));
        TrackSample {
            changed: self.value.replace(value),
            completed: progress >= 1.0,
        }
    }

    fn pause(&mut self, now: Duration) -> TrackSample {
        let sample = self.sample(now);
        if !sample.completed {
            self.paused_at = Some(now);
        }
        sample
    }

    fn resume(&mut self, now: Duration) {
        if let Some(paused_at) = self.paused_at.take() {
            self.started_at = self
                .started_at
                .checked_add(now.saturating_sub(paused_at))
                .unwrap_or(Duration::MAX);
        }
    }

    fn finish(&mut self) -> bool {
        self.value.replace(self.to.clone())
    }

    fn clone_presentation(&self) -> Box<dyn Any> {
        Box::new(self.value.value())
    }

    fn take_completion(&mut self) -> Option<Box<dyn FnOnce()>> {
        self.completion.take()
    }
}

struct TimelineEntry {
    handle: AnimationHandle,
    impact: AnimationImpact,
    track: Box<dyn AnimationTrack>,
}

/// UI-owned scheduler for all active property animations.
pub struct Timeline {
    timeline_id: u64,
    clock: Rc<dyn FrameClock>,
    entries: BTreeMap<AnimationKey, TimelineEntry>,
    completed_presentations: BTreeMap<AnimationKey, Box<dyn Any>>,
    completed_impacts: BTreeMap<AnimationKey, AnimationImpact>,
    pending_invalidations: BTreeMap<AnimationKey, AnimationImpact>,
    pending_completed: Vec<AnimationHandle>,
    pending_cancelled: Vec<AnimationHandle>,
    pending_callbacks: Vec<Box<dyn FnOnce()>>,
    reduced_motion: bool,
    last_observed: Duration,
    next_slot: u32,
    next_generation: u32,
}

impl Timeline {
    pub fn new(clock: Rc<dyn FrameClock>) -> Self {
        let now = clock.now();
        let timeline_id = NEXT_TIMELINE_ID.with(|next| {
            let timeline_id = next.get();
            next.set(timeline_id.saturating_add(1));
            timeline_id
        });
        Self {
            timeline_id,
            clock,
            entries: BTreeMap::new(),
            completed_presentations: BTreeMap::new(),
            completed_impacts: BTreeMap::new(),
            pending_invalidations: BTreeMap::new(),
            pending_completed: Vec::new(),
            pending_cancelled: Vec::new(),
            pending_callbacks: Vec::new(),
            reduced_motion: false,
            last_observed: now,
            next_slot: 0,
            next_generation: 1,
        }
    }

    pub fn from_clock(clock: impl FrameClock + 'static) -> Self {
        Self::new(Rc::new(clock))
    }

    pub fn clock(&self) -> &dyn FrameClock {
        self.clock.as_ref()
    }

    pub fn animate<T: Interpolate>(
        &mut self,
        key: impl Into<AnimationKey>,
        value: &Animated<T>,
        to: T,
        spec: AnimationSpec,
    ) -> AnimationHandle {
        let from = value.value();
        self.animate_between(key, value, from, to, spec)
    }

    pub fn animate_with_completion<T: Interpolate>(
        &mut self,
        key: impl Into<AnimationKey>,
        value: &Animated<T>,
        to: T,
        spec: AnimationSpec,
        completion: impl FnOnce() + 'static,
    ) -> AnimationHandle {
        let from = value.value();
        self.animate_between_inner(
            key.into(),
            value,
            from,
            to,
            spec,
            Some(Box::new(completion)),
        )
    }

    pub fn animate_between<T: Interpolate>(
        &mut self,
        key: impl Into<AnimationKey>,
        value: &Animated<T>,
        from: T,
        to: T,
        spec: AnimationSpec,
    ) -> AnimationHandle {
        self.animate_between_inner(key.into(), value, from, to, spec, None)
    }

    pub fn animate_between_with_completion<T: Interpolate>(
        &mut self,
        key: impl Into<AnimationKey>,
        value: &Animated<T>,
        from: T,
        to: T,
        spec: AnimationSpec,
        completion: impl FnOnce() + 'static,
    ) -> AnimationHandle {
        self.animate_between_inner(
            key.into(),
            value,
            from,
            to,
            spec,
            Some(Box::new(completion)),
        )
    }

    fn animate_between_inner<T: Interpolate>(
        &mut self,
        key: AnimationKey,
        value: &Animated<T>,
        mut from: T,
        to: T,
        spec: AnimationSpec,
        completion: Option<Box<dyn FnOnce()>>,
    ) -> AnimationHandle {
        let now = self.observe_now();
        if spec.conflict == AnimationConflictPolicy::Continue && self.entries.contains_key(&key) {
            self.sample_key(key, now);
            if let Some(presentation) = self.presentation::<T>(key) {
                from = presentation;
            }
        }
        self.completed_presentations.remove(&key);
        self.completed_impacts.remove(&key);
        if let Some(old) = self.entries.remove(&key) {
            let terminal = match spec.conflict {
                AnimationConflictPolicy::Cancel => AnimationStatus::Cancelled,
                AnimationConflictPolicy::Replace | AnimationConflictPolicy::Continue => {
                    AnimationStatus::Replaced
                }
            };
            old.handle.inner.status.set(terminal);
            self.pending_cancelled.push(old.handle);
        }

        let handle = self.allocate_handle(key);
        if value.replace(from.clone()) {
            self.queue_invalidation(key, spec.impact);
        }

        let mut track = TypedTrack {
            value: value.clone(),
            from,
            to,
            started_at: now,
            duration: spec.duration,
            easing: spec.easing,
            paused_at: None,
            completion,
        };

        if self.reduced_motion || spec.duration.is_zero() {
            if track.finish() {
                self.queue_invalidation(key, spec.impact);
            }
            handle.inner.status.set(AnimationStatus::Completed);
            self.pending_completed.push(handle.clone());
            self.completed_presentations
                .insert(key, track.clone_presentation());
            self.completed_impacts.insert(key, spec.impact);
            if let Some(completion) = track.take_completion() {
                self.pending_callbacks.push(completion);
            }
            return handle;
        }

        self.entries.insert(
            key,
            TimelineEntry {
                handle: handle.clone(),
                impact: spec.impact,
                track: Box::new(track),
            },
        );
        handle
    }

    pub fn pause(&mut self, handle: &AnimationHandle) -> bool {
        let now = self.observe_now();
        let key = handle.key();
        let Some(entry) = self.entries.get_mut(&key) else {
            return false;
        };
        if handle.inner.timeline_id != self.timeline_id
            || entry.handle.id() != handle.id()
            || entry.handle.status() != AnimationStatus::Running
        {
            return false;
        }
        let sample = entry.track.pause(now);
        let impact = entry.impact;
        if sample.changed {
            self.queue_invalidation(key, impact);
        }
        if sample.completed {
            self.complete_key(key);
        } else {
            handle.inner.status.set(AnimationStatus::Paused);
        }
        true
    }

    pub fn resume(&mut self, handle: &AnimationHandle) -> bool {
        let now = self.observe_now();
        let Some(entry) = self.entries.get_mut(&handle.key()) else {
            return false;
        };
        if handle.inner.timeline_id != self.timeline_id
            || entry.handle.id() != handle.id()
            || entry.handle.status() != AnimationStatus::Paused
        {
            return false;
        }
        entry.track.resume(now);
        handle.inner.status.set(AnimationStatus::Running);
        true
    }

    pub fn cancel(&mut self, handle: &AnimationHandle) -> bool {
        let key = handle.key();
        if self.entries.get(&key).is_none_or(|entry| {
            handle.inner.timeline_id != self.timeline_id || entry.handle.id() != handle.id()
        }) {
            return false;
        }
        let entry = self.entries.remove(&key).expect("entry was checked above");
        self.queue_invalidation(key, entry.impact);
        entry.handle.inner.status.set(AnimationStatus::Cancelled);
        self.pending_cancelled.push(entry.handle);
        self.completed_presentations.remove(&key);
        self.completed_impacts.remove(&key);
        true
    }

    pub fn cancel_key(&mut self, key: impl Into<AnimationKey>) -> bool {
        let key = key.into();
        let Some(entry) = self.entries.remove(&key) else {
            return false;
        };
        self.queue_invalidation(key, entry.impact);
        entry.handle.inner.status.set(AnimationStatus::Cancelled);
        self.pending_cancelled.push(entry.handle.clone());
        self.completed_presentations.remove(&entry.handle.key());
        self.completed_impacts.remove(&entry.handle.key());
        true
    }

    /// Cancels every animation owned by an element that has been unmounted.
    pub fn element_unmounted(&mut self, element: ElementId) -> usize {
        self.retain_elements(|candidate| candidate != element)
    }

    pub fn cancel_elements(&mut self, elements: impl IntoIterator<Item = ElementId>) -> usize {
        let elements = elements
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        self.retain_elements(|candidate| !elements.contains(&candidate))
    }

    /// Removes tracks whose full generational Element identity is no longer live.
    pub fn retain_elements(&mut self, mut is_live: impl FnMut(ElementId) -> bool) -> usize {
        let stale = self
            .entries
            .keys()
            .copied()
            .filter(|key| !is_live(key.element))
            .collect::<Vec<_>>();
        for key in &stale {
            if let Some(entry) = self.entries.remove(key) {
                entry.handle.inner.status.set(AnimationStatus::Cancelled);
                self.pending_cancelled.push(entry.handle);
            }
        }
        self.completed_presentations
            .retain(|key, _| is_live(key.element));
        self.completed_impacts.retain(|key, _| is_live(key.element));
        self.pending_invalidations
            .retain(|key, _| is_live(key.element));
        stale.len()
    }

    pub fn set_reduced_motion(&mut self, reduced_motion: bool) {
        if self.reduced_motion == reduced_motion {
            return;
        }
        self.reduced_motion = reduced_motion;
        if !reduced_motion {
            return;
        }

        let keys = self.entries.keys().copied().collect::<Vec<_>>();
        for key in keys {
            self.finish_key(key);
        }
    }

    pub const fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Samples one frame. Calling this while idle performs no retained work.
    pub fn tick(&mut self) -> AnimationFrame {
        let now = self.observe_now();
        let keys = self
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                (entry.handle.status() == AnimationStatus::Running).then_some(*key)
            })
            .collect::<Vec<_>>();
        let sampled = keys.len();
        for key in keys {
            self.sample_key(key, now);
        }

        AnimationFrame {
            timestamp: now,
            invalidations: std::mem::take(&mut self.pending_invalidations)
                .into_iter()
                .map(|(key, impact)| AnimationInvalidation { key, impact })
                .collect(),
            completed: std::mem::take(&mut self.pending_completed),
            cancelled: std::mem::take(&mut self.pending_cancelled),
            sampled,
            active: self.animation_count(),
            needs_next_frame: self.needs_frame(),
        }
    }

    /// Runs natural-completion callbacks after the host has applied frame work.
    /// Cancellation and replacement never enqueue a completion callback.
    pub fn dispatch_completion_callbacks(&mut self) -> usize {
        let callbacks = std::mem::take(&mut self.pending_callbacks);
        let count = callbacks.len();
        for callback in callbacks {
            callback();
        }
        count
    }

    pub fn pending_completion_callbacks(&self) -> usize {
        self.pending_callbacks.len()
    }

    /// Returns the active or naturally completed presentation value for a key.
    ///
    /// Natural completion retains the final overlay until it is explicitly
    /// cleared, replaced, or its Element is unmounted. Cancellation clears it.
    pub fn presentation<T: Clone + 'static>(&self, key: impl Into<AnimationKey>) -> Option<T> {
        let key = key.into();
        if let Some(entry) = self.entries.get(&key) {
            return entry
                .track
                .clone_presentation()
                .downcast::<T>()
                .ok()
                .map(|v| *v);
        }
        self.completed_presentations
            .get(&key)
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }

    pub fn clear_presentation(&mut self, key: impl Into<AnimationKey>) -> bool {
        let key = key.into();
        let removed = self.completed_presentations.remove(&key).is_some();
        if removed {
            if let Some(impact) = self.completed_impacts.remove(&key) {
                self.queue_invalidation(key, impact);
            }
        }
        removed
    }

    pub fn needs_frame(&self) -> bool {
        self.entries
            .values()
            .any(|entry| entry.handle.status() == AnimationStatus::Running)
    }

    pub fn is_idle(&self) -> bool {
        !self.needs_frame()
            && self.pending_invalidations.is_empty()
            && self.pending_callbacks.is_empty()
    }

    pub fn animation_count(&self) -> usize {
        self.entries.len()
    }

    pub fn running_animation_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.handle.status() == AnimationStatus::Running)
            .count()
    }

    pub fn contains(&self, handle: &AnimationHandle) -> bool {
        handle.inner.timeline_id == self.timeline_id
            && self
                .entries
                .get(&handle.key())
                .is_some_and(|entry| entry.handle.id() == handle.id())
    }

    pub fn handle_for_key(&self, key: impl Into<AnimationKey>) -> Option<AnimationHandle> {
        self.entries
            .get(&key.into())
            .map(|entry| entry.handle.clone())
    }

    fn sample_key(&mut self, key: AnimationKey, now: Duration) {
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        let sample = entry.track.sample(now);
        let impact = entry.impact;
        if sample.changed {
            self.queue_invalidation(key, impact);
        }
        if sample.completed {
            self.complete_key(key);
        }
    }

    fn complete_key(&mut self, key: AnimationKey) {
        let Some(mut entry) = self.entries.remove(&key) else {
            return;
        };
        entry.handle.inner.status.set(AnimationStatus::Completed);
        self.pending_completed.push(entry.handle.clone());
        self.completed_presentations
            .insert(key, entry.track.clone_presentation());
        self.completed_impacts.insert(key, entry.impact);
        if let Some(completion) = entry.track.take_completion() {
            self.pending_callbacks.push(completion);
        }
    }

    fn finish_key(&mut self, key: AnimationKey) {
        let Some(mut entry) = self.entries.remove(&key) else {
            return;
        };
        if entry.track.finish() {
            self.queue_invalidation(key, entry.impact);
        }
        entry.handle.inner.status.set(AnimationStatus::Completed);
        self.pending_completed.push(entry.handle.clone());
        self.completed_presentations
            .insert(key, entry.track.clone_presentation());
        self.completed_impacts.insert(key, entry.impact);
        if let Some(completion) = entry.track.take_completion() {
            self.pending_callbacks.push(completion);
        }
    }

    fn queue_invalidation(&mut self, key: AnimationKey, impact: AnimationImpact) {
        self.pending_invalidations
            .entry(key)
            .and_modify(|existing| {
                if impact == AnimationImpact::Layout {
                    *existing = AnimationImpact::Layout;
                }
            })
            .or_insert(impact);
    }

    fn observe_now(&mut self) -> Duration {
        let observed = self.clock.now();
        self.last_observed = self.last_observed.max(observed);
        self.last_observed
    }

    fn allocate_handle(&mut self, key: AnimationKey) -> AnimationHandle {
        let id = AnimationId::from_parts(self.next_slot, self.next_generation);
        if self.next_slot == u32::MAX {
            self.next_slot = 0;
            self.next_generation = self.next_generation.saturating_add(1).max(1);
        } else {
            self.next_slot += 1;
        }
        AnimationHandle {
            inner: Rc::new(HandleInner {
                timeline_id: self.timeline_id,
                id,
                key,
                status: Cell::new(AnimationStatus::Running),
            }),
        }
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new(Rc::new(SystemClock::new()))
    }
}

impl fmt::Debug for Timeline {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Timeline")
            .field("timeline_id", &self.timeline_id)
            .field("animation_count", &self.animation_count())
            .field("running_animation_count", &self.running_animation_count())
            .field("reduced_motion", &self.reduced_motion)
            .field("last_observed", &self.last_observed)
            .finish_non_exhaustive()
    }
}
