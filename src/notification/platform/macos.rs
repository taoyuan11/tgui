use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSSet, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationAction, UNNotificationActionOptionForeground, UNNotificationCategory,
    UNNotificationCategoryOptionNone, UNNotificationDefaultActionIdentifier,
    UNNotificationDismissActionIdentifier, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use super::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationOptions, NotificationPermission};

#[derive(Default)]
struct MacNotificationDelegateIvars;

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = MacNotificationDelegateIvars]
    #[name = "TguiMacNotificationDelegate"]
    struct MacNotificationDelegate;

    unsafe impl NSObjectProtocol for MacNotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for MacNotificationDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn user_notification_center_will_present_notification_with_completion_handler(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn user_notification_center_did_receive_notification_response_with_completion_handler(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &DynBlock<dyn Fn()>,
        ) {
            let action_identifier = response.actionIdentifier().to_string();
            let notification_id = response.notification().request().identifier().to_string();

            if let Some(callback) = take_action_handler(&notification_id) {
                if action_identifier != UNNotificationDefaultActionIdentifier.to_string()
                    && action_identifier != UNNotificationDismissActionIdentifier.to_string()
                {
                    callback(action_identifier);
                }
            }

            completion_handler.call(());
        }
    }
);

impl MacNotificationDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(MacNotificationDelegateIvars);
        unsafe { msg_send![super(this), init] }
    }
}

fn action_handlers() -> &'static Mutex<HashMap<String, NotificationActionHandler>> {
    static ACTION_HANDLERS: OnceLock<Mutex<HashMap<String, NotificationActionHandler>>> =
        OnceLock::new();
    ACTION_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn notification_delegate() -> &'static Retained<ProtocolObject<dyn UNUserNotificationCenterDelegate>>
{
    static DELEGATE: OnceLock<Retained<ProtocolObject<dyn UNUserNotificationCenterDelegate>>> =
        OnceLock::new();
    DELEGATE.get_or_init(|| ProtocolObject::from_retained(MacNotificationDelegate::new()))
}

fn current_notification_center() -> Retained<UNUserNotificationCenter> {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.setDelegate(Some(&**notification_delegate()));
    center
}

fn permission_from_authorization_status(status: UNAuthorizationStatus) -> NotificationPermission {
    match status {
        UNAuthorizationStatus::Denied => NotificationPermission::Denied,
        UNAuthorizationStatus::Authorized
        | UNAuthorizationStatus::Provisional
        | UNAuthorizationStatus::Ephemeral => NotificationPermission::Granted,
        _ => NotificationPermission::NotDetermined,
    }
}

fn register_action_handler(notification_id: &str, handler: NotificationActionHandler) {
    action_handlers()
        .lock()
        .expect("macOS notification action handler lock poisoned")
        .insert(notification_id.to_string(), handler);
}

fn take_action_handler(notification_id: &str) -> Option<NotificationActionHandler> {
    action_handlers()
        .lock()
        .expect("macOS notification action handler lock poisoned")
        .remove(notification_id)
}

fn build_notification_content(
    options: &NotificationOptions,
) -> Retained<UNMutableNotificationContent> {
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
    if !options.action_items().is_empty() {
        let category_identifier = format!(
            "tgui.notification.category.{}",
            options
                .notification_id()
                .expect("notification id must be assigned before building category")
        );
        content.setCategoryIdentifier(&NSString::from_str(&category_identifier));
    }
    content
}

fn configure_notification_category(
    center: &UNUserNotificationCenter,
    options: &NotificationOptions,
) {
    if options.action_items().is_empty() {
        return;
    }

    let category_identifier = NSString::from_str(&format!(
        "tgui.notification.category.{}",
        options
            .notification_id()
            .expect("notification id must be assigned before configuring category")
    ));
    let actions: Vec<Retained<UNNotificationAction>> = options
        .action_items()
        .iter()
        .map(|action| {
            UNNotificationAction::actionWithIdentifier_title_options(
                &NSString::from_str(action.id()),
                &NSString::from_str(action.label()),
                UNNotificationActionOptionForeground,
            )
        })
        .collect();
    let actions = NSArray::from_retained_slice(&actions);
    let intent_identifiers = NSArray::from_slice(&[] as &[&NSString]);
    let category = UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
        &category_identifier,
        &actions,
        &intent_identifiers,
        UNNotificationCategoryOptionNone,
    );
    let categories = NSSet::from_retained_slice(&[category]);
    center.setNotificationCategories(&categories);
}

