#[cfg(all(target_os = "android", feature = "android"))]
use std::ffi::c_void;
#[cfg(all(target_os = "android", feature = "android"))]
use std::sync::{Mutex, OnceLock};

#[cfg(all(target_os = "android", feature = "android"))]
use jni::errors::Error as JniError;
#[cfg(all(target_os = "android", feature = "android"))]
use jni::objects::{Global, JByteBuffer, JClass, JObject, JString, JValue};
#[cfg(all(target_os = "android", feature = "android"))]
use jni::sys::{jint, jobject, JavaVM as RawJavaVM};
#[cfg(all(target_os = "android", feature = "android"))]
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM, NativeMethod};
#[cfg(all(target_os = "android", feature = "android"))]
use winit_android::activity::{AndroidApp, AndroidAppWaker};

#[cfg(all(target_os = "android", feature = "android"))]
use crate::foundation::binding::InvalidationSignal;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::foundation::binding::TextChange;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::log::Log;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::runtime::BoundRuntimeHandler;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::ui::widget::{CompositionState, TextEditState};

#[cfg(all(target_os = "android", feature = "android"))]
const DEX_BYTES: &[u8] = include_bytes!("android_text_input_bridge.dex");
#[cfg(all(target_os = "android", feature = "android"))]
const BRIDGE_CLASS_NAME: &str = "com.tgui.TguiTextInputBridge";

#[cfg(all(target_os = "android", feature = "android"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AndroidTextInputSnapshot {
    pub(crate) text: String,
    pub(crate) selection_start: usize,
    pub(crate) selection_end: usize,
    pub(crate) composing_start: Option<usize>,
    pub(crate) composing_end: Option<usize>,
}

#[cfg(all(target_os = "android", feature = "android"))]
struct BridgeState {
    vm: JavaVM,
    activity: Global<JObject<'static>>,
    bridge_class: Global<JClass<'static>>,
    waker: AndroidAppWaker,
}

#[cfg(all(target_os = "android", feature = "android"))]
static BRIDGE_STATE: OnceLock<BridgeState> = OnceLock::new();
#[cfg(all(target_os = "android", feature = "android"))]
static PENDING_SNAPSHOT: OnceLock<Mutex<Option<AndroidTextInputSnapshot>>> = OnceLock::new();
#[cfg(all(target_os = "android", feature = "android"))]
static LAST_SYNCED_SNAPSHOT: OnceLock<Mutex<Option<AndroidTextInputSnapshot>>> = OnceLock::new();
#[cfg(all(target_os = "android", feature = "android"))]
static INVALIDATION_SIGNAL: OnceLock<InvalidationSignal> = OnceLock::new();

#[cfg(all(target_os = "android", feature = "android"))]
fn pending_snapshot_slot() -> &'static Mutex<Option<AndroidTextInputSnapshot>> {
    PENDING_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

#[cfg(all(target_os = "android", feature = "android"))]
fn last_synced_snapshot_slot() -> &'static Mutex<Option<AndroidTextInputSnapshot>> {
    LAST_SYNCED_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

