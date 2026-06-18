use std::sync::{mpsc, Arc, Mutex};

use crate::foundation::view_model::{CommandContext, ValueCommand};
use crate::platform::backend::event_loop::EventLoopProxy;

use super::platform::{platform_permission_status, platform_request_permission, platform_send};
use super::types::{
    NotificationActionEvent, NotificationError, NotificationOptions, NotificationPermission,
};

type AsyncNotificationCallback<VM> = Box<dyn FnOnce(&mut VM, &CommandContext<VM>) + Send>;
type ScopedNotificationDispatcher<VM> =
    Arc<dyn Fn(PendingNotificationCompletion<VM>) -> Result<(), NotificationError> + Send + Sync>;

pub(crate) struct PendingNotificationCompletion<VM> {
    pub(crate) window_key: String,
    pub(crate) window_instance_id: u64,
    pub(crate) callback: AsyncNotificationCallback<VM>,
}

pub(crate) struct AsyncNotificationReceiver<VM> {
    receiver: mpsc::Receiver<PendingNotificationCompletion<VM>>,
}

impl<VM> AsyncNotificationReceiver<VM> {
    pub(crate) fn try_iter(&self) -> mpsc::TryIter<'_, PendingNotificationCompletion<VM>> {
        self.receiver.try_iter()
    }
}

pub(crate) struct AsyncNotificationDispatcher<VM> {
    sender: mpsc::Sender<PendingNotificationCompletion<VM>>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
}

impl<VM> Clone for AsyncNotificationDispatcher<VM> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            proxy: self.proxy.clone(),
        }
    }
}

impl<VM> AsyncNotificationDispatcher<VM> {
    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock().expect("notification proxy lock poisoned") = Some(proxy);
    }

    pub(crate) fn dispatch(
        &self,
        completion: PendingNotificationCompletion<VM>,
    ) -> Result<(), NotificationError> {
        self.sender.send(completion).map_err(|_| {
            NotificationError::Backend(
                "failed to dispatch notification completion to the runtime".to_string(),
            )
        })?;

        // 平台通知动作通常在外部线程回调；这里显式唤醒事件循环，
        // 让运行时在主线程消费 completion 并安全地回调 ViewModel。
        if let Some(proxy) = self
            .proxy
            .lock()
            .expect("notification proxy lock poisoned")
            .as_ref()
            .cloned()
        {
            proxy.wake_up();
        }

        Ok(())
    }
}

pub(crate) fn async_notification_channel<VM>() -> (
    AsyncNotificationDispatcher<VM>,
    AsyncNotificationReceiver<VM>,
) {
    let (sender, receiver) = mpsc::channel();
    (
        AsyncNotificationDispatcher {
            sender,
            proxy: Arc::new(Mutex::new(None)),
        },
        AsyncNotificationReceiver { receiver },
    )
}

struct NotificationRuntimeContext<VM> {
    window_key: String,
    window_instance_id: u64,
    app_id: Option<String>,
    dispatcher: ScopedNotificationDispatcher<VM>,
}

impl<VM> Clone for NotificationRuntimeContext<VM> {
    fn clone(&self) -> Self {
        Self {
            window_key: self.window_key.clone(),
            window_instance_id: self.window_instance_id,
            app_id: self.app_id.clone(),
            dispatcher: self.dispatcher.clone(),
        }
    }
}

/// 提供系统通知发送、权限查询和交互动作回调能力。
pub struct Notifications<VM> {
    runtime: Option<NotificationRuntimeContext<VM>>,
}

impl<VM> Clone for Notifications<VM> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
        }
    }
}

