use std::fs;
use std::{env, path::PathBuf};

use super::{
    sanitize_windows_shortcut_file_name, validate_app_id, NotificationActionHandler,
    PermissionCallback,
};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

const WINDOWS_APP_USER_MODEL_ID_KEY: windows::Win32::Foundation::PROPERTYKEY =
    windows::Win32::Foundation::PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

pub(crate) fn prepare_platform_notifications(
    app_id: Option<&str>,
    display_name: &str,
) -> Result<(), NotificationError> {
    let app_id = validate_app_id(app_id)?;
    ensure_windows_notification_identity(app_id, display_name, true)
}

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    use std::sync::{Arc, Mutex};
    use windows::core::{IInspectable, Interface, HSTRING};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{
        ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
    };

    let app_id = validate_app_id(app_id)?;
    ensure_windows_notification_identity(
        app_id,
        options.app_name_text().unwrap_or(options.title()),
        false,
    )?;
    let xml = windows_toast_xml(&options);
    let document = XmlDocument::new().map_err(windows_error)?;
    document
        .LoadXml(&HSTRING::from(xml))
        .map_err(windows_error)?;
    let toast = ToastNotification::CreateToastNotification(&document).map_err(windows_error)?;
    if let Some(id) = options.notification_id() {
        toast.SetTag(&HSTRING::from(id)).map_err(windows_error)?;
    }

    if let Some(on_action) = on_action {
        let callback = Arc::new(Mutex::new(Some(on_action)));
        let callback_for_event = callback.clone();
        let handler =
            TypedEventHandler::<ToastNotification, IInspectable>::new(move |_sender, args| {
                if let Some(args) = args.as_ref() {
                    if let Ok(activated) = args.cast::<ToastActivatedEventArgs>() {
                        if let Ok(arguments) = activated.Arguments() {
                            if let Some(action_id) =
                                parse_windows_action_argument(&arguments.to_string())
                            {
                                if let Some(callback) = callback_for_event
                                    .lock()
                                    .expect("notification callback lock poisoned")
                                    .take()
                                {
                                    callback(action_id);
                                }
                            }
                        }
                    }
                }
                Ok(())
            });
        toast.Activated(&handler).map_err(windows_error)?;
    }

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))
        .map_err(windows_error)?;
    notifier.Show(&toast).map_err(windows_error)?;
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

fn ensure_windows_notification_identity(
    app_id: &str,
    display_name: &str,
    force_shortcut_update: bool,
) -> Result<(), NotificationError> {
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    // Windows Toast 依赖 AppUserModelID 和开始菜单快捷方式之间的绑定。
    // 这里在真正发送通知前确保当前进程和快捷方式身份一致，避免通知被系统静默丢弃。
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(&windows::core::HSTRING::from(app_id))
            .map_err(windows_error)?;
    }
    ensure_windows_notification_shortcut(app_id, display_name, force_shortcut_update)
}

