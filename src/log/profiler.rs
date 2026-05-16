use std::fmt::Display;
use std::sync::OnceLock;
use std::time::Duration;

use super::api::Log;

const TEXT_PROFILE_ENV: &str = "TGUI_TEXT_PROFILE";
const TEXT_PROFILE_MIN_MS_ENV: &str = "TGUI_TEXT_PROFILE_MIN_MS";
const TEXT_PROFILE_LABELS: &[&str] = &[
    "textarea_about_to_wait",
    "textarea_animation",
    "textarea_animation_keys",
    "textarea_computed_scene",
    "textarea_flush_pending",
    "textarea_flush_session",
    "textarea_input_edit",
    "textarea_invalidate_scene",
    "textarea_invalidation",
    "textarea_keyboard",
    "textarea_patch_layout",
    "textarea_patch_scene",
    "textarea_patch_scene_collect",
    "textarea_patch_scene_collect_root",
    "textarea_patch_scene_focus_override",
    "textarea_patch_scene_layout_overrides",
    "textarea_patch_scene_recompose",
    "textarea_patch_scene_resolve_roots",
    "textarea_patch_scene_root_clone",
    "textarea_redraw",
    "textarea_render",
    "textarea_text_widget",
    "textarea_theme_sync",
];

pub(crate) fn text_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var(TEXT_PROFILE_ENV)
            .map(|value| {
                let value = value.trim();
                matches!(value, "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
            })
            .unwrap_or(false)
    })
}

fn text_profile_min_duration() -> Duration {
    static MIN_DURATION: OnceLock<Duration> = OnceLock::new();
    *MIN_DURATION.get_or_init(|| {
        std::env::var(TEXT_PROFILE_MIN_MS_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| Duration::from_secs_f64(value / 1000.0))
            .unwrap_or(Duration::ZERO)
    })
}

fn text_profile_label_enabled(label: &str) -> bool {
    TEXT_PROFILE_LABELS.contains(&label)
}

pub(crate) fn log_text_profile(label: &str, duration: Duration, message: impl Display) {
    if !text_profile_enabled()
        || !text_profile_label_enabled(label)
        || duration < text_profile_min_duration()
    {
        return;
    }

    Log::with_tag("tgui-text-prof").debug(format_args!(
        "{label} took {:.3}ms {message}",
        duration.as_secs_f64() * 1000.0
    ));
}
