//! Android JNI 桥接：装载 `bridge.dex`、注册 native 回调、管理 requestId 槽位。
//!
//! 仅在 `target_os = "android"` 编译。`winit-android` 直接依赖；这里不依赖 tgui 的
//! `feature = "android"`，以便桌面 `cargo check` 时不被牵连。

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use jni::errors::Error as JniError;
use jni::objects::{Global, JByteBuffer, JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jint, jlong, jobject, JavaVM as RawJavaVM};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM, NativeMethod};

use winit_android::activity::AndroidApp;

use crate::dialog::types::{DialogError, MessageDialogButtons, MessageDialogResult};

const DEX_BYTES: &[u8] = include_bytes!("bridge.dex");

// `Class.forName` 用点分名，`InMemoryDexClassLoader.loadClass` 同样接受点分名。
const BRIDGE_CLASS_NAME: &str = "com.tgui.TguiDialogBridge";

// 与 Java 端常量对齐。
const BUTTON_OK: jint = 1;
const BUTTON_CANCEL: jint = 2;
const BUTTON_YES: jint = 3;
const BUTTON_NO: jint = 4;

const BUTTONS_OK: jint = 0;
const BUTTONS_OK_CANCEL: jint = 1;
const BUTTONS_YES_NO: jint = 2;
const BUTTONS_YES_NO_CANCEL: jint = 3;

pub(crate) const FILE_OPEN: jint = 0;
pub(crate) const FILE_OPEN_MULTI: jint = 1;
pub(crate) const FILE_PICK_FOLDER: jint = 2;
pub(crate) const FILE_SAVE: jint = 3;

/// 桥接回调把 JNI 结果转成 Rust 枚举送回 Rust 调度层。
pub(crate) enum BridgeResult {
    Message(MessageDialogResult),
    File { ok: bool, uris: Vec<String> },
}

type Slot = Box<dyn FnOnce(BridgeResult) + Send>;

static SLOTS: OnceLock<Mutex<HashMap<u64, Slot>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static STATE: OnceLock<BridgeState> = OnceLock::new();

struct BridgeState {
    vm: JavaVM,
    activity: Global<JObject<'static>>,
    bridge_class: Global<JClass<'static>>,
}

// `BridgeState` 内部全是 `JavaVM` + `Global<…>`，本身已经满足 Send/Sync。
// 用 `OnceLock` 静态存储不需要额外 unsafe impl。

fn slots() -> &'static Mutex<HashMap<u64, Slot>> {
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn alloc_request_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn install_slot(id: u64, slot: Slot) {
    slots()
        .lock()
        .expect("dialog slots poisoned")
        .insert(id, slot);
}

fn take_slot(id: u64) -> Option<Slot> {
    slots().lock().expect("dialog slots poisoned").remove(&id)
}

/// 当 JNI 调度失败时清理已分配但永远不会被回调触发的槽位。
pub(crate) fn discard_slot(id: u64) {
    let _ = take_slot(id);
}

pub(crate) fn map_buttons_kind(buttons: MessageDialogButtons) -> jint {
    match buttons {
        MessageDialogButtons::Ok => BUTTONS_OK,
        MessageDialogButtons::OkCancel => BUTTONS_OK_CANCEL,
        MessageDialogButtons::YesNo => BUTTONS_YES_NO,
        MessageDialogButtons::YesNoCancel => BUTTONS_YES_NO_CANCEL,
    }
}

fn map_message_button(which: jint) -> MessageDialogResult {
    match which {
        BUTTON_OK => MessageDialogResult::Ok,
        BUTTON_YES => MessageDialogResult::Yes,
        BUTTON_NO => MessageDialogResult::No,
        BUTTON_CANCEL => MessageDialogResult::Cancel,
        _ => MessageDialogResult::Cancel,
    }
}

