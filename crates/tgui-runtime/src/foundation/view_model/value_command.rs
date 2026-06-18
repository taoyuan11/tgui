use std::sync::Arc;

use super::context::CommandContext;

type ValueCommandHandler<T, V> = dyn Fn(&mut T, V) + Send + Sync;
type ContextValueCommandHandler<T, V> = dyn Fn(&mut T, V, &CommandContext<T>) + Send + Sync;

enum ValueCommandKind<T, V> {
    Plain(Arc<ValueCommandHandler<T, V>>),
    WithContext(Arc<ContextValueCommandHandler<T, V>>),
}

/// 携带事件负载的视图模型命令。
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
    /// 使用普通闭包或方法引用创建带负载命令。
    ///
    /// 参数：
    /// - `handler`：命令执行时调用的处理函数。
    ///
    /// 返回值：
    /// - 返回新的 `ValueCommand<T, V>`。
    pub fn new(handler: impl Fn(&mut T, V) + Send + Sync + 'static) -> Self {
        Self {
            handler: ValueCommandKind::Plain(Arc::new(handler)),
        }
    }

    /// 创建一个可访问运行时服务的带负载命令。
    ///
    /// 参数：
    /// - `handler`：可访问 `CommandContext<T>` 的处理函数。
    ///
    /// 返回值：
    /// - 返回新的 `ValueCommand<T, V>`。
    pub fn new_with_context(
        handler: impl Fn(&mut T, V, &CommandContext<T>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            handler: ValueCommandKind::WithContext(Arc::new(handler)),
        }
    }

    /// 在给定视图模型实例上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `value`：命令负载。
    pub fn execute(&self, view_model: &mut T, value: V) {
        let context = CommandContext::detached();
        self.execute_with_context(view_model, value, &context);
    }

    /// 在给定视图模型实例和运行时上下文上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `value`：命令负载。
    /// - `context`：运行时上下文。
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
