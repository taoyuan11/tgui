use std::cell::RefCell;

#[derive(Clone, Copy)]
pub struct Signal<T: 'static> {
    inner: &'static RefCell<T>,
}

pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
    let leaked = Box::leak(Box::new(RefCell::new(value)));
    Signal { inner: leaked }
}

impl<T: 'static> Signal<T> {
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut self.inner.borrow_mut());
    }
}

impl<T: Clone + 'static> Signal<T> {
    pub fn get(&self) -> T {
        self.inner.borrow().clone()
    }
}