impl<VM: 'static> Notifications<VM> {
    pub(crate) fn detached() -> Self {
        Self { runtime: None }
    }

    pub(crate) fn from_runtime(
        window_key: String,
        window_instance_id: u64,
        app_id: Option<String>,
        dispatcher: AsyncNotificationDispatcher<VM>,
    ) -> Self {
        Self {
            runtime: Some(NotificationRuntimeContext {
                window_key,
                window_instance_id,
                app_id,
                dispatcher: Arc::new(move |completion| dispatcher.dispatch(completion)),
            }),
        }
    }

    pub(crate) fn scope<ChildVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut VM) -> &'a mut ChildVm + Send + Sync>,
    ) -> Notifications<ChildVm> {
        let Some(runtime) = &self.runtime else {
            return Notifications { runtime: None };
        };

        let dispatcher = runtime.dispatcher.clone();
        Notifications {
            runtime: Some(NotificationRuntimeContext {
                window_key: runtime.window_key.clone(),
                window_instance_id: runtime.window_instance_id,
                app_id: runtime.app_id.clone(),
                dispatcher: Arc::new(move |completion: PendingNotificationCompletion<ChildVm>| {
                    let scoped_selector = selector.clone();
                    dispatcher(PendingNotificationCompletion {
                        window_key: completion.window_key,
                        window_instance_id: completion.window_instance_id,
                        callback: Box::new(move |view_model, context| {
                            let scoped_context = context.scope(scoped_selector.clone());
                            (completion.callback)(scoped_selector(view_model), &scoped_context);
                        }),
                    })
                }),
            }),
        }
    }

    /// 发送普通系统通知。
    ///
    /// 参数:
    /// - `options`: 通知展示参数；若未显式设置通知 ID，会在发送前自动生成。
    ///
    /// 返回值: 成功时返回最终使用的通知 ID。
    pub fn send(&self, mut options: NotificationOptions) -> Result<String, NotificationError> {
        options.validate(false)?;
        if !options.action_items().is_empty() {
            return Err(NotificationError::InvalidOptions(
                "use send_with_actions for interactive notifications".to_string(),
            ));
        }

        let runtime = self.runtime_context()?;
        let notification_id = options.ensure_id();
        platform_send(
            options,
            runtime.app_id.as_deref(),
            None::<Box<dyn FnOnce(String) + Send>>,
        )?;
        Ok(notification_id)
    }

    /// 发送带交互动作的系统通知。
    ///
    /// 参数:
    /// - `options`: 通知展示参数，必须至少包含一个动作。
    /// - `callback`: 用户点击动作后在运行时主线程执行的回调命令。
    ///
    /// 返回值: 成功时返回最终使用的通知 ID。
    pub fn send_with_actions(
        &self,
        mut options: NotificationOptions,
        callback: ValueCommand<VM, Result<NotificationActionEvent, NotificationError>>,
    ) -> Result<String, NotificationError> {
        options.validate(true)?;
        let runtime = self.runtime_context()?.clone();
        let notification_id = options.ensure_id();
        let callback_notification_id = notification_id.clone();
        let dispatcher = runtime.dispatcher.clone();
        let window_key = runtime.window_key.clone();
        let window_instance_id = runtime.window_instance_id;

        platform_send(
            options,
            runtime.app_id.as_deref(),
            Some(Box::new(move |action_id| {
                let event = NotificationActionEvent {
                    notification_id: callback_notification_id,
                    action_id,
                };
                let _ = dispatcher(PendingNotificationCompletion {
                    window_key,
                    window_instance_id,
                    callback: Box::new(move |view_model, context| {
                        callback.execute_with_context(view_model, Ok(event), context);
                    }),
                });
            })),
        )?;

        Ok(notification_id)
    }

    /// 请求平台通知权限。
    ///
    /// 参数:
    /// - `callback`: 权限请求完成后在运行时主线程执行的回调命令。
    ///
    /// 返回值: 若请求已成功提交给平台，则返回 `Ok(())`。
    pub fn request_permission(
        &self,
        callback: ValueCommand<VM, Result<NotificationPermission, NotificationError>>,
    ) -> Result<(), NotificationError> {
        let runtime = self.runtime_context()?.clone();
        let dispatcher = runtime.dispatcher.clone();
        let window_key = runtime.window_key;
        let window_instance_id = runtime.window_instance_id;

        platform_request_permission(Box::new(move |result| {
            let _ = dispatcher(PendingNotificationCompletion {
                window_key,
                window_instance_id,
                callback: Box::new(move |view_model, context| {
                    callback.execute_with_context(view_model, result, context);
                }),
            });
        }))
    }

    /// 查询当前平台的通知权限状态。
    ///
    /// 返回值: 平台可用时返回权限状态，否则返回平台错误。
    pub fn permission_status(&self) -> Result<NotificationPermission, NotificationError> {
        self.runtime_context()?;
        platform_permission_status()
    }

    fn runtime_context(&self) -> Result<&NotificationRuntimeContext<VM>, NotificationError> {
        self.runtime.as_ref().ok_or_else(|| {
            NotificationError::Backend(
                "notification context is not available for this command".to_string(),
            )
        })
    }
}