#[cfg(all(target_os = "android", feature = "android"))]
fn jni_to_string(err: JniError) -> String {
    format!("jni error: {err}")
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(crate) fn install_android_text_input_bridge(
    app: &AndroidApp,
    invalidation: &InvalidationSignal,
) -> Result<(), String> {
    if BRIDGE_STATE.get().is_some() {
        return Ok(());
    }
    let _ = INVALIDATION_SIGNAL.set(invalidation.clone());

    let vm_raw = app.vm_as_ptr() as *mut RawJavaVM;
    if vm_raw.is_null() {
        return Err("AndroidApp::vm_as_ptr returned null".to_string());
    }
    let activity_raw = app.activity_as_ptr() as jobject;
    if activity_raw.is_null() {
        return Err("AndroidApp::activity_as_ptr returned null".to_string());
    }

    let vm = unsafe { JavaVM::from_raw(vm_raw) };
    let state = vm
        .attach_current_thread(|env| -> Result<BridgeState, JniError> {
            let activity_borrowed = unsafe { JObject::from_raw(env, activity_raw) };
            let activity = env.new_global_ref(&activity_borrowed)?;
            let bridge_class = load_bridge_class(&mut *env, &activity_borrowed)?;
            register_natives(&mut *env, &bridge_class)?;
            env.call_static_method(
                &bridge_class,
                jni_str!("install"),
                jni_sig!("(Landroid/app/Activity;)V"),
                &[JValue::Object(&activity_borrowed)],
            )?;
            Ok(BridgeState {
                vm: vm.clone(),
                activity,
                bridge_class,
                waker: app.create_waker(),
            })
        })
        .map_err(jni_to_string)?;

    let _ = BRIDGE_STATE.set(state);
    Ok(())
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(crate) fn take_pending_android_text_input_snapshot() -> Option<AndroidTextInputSnapshot> {
    pending_snapshot_slot()
        .lock()
        .expect("android text input snapshot slot poisoned")
        .take()
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(crate) fn set_soft_input_active(active: bool) {
    if !active {
        *last_synced_snapshot_slot()
            .lock()
            .expect("android text input sync slot poisoned") = None;
    }
    let result = with_bridge_state(|state| {
        state
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                env.call_static_method(
                    &state.bridge_class,
                    jni_str!("setInputEnabled"),
                    jni_sig!("(Landroid/app/Activity;Z)V"),
                    &[
                        JValue::Object(state.activity.as_ref()),
                        JValue::Bool(active),
                    ],
                )?;
                Ok(())
            })
            .map_err(jni_to_string)
    });
    if let Err(error) = result {
        Log::with_tag("tgui-android-ime").warn(error);
    }
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(crate) fn sync_soft_input_state(snapshot: &AndroidTextInputSnapshot) {
    {
        let mut last_synced = last_synced_snapshot_slot()
            .lock()
            .expect("android text input sync slot poisoned");
        if last_synced.as_ref() == Some(snapshot) {
            return;
        }
        *last_synced = Some(snapshot.clone());
    }

    let result = with_bridge_state(|state| {
        state
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                let text = env.new_string(&snapshot.text)?;
                env.call_static_method(
                    &state.bridge_class,
                    jni_str!("syncState"),
                    jni_sig!("(Landroid/app/Activity;Ljava/lang/String;IIII)V"),
                    &[
                        JValue::Object(state.activity.as_ref()),
                        JValue::Object(&text),
                        JValue::Int(byte_index_to_utf16_index(
                            &snapshot.text,
                            snapshot.selection_start,
                        )),
                        JValue::Int(byte_index_to_utf16_index(
                            &snapshot.text,
                            snapshot.selection_end,
                        )),
                        JValue::Int(
                            snapshot
                                .composing_start
                                .map(|value| byte_index_to_utf16_index(&snapshot.text, value))
                                .unwrap_or(-1),
                        ),
                        JValue::Int(
                            snapshot
                                .composing_end
                                .map(|value| byte_index_to_utf16_index(&snapshot.text, value))
                                .unwrap_or(-1),
                        ),
                    ],
                )?;
                Ok(())
            })
            .map_err(jni_to_string)
    });
    if let Err(error) = result {
        Log::with_tag("tgui-android-ime").warn(error);
    }
}

#[cfg(all(target_os = "android", feature = "android"))]
fn with_bridge_state<T>(f: impl FnOnce(&BridgeState) -> Result<T, String>) -> Result<T, String> {
    let Some(state) = BRIDGE_STATE.get() else {
        return Err("android text input bridge not initialized".to_string());
    };
    f(state)
}

#[cfg(all(target_os = "android", feature = "android"))]
fn load_bridge_class<'local>(
    env: &mut Env<'local>,
    activity_local: &JObject<'local>,
) -> Result<Global<JClass<'static>>, JniError> {
    let buffer: JByteBuffer<'local> =
        unsafe { env.new_direct_byte_buffer(DEX_BYTES.as_ptr() as *mut u8, DEX_BYTES.len()) }?;

    let parent_loader = env
        .call_method(
            activity_local,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .and_then(|v| v.l())?;

    let dex_loader = env.new_object(
        jni_str!("dalvik.system.InMemoryDexClassLoader"),
        jni_sig!("(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V"),
        &[JValue::Object(&buffer), JValue::Object(&parent_loader)],
    )?;

    let class_name = env.new_string(BRIDGE_CLASS_NAME)?;
    let bridge_class_obj = env
        .call_method(
            &dex_loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )
        .and_then(|v| v.l())?;
    let bridge_class = env.cast_local::<JClass>(bridge_class_obj)?;
    env.new_global_ref(&bridge_class)
}

#[cfg(all(target_os = "android", feature = "android"))]
fn register_natives<'local>(
    env: &mut Env<'local>,
    bridge_class: &Global<JClass<'static>>,
) -> Result<(), JniError> {
    const METHODS: &[NativeMethod<'static>] = &[unsafe {
        NativeMethod::from_raw_parts(
            jni_str!("onTextChanged"),
            jni_str!("(Ljava/lang/String;IIII)V"),
            native_on_text_changed as *mut c_void,
        )
    }];

    unsafe { env.register_native_methods(bridge_class, METHODS) }
}

