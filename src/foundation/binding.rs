use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::animation::Transition;

#[derive(Clone, Default)]
pub(crate) struct InvalidationSignal {
    dirty: Arc<AtomicBool>,
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            dirty: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    pub(crate) fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub struct ViewModelContext {
    invalidation: InvalidationSignal,
}

impl ViewModelContext {
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self { invalidation }
    }

    pub fn observable<T>(&self, value: T) -> Observable<T> {
        Observable::new(value, self.invalidation.clone())
    }
}

#[derive(Clone)]
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

    pub fn set(&self, value: T) {
        *self.value.lock().expect("observable lock poisoned") = value;
        self.invalidation.mark_dirty();
    }

    pub fn update<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.lock().expect("observable lock poisoned");
        let result = updater(&mut value);
        self.invalidation.mark_dirty();
        result
    }

    pub fn binding(&self) -> Binding<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let observable = self.clone();
        Binding::new(move || observable.get())
    }
}

impl<T: Clone> Observable<T> {
    pub fn get(&self) -> T {
        self.value.lock().expect("observable lock poisoned").clone()
    }
}

#[derive(Clone)]
pub struct Binding<T> {
    reader: Arc<dyn Fn() -> T + Send + Sync>,
    transition: Option<Transition>,
}

impl<T> Binding<T> {
    pub fn new(reader: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            reader: Arc::new(reader),
            transition: None,
        }
    }

    pub fn get(&self) -> T {
        (self.reader)()
    }

    pub fn animated(mut self, transition: Transition) -> Self {
        self.transition = Some(transition);
        self
    }

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
