use std::sync::Arc;

use crate::dialog::Dialogs;
use crate::log::Log;

/// Marker trait for types that can back a `tgui` application.
///
/// Implement this trait explicitly on your root application view model.
pub trait ViewModel: Send + 'static {}

pub struct CommandContext<T> {
    dialogs: Dialogs<T>,
    log: Log,
}

impl<T> Clone for CommandContext<T> {
    fn clone(&self) -> Self {
        Self {
            dialogs: self.dialogs.clone(),
            log: self.log.clone(),
        }
    }
}

impl<T: 'static> CommandContext<T> {
    pub fn dialogs(&self) -> Dialogs<T> {
        self.dialogs.clone()
    }

    pub fn log(&self) -> Log {
        self.log.clone()
    }

    pub(crate) fn new(dialogs: Dialogs<T>, log: Log) -> Self {
        Self { dialogs, log }
    }

    pub(crate) fn detached() -> Self {
        Self::new(Dialogs::detached(), Log::default())
    }

    pub(crate) fn scope<ChildVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut T) -> &'a mut ChildVm + Send + Sync>,
    ) -> CommandContext<ChildVm> {
        CommandContext::new(self.dialogs.scope(selector), self.log.clone())
    }
}

type CommandHandler<T> = dyn Fn(&mut T) + Send + Sync;
type ContextCommandHandler<T> = dyn Fn(&mut T, &CommandContext<T>) + Send + Sync;
type ValueCommandHandler<T, V> = dyn Fn(&mut T, V) + Send + Sync;
type ContextValueCommandHandler<T, V> = dyn Fn(&mut T, V, &CommandContext<T>) + Send + Sync;

enum CommandKind<T> {
    Plain(Arc<CommandHandler<T>>),
    WithContext(Arc<ContextCommandHandler<T>>),
}

/// Command that mutates a view model without an event payload.
pub struct Command<T> {
    handler: CommandKind<T>,
}

impl<T> Clone for Command<T> {
    fn clone(&self) -> Self {
        Self {
            handler: match &self.handler {
                CommandKind::Plain(handler) => CommandKind::Plain(handler.clone()),
                CommandKind::WithContext(handler) => CommandKind::WithContext(handler.clone()),
            },
        }
    }
}

impl<T: 'static> Command<T> {
    /// Creates a command from a closure or method reference.
    pub fn new(handler: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        Self {
            handler: CommandKind::Plain(Arc::new(handler)),
        }
    }

    /// Creates a command that can access runtime services such as dialogs and logging.
    pub fn new_with_context(
        handler: impl Fn(&mut T, &CommandContext<T>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            handler: CommandKind::WithContext(Arc::new(handler)),
        }
    }

    /// Executes the command against the given view model instance.
    pub fn execute(&self, view_model: &mut T) {
        let context = CommandContext::detached();
        self.execute_with_context(view_model, &context);
    }

    /// Executes the command against the given view model instance with access
    /// to runtime-scoped services.
    pub fn execute_with_context(&self, view_model: &mut T, context: &CommandContext<T>) {
        match &self.handler {
            CommandKind::Plain(handler) => handler(view_model),
            CommandKind::WithContext(handler) => handler(view_model, context),
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut T + Send + Sync>,
    ) -> Command<RootVm> {
        Command::new_with_context(move |view_model, context| {
            let scoped_context = context.scope(selector.clone());
            self.execute_with_context(selector(view_model), &scoped_context);
        })
    }
}

enum ValueCommandKind<T, V> {
    Plain(Arc<ValueCommandHandler<T, V>>),
    WithContext(Arc<ContextValueCommandHandler<T, V>>),
}

/// Command that mutates a view model with an event payload.
pub struct ValueCommand<T, V> {
    handler: ValueCommandKind<T, V>,
}

impl<T, V> Clone for ValueCommand<T, V> {
    fn clone(&self) -> Self {
        Self {
            handler: match &self.handler {
                ValueCommandKind::Plain(handler) => ValueCommandKind::Plain(handler.clone()),
                ValueCommandKind::WithContext(handler) => {
                    ValueCommandKind::WithContext(handler.clone())
                }
            },
        }
    }
}

impl<T: 'static, V> ValueCommand<T, V> {
    /// Creates a payload-carrying command from a closure or method reference.
    pub fn new(handler: impl Fn(&mut T, V) + Send + Sync + 'static) -> Self {
        Self {
            handler: ValueCommandKind::Plain(Arc::new(handler)),
        }
    }

    /// Creates a payload-carrying command with access to runtime services.
    pub fn new_with_context(
        handler: impl Fn(&mut T, V, &CommandContext<T>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            handler: ValueCommandKind::WithContext(Arc::new(handler)),
        }
    }

    /// Executes the command with the provided payload.
    pub fn execute(&self, view_model: &mut T, value: V) {
        let context = CommandContext::detached();
        self.execute_with_context(view_model, value, &context);
    }

    /// Executes the command with the provided payload and runtime services.
    pub fn execute_with_context(&self, view_model: &mut T, value: V, context: &CommandContext<T>) {
        match &self.handler {
            ValueCommandKind::Plain(handler) => handler(view_model, value),
            ValueCommandKind::WithContext(handler) => handler(view_model, value, context),
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut T + Send + Sync>,
    ) -> ValueCommand<RootVm, V>
    where
        V: 'static,
    {
        ValueCommand::new_with_context(move |view_model, value, context| {
            let scoped_context = context.scope(selector.clone());
            self.execute_with_context(selector(view_model), value, &scoped_context);
        })
    }
}
