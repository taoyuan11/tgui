use std::sync::Arc;

use super::{command::CommandEffect, context::CommandContext};

trait ErasedValueCommand<T, V>: Send + Sync {
    fn execute(&self, view_model: &mut T, value: V);
    fn execute_with_context(&self, view_model: &mut T, value: V, context: &CommandContext<T>);
    fn uses_context(&self) -> bool;

    fn declared_effect(&self) -> CommandEffect {
        CommandEffect::Conservative
    }
}

struct PlainValueCommand<F> {
    handler: F,
}

impl<T, V, F> ErasedValueCommand<T, V> for PlainValueCommand<F>
where
    F: Fn(&mut T, V) + Send + Sync,
{
    fn execute(&self, view_model: &mut T, value: V) {
        (self.handler)(view_model, value);
    }

    fn execute_with_context(&self, view_model: &mut T, value: V, _context: &CommandContext<T>) {
        (self.handler)(view_model, value);
    }

    fn uses_context(&self) -> bool {
        false
    }
}

struct ContextValueCommand<F> {
    handler: F,
}

impl<T: 'static, V, F> ErasedValueCommand<T, V> for ContextValueCommand<F>
where
    F: Fn(&mut T, V, &CommandContext<T>) + Send + Sync,
{
    fn execute(&self, view_model: &mut T, value: V) {
        let context = CommandContext::detached();
        (self.handler)(view_model, value, &context);
    }

    fn execute_with_context(&self, view_model: &mut T, value: V, context: &CommandContext<T>) {
        (self.handler)(view_model, value, context);
    }

    fn uses_context(&self) -> bool {
        true
    }
}

struct EffectValueCommand<T, V> {
    command: Arc<dyn ErasedValueCommand<T, V>>,
    effect: CommandEffect,
}

impl<T: 'static, V> ErasedValueCommand<T, V> for EffectValueCommand<T, V> {
    fn execute(&self, view_model: &mut T, value: V) {
        self.command.execute(view_model, value);
    }

    fn execute_with_context(&self, view_model: &mut T, value: V, context: &CommandContext<T>) {
        self.command
            .execute_with_context(view_model, value, context);
    }

    fn uses_context(&self) -> bool {
        self.command.uses_context()
    }

    fn declared_effect(&self) -> CommandEffect {
        self.effect
    }
}

/// 携带事件负载的视图模型命令。
pub struct ValueCommand<T, V> {
    handler: Arc<dyn ErasedValueCommand<T, V>>,
}

impl<T, V> Clone for ValueCommand<T, V> {
    fn clone(&self) -> Self {
        Self {
            handler: Arc::clone(&self.handler),
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
            handler: Arc::new(PlainValueCommand { handler }),
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
            handler: Arc::new(ContextValueCommand { handler }),
        }
    }

    /// 声明命令对 UI 的可见影响。
    ///
    /// `NoUiChange` 是显式契约：它声明命令没有 UI 可见影响。
    pub fn effect(self, effect: CommandEffect) -> Self
    where
        V: 'static,
    {
        if self.declared_effect() == effect {
            return self;
        }
        Self {
            handler: Arc::new(EffectValueCommand {
                command: self.handler,
                effect,
            }),
        }
    }

    /// 在给定视图模型实例上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `value`：命令负载。
    #[inline]
    pub fn execute(&self, view_model: &mut T, value: V) {
        self.handler.execute(view_model, value);
    }

    /// 在给定视图模型实例和运行时上下文上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `value`：命令负载。
    /// - `context`：运行时上下文。
    #[inline]
    pub fn execute_with_context(&self, view_model: &mut T, value: V, context: &CommandContext<T>) {
        self.handler
            .execute_with_context(view_model, value, context);
    }

    pub(crate) fn declared_effect(&self) -> CommandEffect {
        self.handler.declared_effect()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut T + Send + Sync>,
    ) -> ValueCommand<RootVm, V>
    where
        V: 'static,
    {
        let effect = self.declared_effect();
        let uses_context = self.handler.uses_context();
        let handler = self.handler;
        let command = if uses_context {
            ValueCommand::new_with_context(move |view_model, value, context| {
                let scoped_context = context.scope(selector.clone());
                handler.execute_with_context(selector(view_model), value, &scoped_context);
            })
        } else {
            ValueCommand::new(move |view_model, value| handler.execute(selector(view_model), value))
        };
        command.effect(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::ValueCommand;
    use crate::foundation::view_model::CommandEffect;
    use std::sync::Arc;

    #[derive(Default)]
    struct ChildVm {
        total: usize,
    }

    #[derive(Default)]
    struct RootVm {
        child: ChildVm,
    }

    #[test]
    fn value_command_effect_defaults_to_conservative_and_survives_clone() {
        let conservative = ValueCommand::new(|_: &mut RootVm, _: usize| {});
        assert_eq!(conservative.declared_effect(), CommandEffect::Conservative);

        let no_ui_change = conservative.effect(CommandEffect::NoUiChange);
        assert_eq!(
            no_ui_change.clone().declared_effect(),
            CommandEffect::NoUiChange
        );
        assert_eq!(
            no_ui_change
                .effect(CommandEffect::Conservative)
                .declared_effect(),
            CommandEffect::Conservative
        );
    }

    #[test]
    fn scoped_value_command_preserves_effect_and_targets_child_view_model() {
        let command = ValueCommand::new(|child: &mut ChildVm, value: usize| child.total += value)
            .effect(CommandEffect::NoUiChange);
        let scoped = command.scope(Arc::new(|root: &mut RootVm| &mut root.child));
        assert_eq!(scoped.declared_effect(), CommandEffect::NoUiChange);

        let mut root = RootVm::default();
        scoped.execute(&mut root, 7);
        assert_eq!(root.child.total, 7);
    }

    #[test]
    fn scoped_context_value_command_preserves_effect_and_targets_child_view_model() {
        let command = ValueCommand::new_with_context(|child: &mut ChildVm, value: usize, _| {
            child.total += value;
        })
        .effect(CommandEffect::NoUiChange);
        let scoped = command.scope(Arc::new(|root: &mut RootVm| &mut root.child));
        assert_eq!(scoped.declared_effect(), CommandEffect::NoUiChange);

        let mut root = RootVm::default();
        scoped.execute(&mut root, 7);
        assert_eq!(root.child.total, 7);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn command_effect_does_not_increase_value_command_size() {
        assert_eq!(std::mem::size_of::<ValueCommand<RootVm, usize>>(), 16);
        assert_eq!(
            std::mem::size_of::<Option<ValueCommand<RootVm, usize>>>(),
            16
        );
    }
}
