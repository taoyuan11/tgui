use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::animation::{
    AnimatedValue, AnimationControllerBuilder, AnimationCoordinator, Transition,
};
use crate::platform::backend::event_loop::EventLoopProxy;

#[derive(Clone, Default)]
pub(crate) struct InvalidationSignal {
    revision: Arc<AtomicU64>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(1)),
            proxy: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn mark_dirty(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
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

    /// Creates an observable piece of reactive state.
    pub fn observable<T>(&self, value: T) -> Observable<T> {
        Observable::new(value, self.invalidation.clone())
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
        }
    }

    pub(crate) fn new_legacy(initial_text: impl Into<crate::ui::layout::Value<String>>) -> Self {
        Self::new(initial_text.into().resolve(), InvalidationSignal::new())
    }

    pub fn text(&self) -> String {
        self.state
            .lock()
            .expect("text controller lock poisoned")
            .text
            .clone()
    }

    pub fn snapshot(&self) -> TextSnapshot {
        let state = self.state.lock().expect("text controller lock poisoned");
        TextSnapshot {
            text: state.text.clone(),
            revision: state.revision,
        }
    }

    pub fn revision(&self) -> u64 {
        self.state
            .lock()
            .expect("text controller lock poisoned")
            .revision
    }

    pub fn set_text(&self, text: impl Into<String>) {
        if self.set_text_silent(text) {
            self.invalidation.mark_dirty();
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

impl From<Binding<String>> for TextController {
    fn from(value: Binding<String>) -> Self {
        TextController::new_legacy(crate::ui::layout::Value::Bound(value))
    }
}

impl From<crate::ui::layout::Value<String>> for TextController {
    fn from(value: crate::ui::layout::Value<String>) -> Self {
        TextController::new_legacy(value)
    }
}

#[derive(Clone)]
/// Shared mutable state that marks the UI dirty whenever it changes.
///
/// Create it through [`ViewModelContext::observable`], then derive UI-facing
/// values using [`Observable::binding`].
pub struct Observable<T> {
    value: Arc<Mutex<T>>,
    invalidation: InvalidationSignal,
}

impl<T> Observable<T> {
    fn new(value: T, invalidation: InvalidationSignal) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            invalidation,
        }
    }

    /// Replaces the current value and requests a UI refresh.
    pub fn set(&self, value: T) {
        *self.value.lock().expect("observable lock poisoned") = value;
        self.invalidation.mark_dirty();
    }

    /// Mutates the current value in place and requests a UI refresh.
    pub fn update<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.lock().expect("observable lock poisoned");
        let result = updater(&mut value);
        self.invalidation.mark_dirty();
        result
    }

    /// Creates a binding that reads the current observable value on demand.
    pub fn binding(&self) -> Binding<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let observable = self.clone();
        Binding::new(move || observable.get())
    }
}

impl<T: Clone> Observable<T> {
    /// Returns a cloned snapshot of the current value.
    pub fn get(&self) -> T {
        self.value.lock().expect("observable lock poisoned").clone()
    }
}

#[derive(Clone)]
/// Lazily evaluated value used by widgets and window bindings.
///
/// A binding can be derived from an [`Observable`] or created from any closure.
/// Use [`Binding::map`] to derive more values and [`Binding::animated`] to attach
/// a declarative transition.
pub struct Binding<T> {
    reader: Arc<dyn Fn() -> T + Send + Sync>,
    transition: Option<Transition>,
}

impl<T> Binding<T> {
    /// Creates a binding from a reader closure.
    pub fn new(reader: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            reader: Arc::new(reader),
            transition: None,
        }
    }

    /// Reads the current value of the binding.
    pub fn get(&self) -> T {
        (self.reader)()
    }

    /// Marks the binding as animatable when consumed by a supported UI property.
    pub fn animated(mut self, transition: impl Into<Transition>) -> Self {
        self.transition = Some(transition.into());
        self
    }

    /// Derives a new binding from the current one.
    pub fn map<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Binding<U>
    where
        T: 'static,
    {
        let reader = self.reader.clone();
        Binding {
            reader: Arc::new(move || mapper(reader())),
            transition: self.transition,
        }
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        self.transition
    }
}