/// 在 Android 主线程 / runtime 启动时调用，幂等地装载 dex 并注册 native 方法。
pub(crate) fn install_android_app(app: &AndroidApp) -> Result<(), DialogError> {
    if STATE.get().is_some() {
        return Ok(());
    }

    let vm_raw = app.vm_as_ptr() as *mut RawJavaVM;
    if vm_raw.is_null() {
        return Err(DialogError::Backend(
            "AndroidApp::vm_as_ptr returned null".to_string(),
        ));
    }
    // `AndroidApp::activity_as_ptr()` 返回的就是 Activity 的 JNI 全局引用（jobject），
    // 不是 `ANativeActivity*`，所以这里直接当 jobject 使用。
    let activity_jobject = app.activity_as_ptr() as jobject;
    if activity_jobject.is_null() {
        return Err(DialogError::Backend(
            "AndroidApp::activity_as_ptr returned null".to_string(),
        ));
    }

    // SAFETY: `vm_raw` 由 winit-android 提供，是有效的 JavaVM 指针。
    let vm = unsafe { JavaVM::from_raw(vm_raw) };

    let state = vm.attach_current_thread(|env| -> Result<BridgeState, DialogError> {
        // SAFETY: `activity_jobject` 本身就是 JNI 全局引用，借用一个 JObject<'local> 包装它
        // 只是为了用 jni-rs 的 API 创建我们自己的 global ref；JObject 是 #[repr(transparent)]
        // 的零代价包装，无 Drop 副作用，因此不会破坏借来的全局引用。
        let activity_borrowed = unsafe { JObject::from_raw(env, activity_jobject) };
        let activity_global = env.new_global_ref(&activity_borrowed)?;

        let class_global = load_bridge_class(&mut *env, &activity_borrowed)?;
        register_natives(&mut *env, &class_global)?;

        Ok(BridgeState {
            vm: vm.clone(),
            activity: activity_global,
            bridge_class: class_global,
        })
    })?;

    // 多线程下可能并发 install，用 `set` 而不是 `get_or_init` 才能在丢失竞争时丢弃备用 state。
    let _ = STATE.set(state);
    Ok(())
}

fn jni_to_dialog_err(err: JniError) -> DialogError {
    DialogError::Backend(format!("jni error: {err}"))
}

fn load_bridge_class<'local>(
    env: &mut Env<'local>,
    activity_local: &JObject<'local>,
) -> Result<Global<JClass<'static>>, DialogError> {
    // 用 `InMemoryDexClassLoader(ByteBuffer, ClassLoader)` 加载库内 dex。
    let buffer: JByteBuffer<'local> =
        unsafe { env.new_direct_byte_buffer(DEX_BYTES.as_ptr() as *mut u8, DEX_BYTES.len()) }
            .map_err(jni_to_dialog_err)?;

    let parent_loader = env
        .call_method(
            activity_local,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .and_then(|v| v.l())
        .map_err(jni_to_dialog_err)?;

    let dex_loader = env
        .new_object(
            jni_str!("dalvik.system.InMemoryDexClassLoader"),
            jni_sig!("(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V"),
            &[JValue::Object(&buffer), JValue::Object(&parent_loader)],
        )
        .map_err(jni_to_dialog_err)?;

    let class_name = env
        .new_string(BRIDGE_CLASS_NAME)
        .map_err(jni_to_dialog_err)?;
    let bridge_class_obj = env
        .call_method(
            &dex_loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )
        .and_then(|v| v.l())
        .map_err(jni_to_dialog_err)?;

    let bridge_class = env.cast_local::<JClass>(bridge_class_obj)?;
    Ok(env.new_global_ref(&bridge_class)?)
}

fn register_natives<'local>(
    env: &mut Env<'local>,
    bridge_class: &Global<JClass<'static>>,
) -> Result<(), DialogError> {
    const METHODS: &[NativeMethod<'static>] = &[
        // SAFETY: 函数签名与 Java 端 native 声明一致。
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("onMessageResult"),
                jni_str!("(JI)V"),
                native_on_message_result as *mut c_void,
            )
        },
        // SAFETY: 同上。
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("onFileResult"),
                jni_str!("(JI[Ljava/lang/String;)V"),
                native_on_file_result as *mut c_void,
            )
        },
    ];

    // SAFETY: METHODS 中的函数指针、签名都由本模块控制，与桥接类匹配。
    unsafe { env.register_native_methods(bridge_class, METHODS) }?;
    Ok(())
}

fn ensure_state() -> Result<&'static BridgeState, DialogError> {
    STATE.get().ok_or_else(|| {
        DialogError::Backend(
            "dialog bridge not initialized; runtime must call install_android_app first"
                .to_string(),
        )
    })
}

