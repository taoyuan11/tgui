mod platform;
mod runtime;
#[cfg(test)]
mod tests;
mod types;

pub use runtime::Notifications;
pub use types::{
    NotificationAction, NotificationActionEvent, NotificationError, NotificationOptions,
    NotificationPermission,
};

#[cfg(target_os = "android")]
pub(crate) use platform::install_android_app;
#[cfg(target_os = "windows")]
pub(crate) use platform::prepare_platform_notifications;
pub(crate) use runtime::{
    async_notification_channel, AsyncNotificationDispatcher, AsyncNotificationReceiver,
};