fn ensure_windows_notification_shortcut(
    app_id: &str,
    display_name: &str,
    force_update: bool,
) -> Result<(), NotificationError> {
    use windows::core::Interface;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    struct ComGuard(bool);

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }

    let com_guard = match unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() } {
        Ok(_) => ComGuard(true),
        Err(error) if error.code() == windows::Win32::Foundation::RPC_E_CHANGED_MODE => {
            ComGuard(false)
        }
        Err(error) => return Err(windows_error(error)),
    };

    let shortcut_path = windows_notification_shortcut_path(display_name)?;
    if shortcut_path.exists() && !force_update {
        return Ok(());
    }

    let app_id_shortcut_path = windows_notification_shortcut_path(app_id)?;
    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            NotificationError::Backend(format!(
                "failed to create notification shortcut directory: {error}"
            ))
        })?;
    }
    if app_id_shortcut_path != shortcut_path && app_id_shortcut_path.exists() {
        let _ = fs::remove_file(&app_id_shortcut_path);
    }

    let exe_path = env::current_exe().map_err(|error| {
        NotificationError::Backend(format!(
            "failed to resolve current executable for notifications: {error}"
        ))
    })?;
    let exe_string = exe_path.to_string_lossy().into_owned();
    let shortcut_string = shortcut_path.to_string_lossy().into_owned();
    let description = format!("{display_name} notifications");

    unsafe {
        let shell_link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(windows_error)?;
        shell_link
            .SetPath(&windows::core::HSTRING::from(exe_string.as_str()))
            .map_err(windows_error)?;
        if let Some(working_directory) = exe_path.parent() {
            let working_directory = working_directory.to_string_lossy().into_owned();
            shell_link
                .SetWorkingDirectory(&windows::core::HSTRING::from(working_directory))
                .map_err(windows_error)?;
        }
        shell_link
            .SetDescription(&windows::core::HSTRING::from(description))
            .map_err(windows_error)?;
        shell_link
            .SetIconLocation(&windows::core::HSTRING::from(exe_string.as_str()), 0)
            .map_err(windows_error)?;

        let property_store: IPropertyStore = shell_link.cast().map_err(windows_error)?;
        let app_id_variant: PROPVARIANT = app_id.into();
        property_store
            .SetValue(&WINDOWS_APP_USER_MODEL_ID_KEY, &app_id_variant)
            .map_err(windows_error)?;
        property_store.Commit().map_err(windows_error)?;

        let persist_file: IPersistFile = shell_link.cast().map_err(windows_error)?;
        persist_file
            .Save(&windows::core::HSTRING::from(shortcut_string), true)
            .map_err(windows_error)?;
    }

    drop(com_guard);
    Ok(())
}

fn windows_notification_shortcut_path(shortcut_name: &str) -> Result<PathBuf, NotificationError> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Programs, SHGetKnownFolderPath, KNOWN_FOLDER_FLAG};

    let programs_dir = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Programs, KNOWN_FOLDER_FLAG(0), None)
            .map_err(windows_error)?
    };
    let programs_path = unsafe { programs_dir.to_string() }.map_err(|error| {
        NotificationError::Backend(format!(
            "failed to resolve Start Menu programs directory: {error}"
        ))
    })?;
    unsafe {
        CoTaskMemFree(Some(programs_dir.0 as _));
    }

    Ok(PathBuf::from(programs_path).join("tgui").join(format!(
        "{}.lnk",
        sanitize_windows_shortcut_file_name(shortcut_name)
    )))
}

fn windows_toast_xml(options: &NotificationOptions) -> String {
    fn esc(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    let launch = options
        .notification_id()
        .map(|id| format!(" launch=\"notification_id={}\"", esc(id)))
        .unwrap_or_default();
    let mut xml = format!(
        "<toast{launch}><visual><Signal template=\"ToastGeneric\"><text>{}</text>",
        esc(options.title())
    );
    if let Some(subtitle) = options.subtitle_text() {
        xml.push_str(&format!("<text>{}</text>", esc(subtitle)));
    }
    if let Some(body) = options.body_text() {
        xml.push_str(&format!("<text>{}</text>", esc(body)));
    }
    xml.push_str("</Signal></visual>");
    if !options.action_items().is_empty() {
        xml.push_str("<actions>");
        for action in options.action_items() {
            xml.push_str(&format!(
                "<action content=\"{}\" arguments=\"action_id={}\" activationType=\"foreground\"/>",
                esc(action.label()),
                esc(action.id())
            ));
        }
        xml.push_str("</actions>");
    }
    xml.push_str("</toast>");
    xml
}

fn parse_windows_action_argument(arguments: &str) -> Option<String> {
    arguments
        .split('&')
        .find_map(|part| part.strip_prefix("action_id=").map(str::to_string))
}

fn windows_error(error: windows::core::Error) -> NotificationError {
    NotificationError::Backend(error.to_string())
}
