use super::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let _ = (options, app_id, on_action);
    Err(NotificationError::Backend(
        "macOS notification delivery requires the UserNotifications delegate bridge".to_string(),
    ))
}

pub(crate) fn platform_request_permission(
    callback: PermissionCallback,
) -> Result<(), NotificationError> {
    let _ = callback;
    Err(NotificationError::Backend(
        "macOS notification permission requests require the UserNotifications bridge".to_string(),
    ))
}

pub(crate) fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    Err(NotificationError::Backend(
        "macOS notification permission status requires the UserNotifications bridge".to_string(),
    ))
}
