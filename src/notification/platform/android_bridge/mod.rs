//! Android 通知桥接：装载 `bridge.dex`、注册 native 回调、管理权限与 action 槽位。

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use jni::errors::Error as JniError;
use jni::objects::{Global, JByteBuffer, JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jint, jlong, jobject, JavaVM as RawJavaVM};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM, NativeMethod};
use winit_android::activity::AndroidApp;

use crate::notification::platform::{NotificationActionHandler, PermissionCallback};
use crate::notification::types::{NotificationError, NotificationPermission};

const DEX_BYTES: &[u8] = include_bytes!("bridge.dex");
const BRIDGE_CLASS_NAME: &str = "com.tgui.TguiNotificationBridge";

const PERMISSION_NOT_DETERMINED: jint = 0;
const PERMISSION_GRANTED: jint = 1;
const PERMISSION_DENIED: jint = 2;

pub(crate) struct AndroidNotificationRequest {
    pub(crate) app_id: String,
    pub(crate) notification_id: String,
    pub(crate) channel_name: String,
    pub(crate) title: String,
    pub(crate) body: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) sound: bool,
    pub(crate) action_callback_id: Option<u64>,
    pub(crate) action_ids: Vec<String>,
    pub(crate) action_labels: Vec<String>,
}

type ActionSlot = NotificationActionHandler;
type PermissionSlot = PermissionCallback;

static ACTION_SLOTS: OnceLock<Mutex<HashMap<u64, ActionSlot>>> = OnceLock::new();
static PERMISSION_SLOTS: OnceLock<Mutex<HashMap<u64, PermissionSlot>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static STATE: OnceLock<BridgeState> = OnceLock::new();

struct BridgeState {
    vm: JavaVM,
    activity: Global<JObject<'static>>,
    bridge_class: Global<JClass<'static>>,
}

fn action_slots() -> &'static Mutex<HashMap<u64, ActionSlot>> {
    ACTION_SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn permission_slots() -> &'static Mutex<HashMap<u64, PermissionSlot>> {
    PERMISSION_SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn alloc_request_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn install_action_slot(slot: ActionSlot) -> u64 {
    let id = alloc_request_id();
    action_slots()
        .lock()
        .expect("notification action slots poisoned")
        .insert(id, slot);
    id
}

pub(crate) fn discard_action_slot(id: u64) {
    let _ = action_slots()
        .lock()
        .expect("notification action slots poisoned")
        .remove(&id);
}

fn install_permission_slot(slot: PermissionSlot) -> u64 {
    let id = alloc_request_id();
    permission_slots()
        .lock()
        .expect("notification permission slots poisoned")
        .insert(id, slot);
    id
}

fn take_action_slot(id: u64) -> Option<ActionSlot> {
    action_slots()
        .lock()
        .expect("notification action slots poisoned")
        .remove(&id)
}

fn take_permission_slot(id: u64) -> Option<PermissionSlot> {
    permission_slots()
        .lock()
        .expect("notification permission slots poisoned")
        .remove(&id)
}

pub(crate) fn install_android_app(app: &AndroidApp) -> Result<(), NotificationError> {
    if STATE.get().is_some() {
        return Ok(());
    }

    let vm_raw = app.vm_as_ptr() as *mut RawJavaVM;
    if vm_raw.is_null() {
        return Err(NotificationError::Backend(
            "AndroidApp::vm_as_ptr returned null".to_string(),
        ));
    }
    let activity_jobject = app.activity_as_ptr() as jobject;
    if activity_jobject.is_null() {
        return Err(NotificationError::Backend(
            "AndroidApp::activity_as_ptr returned null".to_string(),
        ));
    }

    let vm = unsafe { JavaVM::from_raw(vm_raw) };
    let state = vm.attach_current_thread(|env| -> Result<BridgeState, NotificationError> {
        let activity_borrowed = unsafe { JObject::from_raw(env, activity_jobject) };
        let activity_global = env.new_global_ref(&activity_borrowed)?;
        let class_global = load_bridge_class(&mut *env, &activity_borrowed)?;
        register_natives(&mut *env, &class_global)?;
        env.call_static_method(
            &class_global,
            jni_str!("install"),
            jni_sig!("(Landroid/app/Activity;)V"),
            &[JValue::Object(&activity_borrowed)],
        )
        .map_err(jni_to_notification_err)?;
        Ok(BridgeState {
            vm: vm.clone(),
            activity: activity_global,
            bridge_class: class_global,
        })
    })?;

    let _ = STATE.set(state);
    Ok(())
}

