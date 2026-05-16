use std::sync::Arc;

use crate::dialog::Dialogs;
use crate::foundation::window_control::WindowControl;
use crate::log::Log;
use crate::notification::Notifications;

/// 命令执行时可访问的运行时上下文。
///
/// 该上下文封装了对话框、通知、窗口控制和日志等运行时服务。
pub struct CommandContext<T> {
    dialogs: Dialogs<T>,
    notifications: Notifications<T>,
    window: WindowControl,
    log: Log,
}

impl<T> Clone for CommandContext<T> {
    fn clone(&self) -> Self {
        Self {
            dialogs: self.dialogs.clone(),
            notifications: self.notifications.clone(),
            window: self.window.clone(),
            log: self.log.clone(),
        }
    }
}

impl<T: 'static> CommandContext<T> {
    /// 返回当前视图模型作用域下的对话框服务。
    ///
    /// 返回值：
    /// - 返回一个可复用的 `Dialogs<T>` 句柄。
    pub fn dialogs(&self) -> Dialogs<T> {
        self.dialogs.clone()
    }

    /// 返回当前视图模型作用域下的通知服务。
    ///
    /// 返回值：
    /// - 返回一个可复用的 `Notifications<T>` 句柄。
    pub fn notifications(&self) -> Notifications<T> {
        self.notifications.clone()
    }

    /// 返回窗口控制服务。
    ///
    /// 返回值：
    /// - 返回一个可复用的 `WindowControl` 句柄。
    pub fn window(&self) -> WindowControl {
        self.window.clone()
    }

    /// 返回日志服务。
    ///
    /// 返回值：
    /// - 返回一个可复用的 `Log` 句柄。
    pub fn log(&self) -> Log {
        self.log.clone()
    }

    pub(crate) fn new(
        dialogs: Dialogs<T>,
        notifications: Notifications<T>,
        window: WindowControl,
        log: Log,
    ) -> Self {
        Self {
            dialogs,
            notifications,
            window,
            log,
        }
    }

    pub(crate) fn detached() -> Self {
        Self::new(
            Dialogs::detached(),
            Notifications::detached(),
            WindowControl::default(),
            Log::default(),
        )
    }

    pub(crate) fn scope<ChildVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut T) -> &'a mut ChildVm + Send + Sync>,
    ) -> CommandContext<ChildVm> {
        let notification_selector = selector.clone();
        CommandContext::new(
            self.dialogs.scope(selector),
            self.notifications.scope(notification_selector),
            self.window.clone(),
            self.log.clone(),
        )
    }
}
