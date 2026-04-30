use std::error::Error;
use std::fmt::{Display, Formatter};
#[cfg(target_os = "windows")]
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
#[cfg(target_os = "windows")]
use std::{env, path::PathBuf};

use crate::foundation::view_model::{CommandContext, ValueCommand};
use crate::platform::backend::event_loop::EventLoopProxy;

const MAX_ACTIONS: usize = 2;

static NEXT_NOTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    UnsupportedPlatform,
    InvalidOptions(String),
    Backend(String),
}

impl Display for NotificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                write!(f, "notifications are not supported on this platform")
            }
            Self::InvalidOptions(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl Error for NotificationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPermission {
    Granted,
    Denied,
    NotDetermined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    id: String,
    label: String,
}

impl NotificationAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationActionEvent {
    pub notification_id: String,
    pub action_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationOptions {
    id: Option<String>,
    title: String,
    body: Option<String>,
    subtitle: Option<String>,
    app_name: Option<String>,
    icon: Option<String>,
    sound: bool,
    actions: Vec<NotificationAction>,
}

impl NotificationOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            title: title.into(),
            body: None,
            subtitle: None,
            app_name: None,
            icon: None,
            sound: true,
            actions: Vec::new(),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn sound(mut self, sound: bool) -> Self {
        self.sound = sound;
        self
    }

    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = NotificationAction>) -> Self {
        self.actions.extend(actions);
        self
    }

    pub fn notification_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body_text(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn subtitle_text(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub fn app_name_text(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    pub fn icon_name(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    pub fn sound_enabled(&self) -> bool {
        self.sound
    }

    pub fn action_items(&self) -> &[NotificationAction] {
        &self.actions
    }

    fn ensure_id(&mut self) -> String {
        if let Some(id) = self.id.as_ref() {
            return id.clone();
        }

        let id = format!(
            "tgui-notification-{}",
            NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::Relaxed)
        );
        self.id = Some(id.clone());
        id
    }

    fn validate(&self, require_actions: bool) -> Result<(), NotificationError> {
        if self.title.trim().is_empty() {
            return Err(NotificationError::InvalidOptions(
                "notification title cannot be empty".to_string(),
            ));
        }

        if require_actions && self.actions.is_empty() {
            return Err(NotificationError::InvalidOptions(
                "interactive notifications require at least one action".to_string(),
            ));
        }

        if self.actions.len() > MAX_ACTIONS {
            return Err(NotificationError::InvalidOptions(format!(
                "notifications support at most {MAX_ACTIONS} actions"
            )));
        }

        for action in &self.actions {
            if action.id.trim().is_empty() {
                return Err(NotificationError::InvalidOptions(
                    "notification action id cannot be empty".to_string(),
                ));
            }
            if action.label.trim().is_empty() {
                return Err(NotificationError::InvalidOptions(
                    "notification action label cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}

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

    pub fn send(&self, mut options: NotificationOptions) -> Result<String, NotificationError> {
        options.validate(false)?;
        if !options.actions.is_empty() {
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

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<Box<dyn FnOnce(String) + Send>>,
) -> Result<(), NotificationError> {
    let mut notification = notify_rust::Notification::new();
    let app_name = options.app_name.as_deref().or(app_id).unwrap_or("tgui");
    notification.appname(app_name).summary(&options.title);
    if let Some(body) = options.body.as_deref() {
        notification.body(body);
    }
    if let Some(icon) = options.icon.as_deref() {
        notification.icon(icon);
    }
    for action in &options.actions {
        notification.action(&action.id, &action.label);
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

#[cfg(target_os = "windows")]
fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<Box<dyn FnOnce(String) + Send>>,
) -> Result<(), NotificationError> {
    use std::sync::{Arc, Mutex};
    use windows::core::{IInspectable, Interface, HSTRING};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{
        ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
    };

    let app_id = validate_app_id(app_id)?;
    ensure_windows_notification_shortcut(
        app_id,
        options.app_name.as_deref().unwrap_or(&options.title),
    )?;
    let xml = windows_toast_xml(&options);
    let document = XmlDocument::new().map_err(windows_error)?;
    document
        .LoadXml(&HSTRING::from(xml))
        .map_err(windows_error)?;
    let toast = ToastNotification::CreateToastNotification(&document).map_err(windows_error)?;
    if let Some(id) = options.id.as_ref() {
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

#[cfg(target_os = "windows")]
const WINDOWS_APP_USER_MODEL_ID_KEY: windows::Win32::Foundation::PROPERTYKEY =
    windows::Win32::Foundation::PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

#[cfg(target_os = "windows")]
fn ensure_windows_notification_shortcut(
    app_id: &str,
    display_name: &str,
) -> Result<(), NotificationError> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize, IPersistFile,
    };
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
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
    let legacy_shortcut_path = windows_notification_shortcut_path(app_id)?;
    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            NotificationError::Backend(format!(
                "failed to create notification shortcut directory: {error}"
            ))
        })?;
    }
    if legacy_shortcut_path != shortcut_path && legacy_shortcut_path.exists() {
        let _ = fs::remove_file(&legacy_shortcut_path);
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

#[cfg(target_os = "windows")]
fn windows_notification_shortcut_path(shortcut_name: &str) -> Result<PathBuf, NotificationError> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Programs, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath};

    let programs_dir = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Programs, KNOWN_FOLDER_FLAG(0), None).map_err(windows_error)?
    };
    let programs_path = unsafe { programs_dir.to_string() }.map_err(|error| {
        NotificationError::Backend(format!(
            "failed to resolve Start Menu programs directory: {error}"
        ))
    })?;
    unsafe {
        CoTaskMemFree(Some(programs_dir.0 as _));
    }

    Ok(PathBuf::from(programs_path)
        .join("tgui")
        .join(format!("{}.lnk", sanitize_windows_shortcut_file_name(shortcut_name))))
}

fn sanitize_windows_shortcut_file_name(app_id: &str) -> String {
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

#[cfg(target_os = "windows")]
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
        .id
        .as_deref()
        .map(|id| format!(" launch=\"notification_id={}\"", esc(id)))
        .unwrap_or_default();
    let mut xml = format!(
        "<toast{launch}><visual><binding template=\"ToastGeneric\"><text>{}</text>",
        esc(&options.title)
    );
    if let Some(subtitle) = options.subtitle.as_deref() {
        xml.push_str(&format!("<text>{}</text>", esc(subtitle)));
    }
    if let Some(body) = options.body.as_deref() {
        xml.push_str(&format!("<text>{}</text>", esc(body)));
    }
    xml.push_str("</binding></visual>");
    if !options.actions.is_empty() {
        xml.push_str("<actions>");
        for action in &options.actions {
            xml.push_str(&format!(
                "<action content=\"{}\" arguments=\"action_id={}\" activationType=\"foreground\"/>",
                esc(&action.label),
                esc(&action.id)
            ));
        }
        xml.push_str("</actions>");
    }
    xml.push_str("</toast>");
    xml
}

#[cfg(target_os = "windows")]
fn parse_windows_action_argument(arguments: &str) -> Option<String> {
    arguments
        .split('&')
        .find_map(|part| part.strip_prefix("action_id=").map(str::to_string))
}

#[cfg(target_os = "windows")]
fn windows_error(error: windows::core::Error) -> NotificationError {
    NotificationError::Backend(error.to_string())
}

#[cfg(target_os = "macos")]
fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<Box<dyn FnOnce(String) + Send>>,
) -> Result<(), NotificationError> {
    let _ = (options, app_id, on_action);
    Err(NotificationError::Backend(
        "macOS notification delivery requires the UserNotifications delegate bridge".to_string(),
    ))
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<Box<dyn FnOnce(String) + Send>>,
) -> Result<(), NotificationError> {
    let _ = (options, app_id, on_action);
    Err(NotificationError::UnsupportedPlatform)
}

#[cfg(any(
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn platform_request_permission(
    callback: Box<dyn FnOnce(Result<NotificationPermission, NotificationError>) + Send>,
) -> Result<(), NotificationError> {
    callback(Ok(NotificationPermission::Granted));
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_request_permission(
    callback: Box<dyn FnOnce(Result<NotificationPermission, NotificationError>) + Send>,
) -> Result<(), NotificationError> {
    let _ = callback;
    Err(NotificationError::Backend(
        "macOS notification permission requests require the UserNotifications bridge".to_string(),
    ))
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn platform_request_permission(
    callback: Box<dyn FnOnce(Result<NotificationPermission, NotificationError>) + Send>,
) -> Result<(), NotificationError> {
    callback(Err(NotificationError::UnsupportedPlatform));
    Ok(())
}

#[cfg(any(
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    Ok(NotificationPermission::Granted)
}

#[cfg(target_os = "macos")]
fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    Err(NotificationError::Backend(
        "macOS notification permission status requires the UserNotifications bridge".to_string(),
    ))
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    Err(NotificationError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::{
        async_notification_channel, NotificationAction, NotificationError, NotificationOptions,
        PendingNotificationCompletion,
    };
    use crate::foundation::binding::ViewModelContext;
    use crate::foundation::view_model::{CommandContext, ValueCommand, ViewModel};
    use crate::ui::widget::Element;
    use std::sync::{Arc, Mutex};

    #[test]
    fn validates_empty_title() {
        let result = NotificationOptions::new("").validate(false);

        assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
    }

    #[test]
    fn validates_empty_action_id() {
        let result = NotificationOptions::new("Title")
            .action(NotificationAction::new("", "Open"))
            .validate(true);

        assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
    }

    #[test]
    fn validates_empty_action_label() {
        let result = NotificationOptions::new("Title")
            .action(NotificationAction::new("open", ""))
            .validate(true);

        assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
    }

    #[test]
    fn validates_action_limit() {
        let result = NotificationOptions::new("Title")
            .action(NotificationAction::new("one", "One"))
            .action(NotificationAction::new("two", "Two"))
            .action(NotificationAction::new("three", "Three"))
            .validate(true);

        assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
    }

    #[test]
    fn sanitizes_windows_shortcut_file_names() {
        assert_eq!(
            super::sanitize_windows_shortcut_file_name("com:tgui/demo?"),
            "com_tgui_demo_"
        );
        assert_eq!(super::sanitize_windows_shortcut_file_name("."), "tgui");
    }

    #[derive(Default)]
    struct TestVm {
        value: Arc<Mutex<Option<String>>>,
    }

    impl ViewModel for TestVm {
        fn new(_context: &ViewModelContext) -> Self {
            Self::default()
        }

        fn view(&self) -> Element<Self> {
            unimplemented!()
        }
    }

    #[test]
    fn dispatches_completion_to_value_command() {
        let (dispatcher, receiver) = async_notification_channel();
        let mut vm = TestVm::default();
        let state = vm.value.clone();
        let command = ValueCommand::new(|vm: &mut TestVm, value: String| {
            *vm.value.lock().expect("state lock poisoned") = Some(value);
        });

        dispatcher
            .dispatch(PendingNotificationCompletion {
                window_key: "main".to_string(),
                window_instance_id: 1,
                callback: Box::new(move |view_model, context| {
                    command.execute_with_context(view_model, "clicked".to_string(), context);
                }),
            })
            .expect("completion should dispatch");

        let completion = receiver
            .try_iter()
            .next()
            .expect("completion should be queued");
        (completion.callback)(&mut vm, &CommandContext::detached());

        assert_eq!(
            state.lock().expect("state lock poisoned").as_deref(),
            Some("clicked")
        );
    }
}
