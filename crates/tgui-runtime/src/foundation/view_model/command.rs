use std::sync::Arc;

use super::context::CommandContext;

trait ErasedCommand<T>: Send + Sync {
    fn execute(&self, view_model: &mut T);
    fn execute_with_context(&self, view_model: &mut T, context: &CommandContext<T>);
    fn uses_context(&self) -> bool;

    fn declared_effect(&self) -> CommandEffect {
        CommandEffect::Conservative
    }
}

struct PlainCommand<F> {
    handler: F,
}

impl<T, F> ErasedCommand<T> for PlainCommand<F>
where
    F: Fn(&mut T) + Send + Sync,
{
    fn execute(&self, view_model: &mut T) {
        (self.handler)(view_model);
    }

    fn execute_with_context(&self, view_model: &mut T, _context: &CommandContext<T>) {
        (self.handler)(view_model);
    }

    fn uses_context(&self) -> bool {
        false
    }
}

struct ContextCommand<F> {
    handler: F,
}

impl<T: 'static, F> ErasedCommand<T> for ContextCommand<F>
where
    F: Fn(&mut T, &CommandContext<T>) + Send + Sync,
{
    fn execute(&self, view_model: &mut T) {
        let context = CommandContext::detached();
        (self.handler)(view_model, &context);
    }

    fn execute_with_context(&self, view_model: &mut T, context: &CommandContext<T>) {
        (self.handler)(view_model, context);
    }

    fn uses_context(&self) -> bool {
        true
    }
}

struct EffectCommand<T> {
    command: Arc<dyn ErasedCommand<T>>,
    effect: CommandEffect,
}

impl<T: 'static> ErasedCommand<T> for EffectCommand<T> {
    fn execute(&self, view_model: &mut T) {
        self.command.execute(view_model);
    }

    fn execute_with_context(&self, view_model: &mut T, context: &CommandContext<T>) {
        self.command.execute_with_context(view_model, context);
    }

    fn uses_context(&self) -> bool {
        self.command.uses_context()
    }

    fn declared_effect(&self) -> CommandEffect {
        self.effect
    }
}

/// 描述命令对 UI 的可见影响。
///
/// 默认使用 [`CommandEffect::Conservative`]，运行时会在命令执行后保守地失效场景。
/// 完全没有 UI 可见影响的命令可选择 [`CommandEffect::NoUiChange`]。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommandEffect {
    /// 命令可能改变 UI；保持完整的保守失效语义。
    #[default]
    Conservative,
    /// 命令没有 UI 可见影响；当命令执行期间也没有任何响应式或 root rebuild
    /// revision 变化时，运行时可以保留现有场景缓存。
    NoUiChange,
}

/// 不携带事件负载的视图模型命令。
pub struct Command<T> {
    handler: Arc<dyn ErasedCommand<T>>,
}

impl<T> Clone for Command<T> {
    fn clone(&self) -> Self {
        Self {
            handler: Arc::clone(&self.handler),
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
            handler: Arc::new(PlainCommand { handler }),
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
            handler: Arc::new(ContextCommand { handler }),
        }
    }

    /// 声明命令对 UI 的可见影响。
    ///
    /// `NoUiChange` 是显式契约而不是自动推断：它声明命令没有 UI 可见影响。
    pub fn effect(self, effect: CommandEffect) -> Self {
        if self.declared_effect() == effect {
            return self;
        }
        Self {
            handler: Arc::new(EffectCommand {
                command: self.handler,
                effect,
            }),
        }
    }

    /// 在给定视图模型实例上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    #[inline]
    pub fn execute(&self, view_model: &mut T) {
        self.handler.execute(view_model);
    }

    /// 在给定视图模型实例和运行时上下文上执行命令。
    ///
    /// 参数：
    /// - `view_model`：要被修改的视图模型实例。
    /// - `context`：运行时上下文。
    #[inline]
    pub fn execute_with_context(&self, view_model: &mut T, context: &CommandContext<T>) {
        self.handler.execute_with_context(view_model, context);
    }

    pub(crate) fn declared_effect(&self) -> CommandEffect {
        self.handler.declared_effect()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut T + Send + Sync>,
    ) -> Command<RootVm> {
        let effect = self.declared_effect();
        let uses_context = self.handler.uses_context();
        let handler = self.handler;
        let command = if uses_context {
            Command::new_with_context(move |view_model, context| {
                let scoped_context = context.scope(selector.clone());
                handler.execute_with_context(selector(view_model), &scoped_context);
            })
        } else {
            Command::new(move |view_model| handler.execute(selector(view_model)))
        };
        command.effect(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandEffect};
    use std::sync::Arc;

    #[derive(Default)]
    struct ChildVm {
        calls: usize,
    }

    #[derive(Default)]
    struct RootVm {
        child: ChildVm,
    }

    #[test]
    fn command_effect_defaults_to_conservative_and_survives_clone() {
        let conservative = Command::new(|_: &mut RootVm| {});
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
    fn scoped_command_preserves_effect_and_targets_child_view_model() {
        let command =
            Command::new(|child: &mut ChildVm| child.calls += 1).effect(CommandEffect::NoUiChange);
        let scoped = command.scope(Arc::new(|root: &mut RootVm| &mut root.child));
        assert_eq!(scoped.declared_effect(), CommandEffect::NoUiChange);

        let mut root = RootVm::default();
        scoped.execute(&mut root);
        assert_eq!(root.child.calls, 1);
    }

    #[test]
    fn scoped_context_command_preserves_effect_and_targets_child_view_model() {
        let command = Command::new_with_context(|child: &mut ChildVm, _| child.calls += 1)
            .effect(CommandEffect::NoUiChange);
        let scoped = command.scope(Arc::new(|root: &mut RootVm| &mut root.child));
        assert_eq!(scoped.declared_effect(), CommandEffect::NoUiChange);

        let mut root = RootVm::default();
        scoped.execute(&mut root);
        assert_eq!(root.child.calls, 1);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn command_effect_does_not_increase_command_size() {
        assert_eq!(std::mem::size_of::<Command<RootVm>>(), 16);
        assert_eq!(std::mem::size_of::<Option<Command<RootVm>>>(), 16);
    }
}
