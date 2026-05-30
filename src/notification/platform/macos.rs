use std::process::Command;

use super::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

/// macOS 通知后端。
///
/// 现代的 `UNUserNotificationCenter` API 要求进程运行在一个已签名的 `.app`
/// bundle 中；从裸二进制(例如 `cargo run`)直接调用会以
/// `bundleProxyForCurrentProcess is nil` 崩溃。因此这里采用与 Linux 后端
/// (notify-rust + notify-send)相同的“原生优先、CLI 兜底”策略:
///
/// - 运行在 bundle 中:走原生 `UNUserNotificationCenter`(支持权限、声音、
///   交互动作回调)。
/// - 未打包(裸二进制):退回到 `osascript display notification`,只展示横幅,
///   不支持动作按钮回调。
pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    if native::is_supported() {
        return native::send(options, app_id, on_action);
    }
    fallback_send(options, on_action)
}

pub(crate) fn platform_request_permission(
    callback: PermissionCallback,
) -> Result<(), NotificationError> {
    if native::is_supported() {
        return native::request_permission(callback);
    }
    // osascript 走 Script Editor 的通知身份,无需单独申请权限。
    callback(Ok(NotificationPermission::Granted));
    Ok(())
}

pub(crate) fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    if native::is_supported() {
        return native::permission_status();
    }
    Ok(NotificationPermission::Granted)
}

/// 未打包二进制的兜底:用 `osascript` 展示横幅通知。
///
/// `display notification` 无法呈现动作按钮,也无法回传被点击的动作,因此交互
/// 回调在这里会被丢弃(优雅降级)。该路径只在裸二进制(`cargo run`)下触发。
fn fallback_send(
    options: NotificationOptions,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let _ = on_action;

    let mut script = String::from("display notification ");
    script.push_str(&applescript_string(options.body_text().unwrap_or("")));
    script.push_str(" with title ");
    script.push_str(&applescript_string(options.title()));
    if let Some(subtitle) = options.subtitle_text() {
        script.push_str(" subtitle ");
        script.push_str(&applescript_string(subtitle));
    }

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|error| {
            NotificationError::Backend(format!("failed to launch osascript: {error}"))
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
        "osascript notification failed: {detail}"
    )))
}

/// 把任意字符串转义为安全的 AppleScript 字符串字面量,避免脚本注入。
pub(crate) fn applescript_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

