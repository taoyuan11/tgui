use std::sync::Arc;

use super::context::CommandContext;

type CommandHandler<T> = dyn Fn(&mut T) + Send + Sync;
type ContextCommandHandler<T> = dyn Fn(&mut T, &CommandContext<T>) + Send + Sync;

enum CommandKind<T> {
    Plain(Arc<CommandHandler<T>>),
    WithContext(Arc<ContextCommandHandler<T>>),
}

/// 不携带事件负载的视图模型命令。
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
    /// 使用普通闭包或方法引用创建命令。
    ///
    /// 参数：
    /// - `handler`：命令执行时调用的处理函数。
    ///
    /// 返回值：
    /// - 返回新的 `Command<T>`。
    pub fn new(handler: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        Self {
            handler: CommandKind::Plain(Arc::new(handler)),
        }
    }

    /// 创建一个可访问运行时服务的命令。
    ///
    /// 参数：
    /// - `handler`：可访问 `CommandContext<T>` 的处理函数。
    ///
    /// 返回值：
    /// - 返回新的 `Command<T>`。
    pub fn new_with_context(
        handler: impl Fn(&mut T, &CommandContext<T>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            handler: CommandKind::WithContext(Arc::new(handler)),
        }
    }

    /// 在给定视图模型实例上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    pub fn execute(&self, view_model: &mut T) {
        let context = CommandContext::detached();
        self.execute_with_context(view_model, &context);
    }

    /// 在给定视图模型实例和运行时上下文上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `context`：运行时上下文。
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
