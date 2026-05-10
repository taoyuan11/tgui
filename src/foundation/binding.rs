use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::animation::{
    AnimatedValue, AnimationControllerBuilder, AnimationCoordinator, Transition,
};
use crate::platform::backend::event_loop::EventLoopProxy;

static NEXT_DEPENDENCY_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyId(u64);

impl DependencyId {
    pub(crate) fn next() -> Self {
        Self(NEXT_DEPENDENCY_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DependencyPhase {
    Structure,
    Layout,
    Scene,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyOwner {
    pub(crate) widget_id: u64,
    pub(crate) phase: DependencyPhase,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DependencyGraph {
    dependencies: HashMap<DependencyId, HashSet<DependencyOwner>>,
    has_global_dependency: bool,
}

impl DependencyGraph {
    pub(crate) fn owners_for(&self, dependency: DependencyId) -> Option<&HashSet<DependencyOwner>> {
        self.dependencies.get(&dependency)
    }

    pub(crate) fn has_global_dependency(&self) -> bool {
        self.has_global_dependency
    }

    #[cfg(test)]
    pub(crate) fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    #[cfg(test)]
    pub(crate) fn contains_owner(&self, owner: DependencyOwner) -> bool {
        self.dependencies
            .values()
            .any(|owners| owners.contains(&owner))
    }

    pub(crate) fn merge_from(&mut self, other: &DependencyGraph) {
        self.has_global_dependency |= other.has_global_dependency;
        for (dependency, owners) in &other.dependencies {
            self.dependencies
                .entry(*dependency)
                .or_default()
                .extend(owners.iter().copied());
        }
    }

    pub(crate) fn remove_widget_owners(&mut self, widget_ids: &HashSet<u64>) {
        if widget_ids.is_empty() {
            return;
        }
        self.dependencies.retain(|_, owners| {
            owners.retain(|owner| !widget_ids.contains(&owner.widget_id));
            !owners.is_empty()
        });
        if self.dependencies.is_empty() {
            self.has_global_dependency = false;
        }
    }

    pub(crate) fn remove_widget_phase_owners(
        &mut self,
        widget_ids: &HashSet<u64>,
        phase: DependencyPhase,
    ) {
        if widget_ids.is_empty() {
            return;
        }
        self.dependencies.retain(|_, owners| {
            owners.retain(|owner| !(owner.phase == phase && widget_ids.contains(&owner.widget_id)));
            !owners.is_empty()
        });
        if self.dependencies.is_empty() {
            self.has_global_dependency = false;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirtyDependencySet {
    Clean,
    Global,
    Dependencies {
        from_revision: u64,
        to_revision: u64,
    },
}

#[derive(Debug)]
struct DirtyDependencyEntry {
    revision: u64,
    dependency: Option<DependencyId>,
}

#[derive(Debug, Default)]
struct DirtyDependencyLog {
    entries: Vec<DirtyDependencyEntry>,
}

const MAX_DIRTY_DEPENDENCY_ENTRIES: usize = 1024;

impl DirtyDependencyLog {
    fn push(&mut self, revision: u64, dependency: Option<DependencyId>) {
        self.entries.push(DirtyDependencyEntry {
            revision,
            dependency,
        });
        if self.entries.len() > MAX_DIRTY_DEPENDENCY_ENTRIES {
            let overflow = self.entries.len() - MAX_DIRTY_DEPENDENCY_ENTRIES;
            self.entries.drain(0..overflow);
        }
    }

    fn dirty_since(
        &self,
        revision: u64,
        current_revision: u64,
    ) -> (DirtyDependencySet, HashSet<DependencyId>) {
        if revision == current_revision {
            return (DirtyDependencySet::Clean, HashSet::new());
        }
        let Some(first) = self.entries.first() else {
            return (DirtyDependencySet::Global, HashSet::new());
        };
        if revision != 0 && first.revision > revision.saturating_add(1) {
            return (DirtyDependencySet::Global, HashSet::new());
        }

        let mut dependencies = HashSet::new();
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.revision > revision)
        {
            let Some(dependency) = entry.dependency else {
                return (DirtyDependencySet::Global, HashSet::new());
            };
            dependencies.insert(dependency);
        }

        (
            DirtyDependencySet::Dependencies {
                from_revision: revision,
                to_revision: current_revision,
            },
            dependencies,
        )
    }
}

#[derive(Default)]
struct DependencyTracker {
    scopes: Vec<DependencyOwner>,
    records: Vec<(DependencyId, DependencyOwner)>,
    global_dependency: bool,
}

thread_local! {
    static DEPENDENCY_TRACKER: RefCell<DependencyTracker> = RefCell::new(DependencyTracker::default());
}

pub(crate) fn track_dependency_scope<R>(owner: DependencyOwner, f: impl FnOnce() -> R) -> R {
    struct ScopeGuard;

    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            DEPENDENCY_TRACKER.with(|tracker| {
                tracker.borrow_mut().scopes.pop();
            });
        }
    }

    DEPENDENCY_TRACKER.with(|tracker| {
        tracker.borrow_mut().scopes.push(owner);
    });
    let _guard = ScopeGuard;
    f()
}

pub(crate) fn with_dependency_collection<R>(f: impl FnOnce() -> R) -> (R, DependencyGraph) {
    DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        tracker.records.clear();
        tracker.global_dependency = false;
    });

    let result = f();
    let graph = DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        let mut graph = DependencyGraph::default();
        graph.has_global_dependency = tracker.global_dependency;
        for (dependency, owner) in tracker.records.drain(..) {
            graph
                .dependencies
                .entry(dependency)
                .or_default()
                .insert(owner);
        }
        graph
    });
    (result, graph)
}

pub(crate) fn record_dependency_read(dependency: Option<DependencyId>) {
    DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        let Some(owner) = tracker.scopes.last().copied() else {
            return;
        };
        if let Some(dependency) = dependency {
            tracker.records.push((dependency, owner));
        } else {
            tracker.global_dependency = true;
        }
    });
}

