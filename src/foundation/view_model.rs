use std::sync::Arc;

pub trait ViewModel: Send + 'static {}

impl<T> ViewModel for T where T: Send + 'static {}

pub struct Command<T> {
    handler: Arc<dyn Fn(&mut T) + Send + Sync>,
}

impl<T> Clone for Command<T> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

impl<T> Command<T> {
    pub fn new(handler: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub fn execute(&self, view_model: &mut T) {
        (self.handler)(view_model);
    }
}

pub struct ValueCommand<T, V> {
    handler: Arc<dyn Fn(&mut T, V) + Send + Sync>,
}

impl<T, V> Clone for ValueCommand<T, V> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

impl<T, V> ValueCommand<T, V> {
    pub fn new(handler: impl Fn(&mut T, V) + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub fn execute(&self, view_model: &mut T, value: V) {
        (self.handler)(view_model, value);
    }
}
