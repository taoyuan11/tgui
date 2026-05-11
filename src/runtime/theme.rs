use crate::application::ThemeSelection;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::foundation::color::Color;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::log::Log;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::ndk::configuration::UiModeNight;
#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
use crate::platform::backend::window::Window;
use crate::platform::window::Theme as WindowTheme;
use crate::ui::theme::{Theme, ThemeSet};
#[cfg(all(target_os = "android", feature = "android"))]
use jni::{jni_sig, jni_str, objects::JObject, JValue, JavaVM};

#[cfg(all(target_os = "android", feature = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SystemBarStyle {
    pub(super) color: Color,
    pub(super) use_dark_icons: bool,
}

#[cfg(all(target_os = "android", feature = "android"))]
impl SystemBarStyle {
    pub(super) fn from_theme(theme: &Theme) -> Self {
        let color = theme.colors.background;
        Self {
            color,
            use_dark_icons: is_light_color(color),
        }
    }
}

pub(super) fn resolve_theme(
    selection: &ThemeSelection,
    theme_set: &ThemeSet,
    window_theme: Option<WindowTheme>,
) -> Theme {
    match selection {
        ThemeSelection::System => theme_set
            .resolve_window_theme(window_theme)
            .as_ref()
            .clone(),
        ThemeSelection::Mode(mode) => theme_set.resolve(*mode, window_theme).as_ref().clone(),
    }
}

pub(super) fn resolve_window_theme(
    window: Option<&dyn Window>,
    #[cfg(all(target_os = "android", feature = "android"))] android_app: Option<&AndroidApp>,
) -> Option<WindowTheme> {
    #[cfg(all(target_os = "android", feature = "android"))]
    if let Some(app) = android_app {
        if let Some(theme) = resolve_android_window_theme(app) {
            return Some(theme);
        }
    }

    window.and_then(|window| window.theme())
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(super) fn resolve_android_window_theme(app: &AndroidApp) -> Option<WindowTheme> {
    resolve_android_window_theme_from_java(app).or_else(|| match app.config().ui_mode_night() {
        UiModeNight::No => Some(WindowTheme::Light),
        UiModeNight::Yes => Some(WindowTheme::Dark),
        UiModeNight::Any | UiModeNight::__Unknown(_) => None,
        _ => None,
    })
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(super) fn resolve_android_window_theme_from_java(app: &AndroidApp) -> Option<WindowTheme> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<Option<WindowTheme>> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
        let ui_mode_service = env
            .get_static_field(
                jni_str!("android/content/Context"),
                jni_str!("UI_MODE_SERVICE"),
                jni_sig!("Ljava/lang/String;"),
            )?
            .l()?;
        let ui_mode_manager = env
            .call_method(
                &activity,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&ui_mode_service)],
            )?
            .l()?;

        if !ui_mode_manager.is_null() {
            let night_mode = env
                .call_method(
                    &ui_mode_manager,
                    jni_str!("getNightMode"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            let light = env
                .get_static_field(
                    jni_str!("android/app/UiModeManager"),
                    jni_str!("MODE_NIGHT_NO"),
                    jni_sig!("I"),
                )?
                .i()?;
            let dark = env
                .get_static_field(
                    jni_str!("android/app/UiModeManager"),
                    jni_str!("MODE_NIGHT_YES"),
                    jni_sig!("I"),
                )?
                .i()?;

            match night_mode {
                mode if mode == light => return Ok(Some(WindowTheme::Light)),
                mode if mode == dark => return Ok(Some(WindowTheme::Dark)),
                _ => {}
            }
        }

        let resources = env
            .call_method(
                &activity,
                jni_str!("getResources"),
                jni_sig!("()Landroid/content/res/Resources;"),
                &[],
            )?
            .l()?;
        let configuration = env
            .call_method(
                &resources,
                jni_str!("getConfiguration"),
                jni_sig!("()Landroid/content/res/Configuration;"),
                &[],
            )?
            .l()?;

        let ui_mode = env
            .get_field(&configuration, jni_str!("uiMode"), jni_sig!("I"))?
            .i()?;
        let mask = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_MASK"),
                jni_sig!("I"),
            )?
            .i()?;
        let light = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_NO"),
                jni_sig!("I"),
            )?
            .i()?;
        let dark = env
            .get_static_field(
                jni_str!("android/content/res/Configuration"),
                jni_str!("UI_MODE_NIGHT_YES"),
                jni_sig!("I"),
            )?
            .i()?;

        Ok(match ui_mode & mask {
            mode if mode == light => Some(WindowTheme::Light),
            mode if mode == dark => Some(WindowTheme::Dark),
            _ => None,
        })
    })
    .ok()
    .flatten()
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(super) fn android_font_scale(android_app: Option<&AndroidApp>) -> Option<f32> {
    let app = android_app?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<Option<f32>> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
        let resources = env
            .call_method(
                &activity,
                jni_str!("getResources"),
                jni_sig!("()Landroid/content/res/Resources;"),
                &[],
            )?
            .l()?;
        let configuration = env
            .call_method(
                &resources,
                jni_str!("getConfiguration"),
                jni_sig!("()Landroid/content/res/Configuration;"),
                &[],
            )?
            .l()?;
        let scale = env
            .get_field(&configuration, jni_str!("fontScale"), jni_sig!("F"))?
            .f()?;

        Ok((scale.is_finite() && scale > 0.0).then_some(scale))
    })
    .ok()
    .flatten()
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(super) fn apply_android_system_bar_style(
    app: &AndroidApp,
    style: SystemBarStyle,
) -> Result<(), String> {
    let scheduler_app = app.clone();
    let callback_app = scheduler_app.clone();
    scheduler_app.run_on_java_main_thread(Box::new(move || {
        if let Err(error) = apply_android_system_bar_style_on_main_thread(&callback_app, style) {
            Log::with_tag("tgui-runtime")
                .warn(format_args!("failed to sync Android system bars: {error}"));
        }
    }));

    Ok(())
}