#[cfg(all(target_os = "android", feature = "android"))]
extern "system" fn native_on_text_changed<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
    selection_start: jint,
    selection_end: jint,
    composing_start: jint,
    composing_end: jint,
) {
    let _ = unowned_env.with_env(|env| -> Result<(), JniError> {
        let text = text.try_to_string(env)?;
        let snapshot = AndroidTextInputSnapshot {
            selection_start: utf16_index_to_byte_index(&text, selection_start.max(0) as usize),
            selection_end: utf16_index_to_byte_index(&text, selection_end.max(0) as usize),
            composing_start: (composing_start >= 0)
                .then_some(utf16_index_to_byte_index(&text, composing_start as usize)),
            composing_end: (composing_end >= 0)
                .then_some(utf16_index_to_byte_index(&text, composing_end as usize)),
            text,
        };
        *pending_snapshot_slot()
            .lock()
            .expect("android text input snapshot slot poisoned") = Some(snapshot);
        if let Some(signal) = INVALIDATION_SIGNAL.get() {
            signal.mark_dirty();
        }
        if let Some(state) = BRIDGE_STATE.get() {
            state.waker.wake();
        }
        Ok(())
    });
}

#[cfg(all(target_os = "android", feature = "android"))]
impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(crate) fn drain_android_text_input_snapshot(&mut self) -> bool {
        let Some(snapshot) = take_pending_android_text_input_snapshot() else {
            return false;
        };
        self.apply_android_text_input_snapshot(snapshot)
    }

    pub(crate) fn sync_android_text_input_state(&mut self) {
        let Some(widget_id) = self.focused_text_input_id() else {
            set_soft_input_active(false);
            return;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            set_soft_input_active(false);
            return;
        };
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value));
        set_soft_input_active(true);
        sync_soft_input_state(&runtime_snapshot(&current_value, &state));
    }

    fn apply_android_text_input_snapshot(&mut self, snapshot: AndroidTextInputSnapshot) -> bool {
        let Some(widget_id) = self.focused_text_input_id() else {
            return false;
        };
        let Some(region) = self.sync_text_input_buffer(widget_id) else {
            return false;
        };
        let current_value = self.text_input_current_value(widget_id, &region.controller);
        let previous_state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, &current_value));
        let (next_text, next_state) =
            snapshot_to_runtime_state(&snapshot, &current_value, &previous_state);
        let text_changed = next_text != current_value;
        let state_changed = next_state != previous_state;
        if !text_changed && !state_changed {
            return false;
        }

        {
            let session = self
                .text_input_buffers
                .get_mut(&widget_id)
                .expect("text input session should exist");
            if text_changed {
                if let Some((old_start, old_end, new_start, new_end)) =
                    replacement_bounds(&current_value, &next_text)
                {
                    session.push_pending_change(TextChange::new(
                        (old_start, old_end),
                        next_text[new_start..new_end].to_string(),
                    ));
                }
                session.current_text = next_text.clone();
                session.rope = ropey::Rope::from_str(&next_text);
            }
        }

        self.text_edit_states.insert(widget_id, next_state.clone());
        self.refresh_text_input_session_display(widget_id, &region, &next_text, &next_state);
        self.ensure_text_input_caret_visible(
            widget_id,
            region.frame,
            region.padding,
            &region.text_style,
            region.multiline,
            region.auto_wrap,
            region.show_scrollbar,
            &next_text,
            &next_state,
        );
        self.invalidate_text_input_scene();
        self.invalidation.mark_dirty();
        self.reset_caret_blink();
        self.sync_ime_state();
        true
    }
}

#[cfg(all(target_os = "android", feature = "android"))]
fn runtime_snapshot(text: &str, state: &TextEditState) -> AndroidTextInputSnapshot {
    if let Some(composition) = state.composition.as_ref() {
        let start = composition.replace_range.0.min(text.len());
        let end = composition.replace_range.1.min(text.len());
        let mut display =
            String::with_capacity(text.len() + composition.text.len().saturating_sub(end - start));
        display.push_str(&text[..start]);
        display.push_str(&composition.text);
        display.push_str(&text[end..]);
        let compose_end = start + composition.text.len();
        let (selection_start, selection_end) = composition
            .cursor
            .map(|(selection_start, selection_end)| {
                (
                    start + selection_start.min(composition.text.len()),
                    start + selection_end.min(composition.text.len()),
                )
            })
            .unwrap_or((compose_end, compose_end));
        AndroidTextInputSnapshot {
            text: display,
            selection_start,
            selection_end,
            composing_start: Some(start),
            composing_end: Some(compose_end),
        }
    } else {
        let (selection_start, selection_end) = state
            .selection_range()
            .unwrap_or((state.cursor, state.cursor));
        AndroidTextInputSnapshot {
            text: text.to_string(),
            selection_start,
            selection_end,
            composing_start: None,
            composing_end: None,
        }
    }
}

