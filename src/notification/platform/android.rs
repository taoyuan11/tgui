use super::android_bridge::{self, AndroidNotificationRequest};
use super::{validate_app_id, NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let app_id = validate_app_id(app_id)?.to_string();
    match platform_permission_status()? {
        NotificationPermission::Granted => {}
        NotificationPermission::NotDetermined => {
            return Err(NotificationError::Backend(
                "notification permission has not been granted; call request_permission() first on Android 13+".to_string(),
            ));
        }
        NotificationPermission::Denied => {
            return Err(NotificationError::Backend(
                "notification permission was denied on Android".to_string(),
            ));
        }
    }

    let notification_id = options.notification_id().ok_or_else(|| {
        NotificationError::Backend(
            "notification id was not initialized before reaching the Android backend".to_string(),
        )
    })?;

    let action_callback_id = on_action.map(android_bridge::install_action_slot);
    let request = AndroidNotificationRequest {
        app_id,
        notification_id: notification_id.to_string(),
        channel_name: options
            .app_name_text()
            .unwrap_or(options.title())
            .to_string(),
        title: options.title().to_string(),
        body: options.body_text().map(str::to_string),
        subtitle: options.subtitle_text().map(str::to_string),
        icon: options.icon_name().map(str::to_string),
        sound: options.sound_enabled(),
        action_callback_id,
        action_ids: options
            .action_items()
            .iter()
            .map(|action| action.id().to_string())
            .collect(),
        action_labels: options
            .action_items()
            .iter()
            .map(|action| action.label().to_string())
            .collect(),
    };

    android_bridge::send_notification(request).inspect_err(|_| {
        if let Some(action_callback_id) = action_callback_id {
            android_bridge::discard_action_slot(action_callback_id);
        }
    })
}

pub(crate) fn platform_request_permission(
    callback: PermissionCallback,
) -> Result<(), NotificationError> {
    android_bridge::request_permission(callback)
}

pub(crate) fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    android_bridge::permission_status()
}

pub(crate) use android_bridge::install_android_app;
