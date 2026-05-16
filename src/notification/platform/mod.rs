#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "android", target_env = "ohos"))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

use super::types::{NotificationError, NotificationPermission};

pub(crate) type NotificationActionHandler = Box<dyn FnOnce(String) + Send>;
pub(crate) type PermissionCallback =
    Box<dyn FnOnce(Result<NotificationPermission, NotificationError>) + Send>;

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
pub(crate) use linux::{platform_permission_status, platform_request_permission, platform_send};
#[cfg(target_os = "macos")]
pub(crate) use macos::{platform_permission_status, platform_request_permission, platform_send};
#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) use unsupported::{
    platform_permission_status, platform_request_permission, platform_send,
};
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    platform_permission_status, platform_request_permission, platform_send,
    prepare_platform_notifications,
};

fn validate_app_id(app_id: Option<&str>) -> Result<&str, NotificationError> {
    app_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            NotificationError::InvalidOptions(
                "Application::app_id must be configured before sending notifications on this platform"
                    .to_string(),
            )
        })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn prepare_platform_notifications(
    _app_id: Option<&str>,
    _display_name: &str,
) -> Result<(), NotificationError> {
    Ok(())
}

pub(crate) fn sanitize_windows_shortcut_file_name(app_id: &str) -> String {
    let sanitized: String = app_id
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.');
    if trimmed.is_empty() {
        "tgui".to_string()
    } else {
        trimmed.to_string()
    }
}
