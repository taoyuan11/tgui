use super::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let mut notification = notify_rust::Notification::new();
    let app_name = options.app_name_text().or(app_id).unwrap_or("tgui");
    notification.appname(app_name).summary(options.title());
    if let Some(body) = options.body_text() {
        notification.body(body);
    }
    if let Some(icon) = options.icon_name() {
        notification.icon(icon);
    }
    for action in options.action_items() {
        notification.action(action.id(), action.label());
    }

    let handle = notification
        .show()
        .map_err(|error| NotificationError::Backend(error.to_string()))?;

    if let Some(on_action) = on_action {
        std::thread::spawn(move || {
            handle.wait_for_action(move |action| {
                if action != "__closed" {
                    on_action(action.to_string());
                }
            });
        });
    }

    Ok(())
}

pub(crate) fn platform_request_permission(
    callback: PermissionCallback,
) -> Result<(), NotificationError> {
    callback(Ok(NotificationPermission::Granted));
    Ok(())
}

pub(crate) fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    Ok(NotificationPermission::Granted)
}
