use std::sync::Arc;

/// Marker trait for types that can back a `tgui` application.
///
/// Most user-defined structs automatically satisfy this trait because every
/// `Send + 'static` type implements it.
pub trait ViewModel: Send + 'static {}

impl<T> ViewModel for T where T: Send + 'static {}

type CommandHandler<T> = dyn Fn(&mut T) + Send + Sync;
type ValueCommandHandler<T, V> = dyn Fn(&mut T, V) + Send + Sync;

/// Command that mutates a view model without an event payload.
pub struct Command<T> {
    handler: Arc<CommandHandler<T>>,
}

impl<T> Clone for Command<T> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

impl<T> Command<T> {
    /// Creates a command from a closure or method reference.
    pub fn new(handler: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Executes the command against the given view model instance.
    pub fn execute(&self, view_model: &mut T) {
        (self.handler)(view_model);
    }
}

/// Command that mutates a view model with an event payload.
pub struct ValueCommand<T, V> {
    handler: Arc<ValueCommandHandler<T, V>>,
}

impl<T, V> Clone for ValueCommand<T, V> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

impl<T, V> ValueCommand<T, V> {
    /// Creates a payload-carrying command from a closure or method reference.
    pub fn new(handler: impl Fn(&mut T, V) + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Executes the command with the provided payload.
    pub fn execute(&self, view_model: &mut T, value: V) {
        (self.handler)(view_model, value);
    }
}