fn send_notification_request(
    center: &UNUserNotificationCenter,
    request: &UNNotificationRequest,
) -> Result<(), NotificationError> {
    let error_slot = std::sync::Arc::new(Mutex::new(None::<String>));
    let error_slot_for_block = error_slot.clone();
    let (sender, receiver) = mpsc::channel();
    let completion = RcBlock::new(move |error: *mut objc2_foundation::NSError| {
        if !error.is_null() {
            let description = unsafe { (&*error).localizedDescription().to_string() };
            *error_slot_for_block
                .lock()
                .expect("macOS notification completion error lock poisoned") = Some(description);
        }
        let _ = sender.send(());
    });
    center.addNotificationRequest_withCompletionHandler(request, Some(&completion));
    let _ = receiver.recv();
    if let Some(error) = error_slot
        .lock()
        .expect("macOS notification completion error lock poisoned")
        .take()
    {
        return Err(NotificationError::Backend(format!(
            "failed to deliver macOS notification: {error}"
        )));
    }
    Ok(())
}

pub(crate) fn platform_send(
    options: NotificationOptions,
    app_id: Option<&str>,
    on_action: Option<NotificationActionHandler>,
) -> Result<(), NotificationError> {
    let _ = app_id;
    let center = current_notification_center();
    let permission = platform_permission_status()?;
    if matches!(permission, NotificationPermission::Denied) {
        return Err(NotificationError::Backend(
            "macOS notification permission has been denied".to_string(),
        ));
    }

    if let (Some(notification_id), Some(handler)) = (options.notification_id(), on_action) {
        register_action_handler(notification_id, handler);
    }

    configure_notification_category(&center, &options);
    let content = build_notification_content(&options);
    let identifier = NSString::from_str(
        options
            .notification_id()
            .expect("notification id must be assigned before platform_send"),
    );
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);

    if let Err(error) = send_notification_request(&center, &request) {
        if let Some(notification_id) = options.notification_id() {
            let _ = take_action_handler(notification_id);
        }
        return Err(error);
    }

    Ok(())
}

pub(crate) fn platform_request_permission(
    callback: PermissionCallback,
) -> Result<(), NotificationError> {
    let center = current_notification_center();
    let callback = std::sync::Arc::new(Mutex::new(Some(callback)));
    let callback_for_block = callback.clone();
    let completion = RcBlock::new(
        move |granted: bool, error: *mut objc2_foundation::NSError| {
            let result = if !error.is_null() {
                let description = unsafe { (&*error).localizedDescription().to_string() };
                Err(NotificationError::Backend(format!(
                    "failed to request macOS notification permission: {description}"
                )))
            } else if granted {
                Ok(NotificationPermission::Granted)
            } else {
                Ok(NotificationPermission::Denied)
            };

            if let Some(callback) = callback_for_block
                .lock()
                .expect("macOS permission callback lock poisoned")
                .take()
            {
                callback(result);
            }
        },
    );
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert
            | UNAuthorizationOptions::Sound
            | UNAuthorizationOptions::Badge,
        &completion,
    );
    Ok(())
}

pub(crate) fn platform_permission_status() -> Result<NotificationPermission, NotificationError> {
    let center = current_notification_center();
    let (sender, receiver) = mpsc::channel();
    let completion = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
        let settings = unsafe { settings.as_ref() };
        let _ = sender.send(permission_from_authorization_status(
            settings.authorizationStatus(),
        ));
    });
    center.getNotificationSettingsWithCompletionHandler(&completion);
    receiver.recv().map_err(|_| {
        NotificationError::Backend(
            "failed to resolve macOS notification permission status".to_string(),
        )
    })
}