pub(crate) fn send_notification(
    request: AndroidNotificationRequest,
) -> Result<(), NotificationError> {
    let state = ensure_state()?;
    state
        .vm
        .attach_current_thread(|env| -> Result<(), NotificationError> {
            let app_id = env.new_string(&request.app_id).map_err(jni_to_notification_err)?;
            let notification_id = env
                .new_string(&request.notification_id)
                .map_err(jni_to_notification_err)?;
            let channel_name = env
                .new_string(&request.channel_name)
                .map_err(jni_to_notification_err)?;
            let title = env
                .new_string(&request.title)
                .map_err(jni_to_notification_err)?;
            let body = optional_string(env, request.body.as_deref())?;
            let subtitle = optional_string(env, request.subtitle.as_deref())?;
            let icon = optional_string(env, request.icon.as_deref())?;
            let action_ids = string_array(env, &request.action_ids)?;
            let action_labels = string_array(env, &request.action_labels)?;
            env.call_static_method(
                &state.bridge_class,
                jni_str!("sendNotification"),
                jni_sig!(
                    "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZJ[Ljava/lang/String;[Ljava/lang/String;)V"
                ),
                &[
                    JValue::Object(state.activity.as_ref()),
                    JValue::Object(&app_id),
                    JValue::Object(&notification_id),
                    JValue::Object(&channel_name),
                    JValue::Object(&title),
                    JValue::Object(&body),
                    JValue::Object(&subtitle),
                    JValue::Object(&icon),
                    JValue::Bool(request.sound.into()),
                    JValue::Long(request.action_callback_id.unwrap_or_default() as jlong),
                    JValue::Object(&action_ids),
                    JValue::Object(&action_labels),
                ],
            )
            .map_err(jni_to_notification_err)?;
            Ok(())
        })
}

pub(crate) fn request_permission(callback: PermissionCallback) -> Result<(), NotificationError> {
    let state = ensure_state()?;
    let request_id = install_permission_slot(callback);
    state
        .vm
        .attach_current_thread(|env| -> Result<(), NotificationError> {
            env.call_static_method(
                &state.bridge_class,
                jni_str!("requestPermission"),
                jni_sig!("(Landroid/app/Activity;J)V"),
                &[
                    JValue::Object(state.activity.as_ref()),
                    JValue::Long(request_id as jlong),
                ],
            )
            .map_err(jni_to_notification_err)?;
            Ok(())
        })
        .inspect_err(|_| {
            let _ = take_permission_slot(request_id);
        })
}

pub(crate) fn permission_status() -> Result<NotificationPermission, NotificationError> {
    let state = ensure_state()?;
    state
        .vm
        .attach_current_thread(|env| -> Result<NotificationPermission, NotificationError> {
            let status = env
                .call_static_method(
                    &state.bridge_class,
                    jni_str!("permissionStatus"),
                    jni_sig!("(Landroid/app/Activity;)I"),
                    &[JValue::Object(state.activity.as_ref())],
                )
                .and_then(|value| value.i())
                .map_err(jni_to_notification_err)?;
            Ok(permission_from_raw(status))
        })
}

fn ensure_state() -> Result<&'static BridgeState, NotificationError> {
    STATE.get().ok_or_else(|| {
        NotificationError::Backend(
            "notification bridge not initialized; runtime must call install_android_app first"
                .to_string(),
        )
    })
}

fn jni_to_notification_err(err: JniError) -> NotificationError {
    NotificationError::Backend(format!("jni error: {err}"))
}