#[derive(Clone, Default)]
pub(crate) struct InvalidationSignal {
    revision: Arc<AtomicU64>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
    dirty_dependencies: Arc<Mutex<DirtyDependencyLog>>,
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(1)),
            proxy: Arc::new(Mutex::new(None)),
            dirty_dependencies: Arc::new(Mutex::new(DirtyDependencyLog::default())),
        }
    }

    pub(crate) fn mark_dirty(&self) {
        self.mark_dirty_dependency(None);
    }

    pub(crate) fn mark_dependency_dirty(&self, dependency: DependencyId) {
        self.mark_dirty_dependency(Some(dependency));
    }

    fn mark_dirty_dependency(&self, dependency: Option<DependencyId>) {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        self.dirty_dependencies
            .lock()
            .expect("dirty dependency log lock poisoned")
            .push(revision, dependency);
        if let Some(proxy) = self
            .proxy
            .lock()
            .expect("invalidation proxy lock poisoned")
            .as_ref()
            .cloned()
        {
            proxy.wake_up();
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock().expect("invalidation proxy lock poisoned") = Some(proxy);
    }

    pub(crate) fn dirty_dependencies_since(
        &self,
        revision: u64,
    ) -> (DirtyDependencySet, HashSet<DependencyId>) {
        let current_revision = self.revision();
        self.dirty_dependencies
            .lock()
            .expect("dirty dependency log lock poisoned")
            .dirty_since(revision, current_revision)
    }
}

#[derive(Clone)]
/// Factory object passed into the view-model constructor.
///
/// It provides access to state primitives that automatically invalidate the UI
/// when their values change.
pub struct ViewModelContext {
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
}

impl ViewModelContext {
    pub(crate) fn new(invalidation: InvalidationSignal, animations: AnimationCoordinator) -> Self {
        Self {
            invalidation,
            animations,
        }
    }

    /// Creates a writable piece of reactive state.
    pub fn state<T>(&self, value: T) -> State<T> {
        State::new(value, self.invalidation.clone())
    }