#[cfg(all(target_os = "android", feature = "android"))]
fn apply_android_system_bar_style_on_main_thread(
    app: &AndroidApp,
    style: SystemBarStyle,
) -> Result<(), String> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw)? };
        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;

        let bar_color = color_to_android_argb(style.color);
        env.call_method(
            &window,
            jni_str!("setStatusBarColor"),
            jni_sig!("(I)V"),
            &[JValue::Int(bar_color)],
        )?;
        env.call_method(
            &window,
            jni_str!("setNavigationBarColor"),
            jni_sig!("(I)V"),
            &[JValue::Int(bar_color)],
        )?;

        let sdk_int = env
            .get_static_field(
                jni_str!("android/os/Build$VERSION"),
                jni_str!("SDK_INT"),
                jni_sig!("I"),
            )?
            .i()?;

        if sdk_int >= 30 {
            let controller = env
                .call_method(
                    &window,
                    jni_str!("getInsetsController"),
                    jni_sig!("()Landroid/view/WindowInsetsController;"),
                    &[],
                )?
                .l()?;

            if !controller.is_null() {
                let light_status = env
                    .get_static_field(
                        jni_str!("android/view/WindowInsetsController"),
                        jni_str!("APPEARANCE_LIGHT_STATUS_BARS"),
                        jni_sig!("I"),
                    )?
                    .i()?;
                let light_navigation = env
                    .get_static_field(
                        jni_str!("android/view/WindowInsetsController"),
                        jni_str!("APPEARANCE_LIGHT_NAVIGATION_BARS"),
                        jni_sig!("I"),
                    )?
                    .i()?;
                let mask = light_status | light_navigation;
                let appearance = if style.use_dark_icons { mask } else { 0 };
                env.call_method(
                    &controller,
                    jni_str!("setSystemBarsAppearance"),
                    jni_sig!("(II)V"),
                    &[JValue::Int(appearance), JValue::Int(mask)],
                )?;
            }
        } else {
            let decor_view = env
                .call_method(
                    &window,
                    jni_str!("getDecorView"),
                    jni_sig!("()Landroid/view/View;"),
                    &[],
                )?
                .l()?;
            let mut visibility = env
                .call_method(
                    &decor_view,
                    jni_str!("getSystemUiVisibility"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;

            let light_status = if sdk_int >= 23 {
                env.get_static_field(
                    jni_str!("android/view/View"),
                    jni_str!("SYSTEM_UI_FLAG_LIGHT_STATUS_BAR"),
                    jni_sig!("I"),
                )?
                .i()?
            } else {
                0
            };
            let light_navigation = if sdk_int >= 26 {
                env.get_static_field(
                    jni_str!("android/view/View"),
                    jni_str!("SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR"),
                    jni_sig!("I"),
                )?
                .i()?
            } else {
                0
            };

            let flags = light_status | light_navigation;
            if style.use_dark_icons {
                visibility |= flags;
            } else {
                visibility &= !flags;
            }

            env.call_method(
                &decor_view,
                jni_str!("setSystemUiVisibility"),
                jni_sig!("(I)V"),
                &[JValue::Int(visibility)],
            )?;
        }

        Ok(())
    })
    .map_err(|error| format!("failed to sync Android system bars: {error}"))?;

    Ok(())
}

#[cfg(all(target_os = "android", feature = "android"))]
fn color_to_android_argb(color: Color) -> i32 {
    ((color.a as i32) << 24) | ((color.r as i32) << 16) | ((color.g as i32) << 8) | color.b as i32
}

#[cfg(all(target_os = "android", feature = "android"))]
pub(super) fn is_light_color(color: Color) -> bool {
    let to_linear = |channel: u8| {
        let value = channel as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };

    let luminance =
        0.2126 * to_linear(color.r) + 0.7152 * to_linear(color.g) + 0.0722 * to_linear(color.b);
    luminance > 0.5
}