#[cfg(all(target_os = "android", feature = "android"))]
fn snapshot_to_runtime_state(
    snapshot: &AndroidTextInputSnapshot,
    current_text: &str,
    previous_state: &TextEditState,
) -> (String, TextEditState) {
    let selection_start = clamp_char_boundary(&snapshot.text, snapshot.selection_start);
    let selection_end = clamp_char_boundary(&snapshot.text, snapshot.selection_end);
    let Some((compose_start, compose_end)) = snapshot
        .composing_start
        .zip(snapshot.composing_end)
        .map(|(start, end)| {
            let start = clamp_char_boundary(&snapshot.text, start);
            let end = clamp_char_boundary(&snapshot.text, end.max(start));
            (start, end)
        })
        .filter(|(start, end)| start < end)
    else {
        return (
            snapshot.text.clone(),
            TextEditState {
                cursor: selection_end,
                anchor: selection_start,
                composition: None,
                scroll_x: previous_state.scroll_x,
                scroll_y: previous_state.scroll_y,
                preferred_column_x: None,
            }
            .clamped_to(&snapshot.text),
        );
    };

    let replace_range = previous_state
        .composition
        .as_ref()
        .map(|composition| composition.replace_range)
        .or_else(|| previous_state.selection_range())
        .unwrap_or((previous_state.cursor, previous_state.cursor));
    let replace_start = clamp_char_boundary(current_text, replace_range.0);
    let replace_end = clamp_char_boundary(current_text, replace_range.1.max(replace_start));
    let cursor = if selection_start >= compose_start && selection_end <= compose_end {
        Some((
            selection_start - compose_start,
            selection_end - compose_start,
        ))
    } else {
        let compose_len = compose_end - compose_start;
        Some((compose_len, compose_len))
    };
    (
        current_text.to_string(),
        TextEditState {
            cursor: replace_end,
            anchor: replace_start,
            composition: Some(CompositionState {
                replace_range: (replace_start, replace_end),
                text: snapshot.text[compose_start..compose_end].to_string(),
                cursor,
            }),
            scroll_x: previous_state.scroll_x,
            scroll_y: previous_state.scroll_y,
            preferred_column_x: None,
        }
        .clamped_to(current_text),
    )
}

#[cfg(all(target_os = "android", feature = "android"))]
fn replacement_bounds(old_text: &str, new_text: &str) -> Option<(usize, usize, usize, usize)> {
    if old_text == new_text {
        return None;
    }

    let mut prefix = 0usize;
    let mut old_iter = old_text.chars();
    let mut new_iter = new_text.chars();
    loop {
        match (old_iter.next(), new_iter.next()) {
            (Some(old_char), Some(new_char)) if old_char == new_char => {
                prefix += old_char.len_utf8();
            }
            _ => break,
        }
    }

    let old_remaining = &old_text[prefix..];
    let new_remaining = &new_text[prefix..];
    let mut suffix = 0usize;
    let mut old_rev = old_remaining.chars().rev();
    let mut new_rev = new_remaining.chars().rev();
    loop {
        match (old_rev.next(), new_rev.next()) {
            (Some(old_char), Some(new_char))
                if old_char == new_char
                    && suffix + old_char.len_utf8() <= old_remaining.len()
                    && suffix + new_char.len_utf8() <= new_remaining.len() =>
            {
                suffix += old_char.len_utf8();
            }
            _ => break,
        }
    }

    Some((
        prefix,
        old_text.len().saturating_sub(suffix),
        prefix,
        new_text.len().saturating_sub(suffix),
    ))
}

#[cfg(all(target_os = "android", feature = "android"))]
fn clamp_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[cfg(all(target_os = "android", feature = "android"))]
fn utf16_index_to_byte_index(text: &str, utf16_index: usize) -> usize {
    if utf16_index == 0 {
        return 0;
    }

    let mut utf16_units = 0usize;
    for (byte_index, ch) in text.char_indices() {
        let next_units = utf16_units + ch.len_utf16();
        if next_units > utf16_index {
            return byte_index;
        }
        utf16_units = next_units;
        if utf16_units == utf16_index {
            return byte_index + ch.len_utf8();
        }
    }
    text.len()
}

#[cfg(all(target_os = "android", feature = "android"))]
fn byte_index_to_utf16_index(text: &str, byte_index: usize) -> jint {
    let byte_index = clamp_char_boundary(text, byte_index);
    let mut utf16_units = 0usize;
    for (char_index, ch) in text.char_indices() {
        if char_index >= byte_index {
            break;
        }
        utf16_units += ch.len_utf16();
    }
    utf16_units.min(jint::MAX as usize) as jint
}
