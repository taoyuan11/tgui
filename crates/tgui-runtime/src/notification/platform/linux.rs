use std::process::Command;

use super::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{
    NotificationAction, NotificationError, NotificationOptions, NotificationPermission,
};

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let fallback_options = options.clone();
    let app_name = options
        .app_name_text()
        .or(app_id)
        .unwrap_or("tgui")
        .to_string();
    let mut notification = notify_rust::Notification::new();
    notification.appname(&app_name).summary(options.title());
    if let Some(body) = options.body_text() {
        notification.body(body);
    }
    if let Some(icon) = options.icon_name() {
        notification.icon(icon);
    }
    for action in options.action_items() {
        notification.action(action.id(), action.label());
    }

    let handle = match notification.show() {
        Ok(handle) => handle,
        Err(primary_error) => {
            return send_via_notify_send(
                fallback_options,
                app_name,
                on_action,
                primary_error.to_string(),
            );
        }
    };

    if let Some(on_action) = on_action {
        std::thread::spawn(move || {
            handle.wait_for_action(move |action| {
                if action != "__closed" {
                    on_action(action.to_string());
                }
            });
        });
    } else {
        std::thread::spawn(move || {
            // Keep the DBus notification handle alive until the server closes it.
            // Some Linux desktops drop the notification entirely if the client
            // connection disappears immediately after `show()`.
            handle.on_close(|_| {});
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

fn send_via_notify_send(
    options: NotificationOptions,
    app_name: String,
    on_action: Option<NotificationActionHandler>,
    primary_error: String,
) -> Result<(), NotificationError> {
    if let Some(on_action) = on_action {
        std::thread::spawn(move || {
            if let Ok(output) = build_notify_send_command(&options, &app_name).output() {
                if output.status.success() {
                    let action = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !action.is_empty() {
                        on_action(action);
                    }
                }
            }
        });
        return Ok(());
    }

    let output = build_notify_send_command(&options, &app_name).output().map_err(|error| {
        NotificationError::Backend(format!(
            "failed to deliver Linux notification via notify-rust ({primary_error}); notify-send fallback failed to start: {error}"
        ))
    })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };

    Err(NotificationError::Backend(format!(
        "failed to deliver Linux notification via notify-rust ({primary_error}); notify-send fallback failed: {detail}"
    )))
}

fn build_notify_send_command(options: &NotificationOptions, app_name: &str) -> Command {
    let mut command = Command::new("notify-send");
    command.arg(format!("--app-name={app_name}"));

    if let Some(icon) = options.icon_name() {
        command.arg(format!("--icon={icon}"));
    }

    for action in options.action_items() {
        command.arg(format!("--action={}", format_notify_send_action(action)));
    }

    command.arg(options.title());
    if let Some(body) = options.body_text() {
        command.arg(body);
    }

    command
}

fn format_notify_send_action(action: &NotificationAction) -> String {
    format!("{}={}", action.id(), action.label())
}
