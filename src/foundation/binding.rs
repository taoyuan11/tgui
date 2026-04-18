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