    /// Creates a cached read-only signal from a reader closure.
    pub fn signal<T>(&self, reader: impl Fn() -> T + Send + Sync + 'static) -> Signal<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        Signal::new(reader, self.invalidation.clone())
    }

    /// Creates an animatable value for imperative timeline-driven animation.
    pub fn animated_value<T>(&self, value: T) -> AnimatedValue<T> {
        AnimatedValue::new(value, self.invalidation.clone())
    }

    /// Starts building a timeline controller that can drive one or more animated values.
    pub fn timeline(&self) -> AnimationControllerBuilder {
        AnimationControllerBuilder::new(self.animations.clone(), self.invalidation.clone())
    }

    /// Creates a retained text controller for `Input` and `Textarea`.
    pub fn text_controller(&self, initial_text: impl Into<String>) -> TextController {
        TextController::new(initial_text, self.invalidation.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    pub text: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChange {
    pub range_bytes: (usize, usize),
    pub inserted_text: String,
}

impl TextChange {
    pub fn new(range_bytes: (usize, usize), inserted_text: impl Into<String>) -> Self {
        Self {
            range_bytes,
            inserted_text: inserted_text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChangeSet {
    pub start_revision: u64,
    pub end_revision: u64,
    pub changes: Vec<TextChange>,
}

#[derive(Clone)]
pub struct TextController {
    state: Arc<Mutex<TextControllerState>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
}

#[derive(Debug)]
struct TextControllerState {
    text: String,
    revision: u64,
}

impl TextController {
    fn new(initial_text: impl Into<String>, invalidation: InvalidationSignal) -> Self {
        Self {
            state: Arc::new(Mutex::new(TextControllerState {
                text: initial_text.into(),
                revision: 1,
            })),
            invalidation,
            dependency: DependencyId::next(),
        }
    }

    pub(crate) fn new_legacy(initial_text: impl Into<crate::ui::layout::Value<String>>) -> Self {
        Self::new(initial_text.into().resolve(), InvalidationSignal::new())
    }

    pub fn text(&self) -> String {
        record_dependency_read(Some(self.dependency));
        self.state
            .lock()
            .expect("text controller lock poisoned")
            .text
            .clone()
    }

    pub fn snapshot(&self) -> TextSnapshot {
        record_dependency_read(Some(self.dependency));
        let state = self.state.lock().expect("text controller lock poisoned");
        TextSnapshot {
            text: state.text.clone(),
            revision: state.revision,
        }
    }

    pub fn revision(&self) -> u64 {
        record_dependency_read(Some(self.dependency));
        self.state
            .lock()
            .expect("text controller lock poisoned")
            .revision
    }

    pub fn set_text(&self, text: impl Into<String>) {
        if self.set_text_silent(text) {
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
    }

    pub fn replace_all(&self, text: impl Into<String>) {
        self.set_text(text);
    }

    pub(crate) fn replace_text_silent(&self, text: impl Into<String>) -> u64 {
        let text = text.into();
        let mut state = self.state.lock().expect("text controller lock poisoned");
        if state.text != text {
            state.text = text;
            state.revision = state.revision.wrapping_add(1).max(1);
        }
        state.revision
    }

    pub(crate) fn set_text_silent(&self, text: impl Into<String>) -> bool {
        let previous = self.revision();
        let next = self.replace_text_silent(text);
        next != previous
    }
}

impl From<String> for TextController {
    fn from(value: String) -> Self {
        TextController::new_legacy(value)
    }
}

impl From<&str> for TextController {
    fn from(value: &str) -> Self {
        TextController::new_legacy(value)
    }
}

impl From<Signal<String>> for TextController {
    fn from(value: Signal<String>) -> Self {
        TextController::new_legacy(crate::ui::layout::Value::Signal(value))
    }
}

impl From<crate::ui::layout::Value<String>> for TextController {
    fn from(value: crate::ui::layout::Value<String>) -> Self {
        TextController::new_legacy(value)
    }
}

#[derive(Clone)]
/// Shared mutable state that marks the UI dirty whenever its value changes.
///
/// Create it through [`ViewModelContext::state`], then derive UI-facing values
/// using [`State::signal`].
pub struct State<T> {
    value: Arc<Mutex<T>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
}

impl<T> State<T> {
    fn new(value: T, invalidation: InvalidationSignal) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            invalidation,
            dependency: DependencyId::next(),
        }
    }

    /// Reads the current value without cloning it.
    pub fn read<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        record_dependency_read(Some(self.dependency));
        let value = self.value.lock().expect("state lock poisoned");
        reader(&value)
    }

    /// Creates a cached signal that reads the current state value on demand.
    pub fn signal(&self) -> Signal<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let state = self.clone();
        Signal::new_tracked(
            move || state.get(),
            self.invalidation.clone(),
            Some(self.dependency),
        )
    }
}

impl<T: PartialEq> State<T> {
    /// Replaces the current value and requests a UI refresh only when it changed.
    pub fn set(&self, value: T) {
        let mut current = self.value.lock().expect("state lock poisoned");
        if *current == value {
            return;
        }
        *current = value;
        drop(current);
        self.invalidation.mark_dependency_dirty(self.dependency);
    }
}

impl<T: Clone> State<T> {
    /// Returns a cloned snapshot of the current value.
    pub fn get(&self) -> T {
        record_dependency_read(Some(self.dependency));
        self.value.lock().expect("state lock poisoned").clone()
    }
}

impl<T: Clone + PartialEq> State<T> {
    /// Mutates the current value in place and requests a UI refresh only when it changed.
    pub fn update<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.lock().expect("state lock poisoned");
        let previous = value.clone();
        let result = updater(&mut value);
        let changed = *value != previous;
        drop(value);
        if changed {
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
        result
    }
}

#[derive(Clone)]
/// Lazily evaluated value used by widgets and window bindings.
///
/// A signal can be derived from a [`State`] or created through
/// [`ViewModelContext::signal`]. Use [`Signal::map`] to derive more values and
/// [`Signal::animated`] to attach a declarative transition.
pub struct Signal<T> {
    reader: Arc<dyn Fn() -> T + Send + Sync>,
    invalidation: InvalidationSignal,
    cache: Arc<Mutex<SignalCache<T>>>,
    transition: Option<Transition>,
    dependency: Option<DependencyId>,
}

struct SignalCache<T> {
    revision: u64,
    value: Option<T>,
}

impl<T> Signal<T> {
    pub(crate) fn new(
        reader: impl Fn() -> T + Send + Sync + 'static,
        invalidation: InvalidationSignal,
    ) -> Self {
        Self::new_tracked(reader, invalidation, None)
    }

    pub(crate) fn new_tracked(
        reader: impl Fn() -> T + Send + Sync + 'static,
        invalidation: InvalidationSignal,
        dependency: Option<DependencyId>,
    ) -> Self {
        Self {
            reader: Arc::new(reader),
            invalidation,
            cache: Arc::new(Mutex::new(SignalCache {
                revision: 0,
                value: None,
            })),
            transition: None,
            dependency,
        }
    }

    fn with_transition(mut self, transition: Option<Transition>) -> Self {
        self.transition = transition;
        self
    }
}

impl<T: Clone> Signal<T> {
    /// Reads the current value of the signal.
    pub fn get(&self) -> T {
        record_dependency_read(self.dependency);
        let revision = self.invalidation.revision();
        {
            let cache = self.cache.lock().expect("signal cache lock poisoned");
            if cache.revision == revision {
                if let Some(value) = cache.value.as_ref() {
                    return value.clone();
                }
            }
        }

        let value = (self.reader)();
        let mut cache = self.cache.lock().expect("signal cache lock poisoned");
        cache.revision = revision;
        cache.value = Some(value.clone());
        value
    }

    /// Marks the signal as animatable when consumed by a supported UI property.
    pub fn animated(mut self, transition: impl Into<Transition>) -> Self {
        self.transition = Some(transition.into());
        self
    }

    /// Derives a cached signal from the current one.
    pub fn map<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_tracked(
            move || mapper(signal.get()),
            self.invalidation.clone(),
            self.dependency,
        )
        .with_transition(self.transition)
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        self.transition
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;

    use crate::animation::Transition;

    use super::{
        track_dependency_scope, with_dependency_collection, DependencyOwner, DependencyPhase,
        DirtyDependencySet, InvalidationSignal, Signal, State, ViewModelContext,
    };
    use crate::animation::AnimationCoordinator;

    fn context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    #[test]
    fn state_set_same_value_does_not_advance_revision() {
        let invalidation = InvalidationSignal::new();
        let state = State::new(1, invalidation.clone());
        let before = invalidation.revision();

        state.set(1);

        assert_eq!(invalidation.revision(), before);
    }

    #[test]
    fn state_update_only_invalidates_when_value_changes() {
        let invalidation = InvalidationSignal::new();
        let state = State::new(String::from("hello"), invalidation.clone());
        let before = invalidation.revision();

        state.update(|value| value.push_str(""));
        assert_eq!(invalidation.revision(), before);

        state.update(|value| value.push('!'));
        assert!(invalidation.revision() > before);
    }

    #[test]
    fn signal_get_caches_within_revision() {
        let ctx = context();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_signal = calls.clone();
        let signal = ctx.signal(move || {
            calls_for_signal.fetch_add(1, Ordering::SeqCst);
            42
        });

        assert_eq!(signal.get(), 42);
        assert_eq!(signal.get(), 42);

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn signal_recomputes_after_state_changes() {
        let ctx = context();
        let state = ctx.state(1);
        let signal = state.signal().map(|value| value * 2);

        assert_eq!(signal.get(), 2);
        state.set(4);
        assert_eq!(signal.get(), 8);
    }

    #[test]
    fn mapped_signal_preserves_transition() {
        let ctx = context();
        let state = ctx.state(1);
        let transition = Transition::linear(Duration::from_millis(10));
        let signal = state.signal().animated(transition).map(|value| value + 1);

        assert_eq!(signal.get(), 2);
        assert_eq!(signal.transition(), Some(transition));
    }

    #[test]
    fn state_change_records_specific_dirty_dependency() {
        let invalidation = InvalidationSignal::new();
        let state = State::new(1, invalidation.clone());
        let baseline = invalidation.revision();

        state.set(2);

        let (dirty, deps) = invalidation.dirty_dependencies_since(baseline);
        assert!(matches!(dirty, DirtyDependencySet::Dependencies { .. }));
        assert_eq!(deps.len(), 1);
    }

    #[test]
    fn unchanged_state_keeps_dirty_dependencies_clean() {
        let invalidation = InvalidationSignal::new();
        let state = State::new(5, invalidation.clone());
        let baseline = invalidation.revision();

        state.set(5);

        let (dirty, deps) = invalidation.dirty_dependencies_since(baseline);
        assert!(matches!(dirty, DirtyDependencySet::Clean));
        assert!(deps.is_empty());
    }

    #[test]
    fn mapped_signal_reads_are_tracked_without_global_fallback() {
        let ctx = context();
        let state = ctx.state(7);
        let mapped = state.signal().map(|value| value + 1);
        let owner = DependencyOwner {
            widget_id: 1,
            phase: DependencyPhase::Scene,
        };

        let (_, graph) =
            with_dependency_collection(|| track_dependency_scope(owner, || mapped.get()));

        assert!(!graph.has_global_dependency());
        assert_eq!(graph.dependency_count(), 1);
    }

    #[test]
    fn opaque_signal_reads_fall_back_to_global_dependency() {
        let invalidation = InvalidationSignal::new();
        let signal = Signal::new(|| 9, invalidation);
        let owner = DependencyOwner {
            widget_id: 2,
            phase: DependencyPhase::Layout,
        };

        let (_, graph) =
            with_dependency_collection(|| track_dependency_scope(owner, || signal.get()));

        assert!(graph.has_global_dependency());
    }
}