mod native {
    use std::collections::HashMap;
    use std::ptr::NonNull;
    use std::sync::{mpsc, Mutex, OnceLock};
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AnyThread};
    use objc2_foundation::{NSArray, NSBundle, NSError, NSSet, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotification, UNNotificationAction, UNNotificationActionOptions, UNNotificationCategory,
        UNNotificationCategoryOptions, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    use super::{NotificationActionHandler, PermissionCallback};
    use crate::notification::types::{
        NotificationError, NotificationOptions, NotificationPermission,
    };

    /// 系统保留的“默认动作”(点击通知本体)标识。
    const DEFAULT_ACTION_IDENTIFIER: &str = "com.apple.UNNotificationDefaultActionIdentifier";
    /// 系统保留的“忽略动作”(划走/关闭通知)标识。
    const DISMISS_ACTION_IDENTIFIER: &str = "com.apple.UNNotificationDismissActionIdentifier";

    /// 判断当前进程是否运行在 `.app` bundle 中。
    ///
    /// `UNUserNotificationCenter` 在没有 bundle 身份时会直接崩溃,而裸二进制的
    /// `mainBundle().bundleIdentifier()` 返回 `nil`,因此用它作为可靠信号。
    pub(crate) fn is_supported() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    pub(crate) fn send(
        options: NotificationOptions,
        _app_id: Option<&str>,
        on_action: Option<NotificationActionHandler>,
    ) -> Result<(), NotificationError> {
        // macOS 的通知身份来自 bundle,app_id 在此无用。
        let center = UNUserNotificationCenter::currentNotificationCenter();
        ensure_delegate(&center);

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(options.title()));
        if let Some(subtitle) = options.subtitle_text() {
            content.setSubtitle(&NSString::from_str(subtitle));
        }
        if let Some(body) = options.body_text() {
            content.setBody(&NSString::from_str(body));
        }
        if options.sound_enabled() {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }

        // platform_send 之前一定调用过 ensure_id,这里仍保留兜底。
        let identifier = options
            .notification_id()
            .map(str::to_string)
            .unwrap_or_default();

        if !options.action_items().is_empty() {
            let category_id = format!("{identifier}.category");
            register_actions(&center, &category_id, &options);
            content.setCategoryIdentifier(&NSString::from_str(&category_id));
            if let Some(handler) = on_action {
                store_handler(identifier.clone(), handler);
            }
        }

        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&identifier),
            &content,
            None,
        );

        // 先申请权限再投递。已决定过权限时该调用会立刻 resolve,不会再弹窗。
        let center_for_block = center.clone();
        let auth_block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            if granted.as_bool() {
                center_for_block.addNotificationRequest_withCompletionHandler(&request, None);
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(
            authorization_options(),
            &auth_block,
        );

        Ok(())
    }

    pub(crate) fn request_permission(
        callback: PermissionCallback,
    ) -> Result<(), NotificationError> {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // 完成回调在后台队列触发一次;用 Mutex<Option<..>> 把 FnOnce 适配成 Fn。
        let callback = Mutex::new(Some(callback));
        let block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let result = if granted.as_bool() {
                Ok(NotificationPermission::Granted)
            } else if let Some(error) = unsafe { error.as_ref() } {
                Err(NotificationError::Backend(
                    error.localizedDescription().to_string(),
                ))
            } else {
                Ok(NotificationPermission::Denied)
            };
            if let Some(callback) = callback
                .lock()
                .expect("notification permission callback lock poisoned")
                .take()
            {
                callback(result);
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(authorization_options(), &block);
        Ok(())
    }

    pub(crate) fn permission_status() -> Result<NotificationPermission, NotificationError> {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let (sender, receiver) = mpsc::channel();
        // getNotificationSettings 的完成回调在后台队列执行,主线程阻塞等待不会死锁。
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            let permission = if status == UNAuthorizationStatus::Authorized
                || status == UNAuthorizationStatus::Provisional
                || status == UNAuthorizationStatus::Ephemeral
            {
                NotificationPermission::Granted
            } else if status == UNAuthorizationStatus::Denied {
                NotificationPermission::Denied
            } else {
                NotificationPermission::NotDetermined
            };
            let _ = sender.send(permission);
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
        receiver.recv_timeout(Duration::from_secs(2)).map_err(|_| {
            NotificationError::Backend("timed out reading macOS notification settings".to_string())
        })
    }

    fn authorization_options() -> UNAuthorizationOptions {
        UNAuthorizationOptions::Alert
            | UNAuthorizationOptions::Sound
            | UNAuthorizationOptions::Badge
    }

    fn register_actions(
        center: &UNUserNotificationCenter,
        category_id: &str,
        options: &NotificationOptions,
    ) {
        let actions: Vec<Retained<UNNotificationAction>> = options
            .action_items()
            .iter()
            .map(|action| {
                UNNotificationAction::actionWithIdentifier_title_options(
                    &NSString::from_str(action.id()),
                    &NSString::from_str(action.label()),
                    UNNotificationActionOptions::Foreground,
                )
            })
            .collect();
        let action_refs: Vec<&UNNotificationAction> = actions.iter().map(|a| &**a).collect();
        let actions_array = NSArray::from_slice(&action_refs);
        let intents = NSArray::<NSString>::from_slice(&[]);
        let category =
            UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                &NSString::from_str(category_id),
                &actions_array,
                &intents,
                UNNotificationCategoryOptions::empty(),
            );
        let categories = NSSet::from_slice(&[&*category]);
        center.setNotificationCategories(&categories);
    }

    fn handler_registry() -> &'static Mutex<HashMap<String, NotificationActionHandler>> {
        static REGISTRY: OnceLock<Mutex<HashMap<String, NotificationActionHandler>>> =
            OnceLock::new();
        REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn store_handler(notification_id: String, handler: NotificationActionHandler) {
        handler_registry()
            .lock()
            .expect("notification handler registry lock poisoned")
            .insert(notification_id, handler);
    }

    fn take_handler(notification_id: &str) -> Option<NotificationActionHandler> {
        handler_registry()
            .lock()
            .expect("notification handler registry lock poisoned")
            .remove(notification_id)
    }

    /// 委托是弱引用属性,必须在静态变量里保活;同时确保只创建并注册一次。
    fn ensure_delegate(center: &UNUserNotificationCenter) {
        static DELEGATE: OnceLock<Retained<Delegate>> = OnceLock::new();
        let delegate = DELEGATE.get_or_init(Delegate::new);
        center.setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
    }

    fn handle_response(response: &UNNotificationResponse) {
        let notification_id = response.notification().request().identifier().to_string();
        let action_id = response.actionIdentifier().to_string();
        let Some(handler) = take_handler(&notification_id) else {
            return;
        };
        // 默认(点击本体)与忽略动作不是用户定义的动作:移除句柄但不回调。
        if action_id == DEFAULT_ACTION_IDENTIFIER || action_id == DISMISS_ACTION_IDENTIFIER {
            return;
        }
        handler(action_id);
    }

    define_class!(
        // SAFETY:
        // - 父类 NSObject 没有子类化约束。
        // - Delegate 不实现 Drop。
        #[unsafe(super(NSObject))]
        #[name = "TguiNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                // 应用处于前台时也展示横幅并播放声音,否则通知只进通知中心。
                completion_handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive_response(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                handle_response(response);
                completion_handler.call(());
            }
        }
    );

    impl Delegate {
        fn new() -> Retained<Self> {
            let this = Self::alloc().set_ivars(());
            unsafe { msg_send![super(this), init] }
        }
    }
}