fn load_bridge_class<'local>(
    env: &mut Env<'local>,
    activity_local: &JObject<'local>,
) -> Result<Global<JClass<'static>>, NotificationError> {
    let buffer: JByteBuffer<'local> =
        unsafe { env.new_direct_byte_buffer(DEX_BYTES.as_ptr() as *mut u8, DEX_BYTES.len()) }
            .map_err(jni_to_notification_err)?;

    let parent_loader = env
        .call_method(
            activity_local,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .and_then(|value| value.l())
        .map_err(jni_to_notification_err)?;

    let dex_loader = env
        .new_object(
            jni_str!("dalvik.system.InMemoryDexClassLoader"),
            jni_sig!("(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V"),
            &[JValue::Object(&buffer), JValue::Object(&parent_loader)],
        )
        .map_err(jni_to_notification_err)?;

    let class_name = env
        .new_string(BRIDGE_CLASS_NAME)
        .map_err(jni_to_notification_err)?;
    let bridge_class_obj = env
        .call_method(
            &dex_loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )
        .and_then(|value| value.l())
        .map_err(jni_to_notification_err)?;
    let bridge_class = env.cast_local::<JClass>(bridge_class_obj)?;
    Ok(env.new_global_ref(&bridge_class)?)
}

fn register_natives<'local>(
    env: &mut Env<'local>,
    bridge_class: &Global<JClass<'static>>,
) -> Result<(), NotificationError> {
    const METHODS: &[NativeMethod<'static>] = &[
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("onNotificationAction"),
                jni_str!("(JLjava/lang/String;)V"),
                native_on_notification_action as *mut c_void,
            )
        },
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("onPermissionResult"),
                jni_str!("(JI)V"),
                native_on_permission_result as *mut c_void,
            )
        },
    ];

    unsafe { env.register_native_methods(bridge_class, METHODS) }?;
    Ok(())
}

fn permission_from_raw(value: jint) -> NotificationPermission {
    match value {
        PERMISSION_GRANTED => NotificationPermission::Granted,
        PERMISSION_DENIED => NotificationPermission::Denied,
        PERMISSION_NOT_DETERMINED => NotificationPermission::NotDetermined,
        _ => NotificationPermission::Denied,
    }
}

fn optional_string<'local>(
    env: &mut Env<'local>,
    value: Option<&str>,
) -> Result<JObject<'local>, NotificationError> {
    match value {
        Some(value) => env
            .new_string(value)
            .map(|string| string.into())
            .map_err(jni_to_notification_err),
        None => Ok(JObject::null()),
    }
}

fn string_array<'local>(
    env: &mut Env<'local>,
    values: &[String],
) -> Result<JObject<'local>, NotificationError> {
    let placeholder = JString::new(env, "").map_err(jni_to_notification_err)?;
    let array: JObjectArray<'local, JString<'local>> =
        JObjectArray::<JString>::new(env, values.len(), &placeholder)
            .map_err(jni_to_notification_err)?;
    for (index, value) in values.iter().enumerate() {
        let element = JString::new(env, value).map_err(jni_to_notification_err)?;
        array
            .set_element(env, index, &element)
            .map_err(jni_to_notification_err)?;
    }
    Ok(array.into())
}

extern "system" fn native_on_notification_action<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    callback_id: jlong,
    action_id: JString<'local>,
) {
    let _ = unowned_env.with_env(|env| -> Result<(), JniError> {
        let action_id = action_id.try_to_string(env)?;
        if let Some(callback) = take_action_slot(callback_id as u64) {
            callback(action_id);
        }
        Ok(())
    });
}

extern "system" fn native_on_permission_result<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    request_id: jlong,
    status: jint,
) {
    let _ = unowned_env.with_env(|_env| -> Result<(), JniError> {
        if let Some(callback) = take_permission_slot(request_id as u64) {
            callback(Ok(permission_from_raw(status)));
        }
        Ok(())
    });
}