/// 触发 Android `AlertDialog`。回调时把 `MessageDialogResult` 送进 slot。
pub(crate) fn dispatch_message(
    request_id: u64,
    title: Option<String>,
    description: Option<String>,
    buttons: MessageDialogButtons,
) -> Result<(), DialogError> {
    let state = ensure_state()?;
    state
        .vm
        .attach_current_thread(|env| -> Result<(), DialogError> {
            let title_j = optional_string(env, title.as_deref())?;
            let body_j = optional_string(env, description.as_deref())?;
            env.call_static_method(
                &state.bridge_class,
                jni_str!("showMessageDialog"),
                jni_sig!("(Landroid/app/Activity;JLjava/lang/String;Ljava/lang/String;I)V"),
                &[
                    JValue::Object(state.activity.as_ref()),
                    JValue::Long(request_id as jlong),
                    JValue::Object(&title_j),
                    JValue::Object(&body_j),
                    JValue::Int(map_buttons_kind(buttons)),
                ],
            )
            .map_err(jni_to_dialog_err)?;
            Ok(())
        })
}

/// 触发 Android SAF 文件 / 目录选择。
pub(crate) fn dispatch_file(
    request_id: u64,
    request_kind: jint,
    title: Option<String>,
    suggested_file_name: Option<String>,
    mime_types: Vec<String>,
) -> Result<(), DialogError> {
    let state = ensure_state()?;
    state
        .vm
        .attach_current_thread(|env| -> Result<(), DialogError> {
            let title_j = optional_string(env, title.as_deref())?;
            let suggested_j = optional_string(env, suggested_file_name.as_deref())?;
            let mimes_j = string_array(env, &mime_types)?;
            env.call_static_method(
                &state.bridge_class,
                jni_str!("startFileDialog"),
                jni_sig!(
                    "(Landroid/app/Activity;JILjava/lang/String;Ljava/lang/String;[Ljava/lang/String;)V"
                ),
                &[
                    JValue::Object(state.activity.as_ref()),
                    JValue::Long(request_id as jlong),
                    JValue::Int(request_kind),
                    JValue::Object(&title_j),
                    JValue::Object(&suggested_j),
                    JValue::Object(&mimes_j),
                ],
            )
            .map_err(jni_to_dialog_err)?;
            Ok(())
        })
}

fn optional_string<'local>(
    env: &mut Env<'local>,
    value: Option<&str>,
) -> Result<JObject<'local>, DialogError> {
    match value {
        Some(s) => env
            .new_string(s)
            .map(|jstr| jstr.into())
            .map_err(jni_to_dialog_err),
        None => Ok(JObject::null()),
    }
}

fn string_array<'local>(
    env: &mut Env<'local>,
    values: &[String],
) -> Result<JObject<'local>, DialogError> {
    let placeholder = JString::new(env, "").map_err(jni_to_dialog_err)?;
    let array: JObjectArray<'local, JString<'local>> =
        JObjectArray::<JString>::new(env, values.len(), &placeholder).map_err(jni_to_dialog_err)?;
    for (i, value) in values.iter().enumerate() {
        let element = JString::new(env, value).map_err(jni_to_dialog_err)?;
        array
            .set_element(env, i, &element)
            .map_err(jni_to_dialog_err)?;
    }
    Ok(array.into())
}

// 由 JVM 通过 RegisterNatives 调用，运行在 UI 线程（Java 端用 runOnUiThread 触发）。
extern "system" fn native_on_message_result<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    request_id: jlong,
    which: jint,
) {
    let _ = unowned_env.with_env(|_env| -> Result<(), JniError> {
        if let Some(slot) = take_slot(request_id as u64) {
            slot(BridgeResult::Message(map_message_button(which)));
        }
        Ok(())
    });
}

extern "system" fn native_on_file_result<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    request_id: jlong,
    result_code: jint,
    uris: JObjectArray<'local>,
) {
    let _ = unowned_env.with_env(|env| -> Result<(), JniError> {
        let ok = result_code == -1; // android.app.Activity.RESULT_OK
        let mut collected: Vec<String> = Vec::new();
        if !uris.is_null() {
            let arr = env.cast_local::<JObjectArray<JString>>(uris)?;
            let len = arr.len(env)?;
            for i in 0..len {
                let jstring = arr.get_element(env, i)?;
                if !jstring.is_null() {
                    collected.push(jstring.try_to_string(env)?);
                }
            }
        }
        if let Some(slot) = take_slot(request_id as u64) {
            slot(BridgeResult::File {
                ok,
                uris: collected,
            });
        }
        Ok(())
    });
}
