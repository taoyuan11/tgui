use super::super::scene::ReactiveProgressLabel;
use super::resolved_freeze::lifecycle_snapshot;
use super::*;
use crate::ui::widget::common::{ComputedSceneCursor, ComputedScenePrefixCursor};
use crate::ui::widget::r#virtual::{apply_virtual_runtime_state_to_element, VirtualViewportHint};
use crate::ui::widget::{FocusScopeState, TransformRecord};

const BACKGROUND_BRUSH_DIRECT_FALLBACK_BIT: u16 = 1 << 0;
const BACKGROUND_BLUR_DIRECT_FALLBACK_BIT: u16 = 1 << 1;
const OFFSET_DIRECT_FALLBACK_BIT: u16 = 1 << 2;
const BACKGROUND_DIRECT_FALLBACK_BIT: u16 = 1 << 3;
const CONTAINER_OPACITY_DIRECT_FALLBACK_BIT: u16 = 1 << 4;
const BORDER_COLOR_DIRECT_FALLBACK_BIT: u16 = 1 << 5;
const BORDER_RADIUS_DIRECT_FALLBACK_BIT: u16 = 1 << 6;
const BORDER_WIDTH_DIRECT_FALLBACK_BIT: u16 = 1 << 7;
const TEXT_OPACITY_DIRECT_FALLBACK_BIT: u16 = 1 << 8;
const SCALE_DIRECT_FALLBACK_BIT: u16 = 1 << 9;
const SLIDER_VALUE_DIRECT_FALLBACK_BIT: u16 = 1 << 10;

#[cfg(feature = "bench-support")]
thread_local! {
    static FORCE_LEGACY_SCENE_SNAPSHOTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_TEXTURE_MASK_TINT_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_TEXT_CONTENT_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_TEXT_COLOR_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BACKGROUND_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BACKGROUND_BRUSH_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BACKGROUND_BLUR_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_OFFSET_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_SCALE_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BORDER_COLOR_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BORDER_RADIUS_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_BORDER_WIDTH_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_TEXT_OPACITY_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_CONTAINER_OPACITY_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_PROGRESS_VALUE_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCE_LEGACY_SLIDER_VALUE_REACTIVE_RESOLVE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_scene_snapshots<R>(f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_SCENE_SNAPSHOTS.with(|flag| {
        let previous = flag.replace(true);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former texture-mask tint resolver, which rebuilt the
/// complete visual state even though the retained tint patch only needs the icon state and color.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_texture_mask_tint_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_TEXTURE_MASK_TINT_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former fixed-Text content resolver, which rebuilt the
/// complete visual state even though the retained content patch only needs text and font data.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_text_content_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_TEXT_CONTENT_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former Text color resolver, which rebuilt the complete
/// visual state even though the retained color patch only needs the resolved color and opacity.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_text_color_reactive_resolve<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_TEXT_COLOR_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former plain-container background resolver, which rebuilt
/// the complete visual state even though a retained fill-color patch only needs the background
/// frame, effective opacity, and resolved background color.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_background_reactive_resolve<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_BACKGROUND_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former BackgroundBrush resolver, which rebuilt the complete
/// visual state even when one empty, otherwise static Container only needs a retained brush write.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_background_brush_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_BACKGROUND_BRUSH_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former BackgroundBlur resolver, which rebuilt the complete
/// visual state even when one empty, otherwise static Container only needs a retained blur write.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_background_blur_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_BACKGROUND_BLUR_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former Offset resolver, which rebuilt the complete visual
/// state for a default-hidden empty solid Container before writing one retained rect and hit.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_offset_reactive_resolve<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_OFFSET_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former Scale resolver, which rebuilt the complete visual
/// state for a default-hidden empty solid Container before writing one retained rect and hit.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_scale_reactive_resolve<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_SCALE_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former plain-container BorderColor resolver, which rebuilt
/// the complete visual state even though a retained stroke-color patch only needs stable frame,
/// width, opacity, and the explicit border color.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_border_color_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_BORDER_COLOR_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former plain-container BorderRadius resolver, which
/// rebuilt the complete visual state for an otherwise static surface.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_border_radius_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_BORDER_RADIUS_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_border_width_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_BORDER_WIDTH_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_text_opacity_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_TEXT_OPACITY_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former plain-container opacity resolver, which rebuilt the
/// complete visual state even when one empty solid surface only needs retained color writes.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_container_opacity_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_CONTAINER_OPACITY_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former ProgressValue resolver, which rebuilt the complete
/// visual state and every widget style even though a retained progress patch only needs the
/// progress geometry, colors, and optional label payload.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_progress_value_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_PROGRESS_VALUE_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

/// Benchmark-only A/B control for the former SliderValue resolver, which rebuilt generic visual
/// chrome even though the retained patch only moves the active track, thumb, optional label, and
/// matching hit metadata.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_slider_value_reactive_resolve<R>(
    legacy: bool,
    f: impl FnOnce() -> R,
) -> R {
    FORCE_LEGACY_SLIDER_VALUE_REACTIVE_RESOLVE.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

fn legacy_texture_mask_tint_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_TEXTURE_MASK_TINT_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_text_content_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_TEXT_CONTENT_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_text_color_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_TEXT_COLOR_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_background_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BACKGROUND_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_background_brush_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BACKGROUND_BRUSH_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_background_blur_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BACKGROUND_BLUR_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_offset_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_OFFSET_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_scale_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_SCALE_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod background_brush_direct_probe {
    thread_local! {
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod background_blur_direct_probe {
    thread_local! {
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod offset_direct_probe {
    thread_local! {
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        HITS.with(|hits| hits.set(0));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod scale_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod container_opacity_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod border_color_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod border_radius_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod border_width_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod text_opacity_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

#[cfg(all(test, feature = "bench-support"))]
pub(crate) mod slider_value_direct_probe {
    thread_local! {
        static ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PREPARED_FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(crate) fn reset() {
        ATTEMPTS.with(|attempts| attempts.set(0));
        HITS.with(|hits| hits.set(0));
        PREPARED_FALLBACKS.with(|fallbacks| fallbacks.set(0));
    }

    pub(crate) fn record_attempt() {
        ATTEMPTS.with(|attempts| attempts.set(attempts.get().saturating_add(1)));
    }

    pub(crate) fn record_hit() {
        HITS.with(|hits| hits.set(hits.get().saturating_add(1)));
    }

    pub(crate) fn record_prepared_fallback() {
        PREPARED_FALLBACKS.with(|fallbacks| {
            fallbacks.set(fallbacks.get().saturating_add(1));
        });
    }

    pub(crate) fn attempts() -> usize {
        ATTEMPTS.with(std::cell::Cell::get)
    }

    pub(crate) fn hits() -> usize {
        HITS.with(std::cell::Cell::get)
    }

    pub(crate) fn prepared_fallbacks() -> usize {
        PREPARED_FALLBACKS.with(std::cell::Cell::get)
    }
}

fn legacy_border_color_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BORDER_COLOR_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_border_radius_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BORDER_RADIUS_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_border_width_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_BORDER_WIDTH_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_text_opacity_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_TEXT_OPACITY_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_container_opacity_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_CONTAINER_OPACITY_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_progress_value_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_PROGRESS_VALUE_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

fn legacy_slider_value_reactive_resolve_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_SLIDER_VALUE_REACTIVE_RESOLVE.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    {
        false
    }
}

#[cfg(feature = "bench-support")]
fn use_legacy_scene_snapshots() -> bool {
    FORCE_LEGACY_SCENE_SNAPSHOTS.with(std::cell::Cell::get)
}

pub(super) enum SceneDeltaSnapshot<VM> {
    Cursor(ComputedSceneCursor, std::marker::PhantomData<fn() -> VM>),
    #[cfg(feature = "bench-support")]
    Legacy(ComputedScene<VM>),
}

impl<VM> SceneDeltaSnapshot<VM> {
    fn capture(computed: &ComputedScene<VM>) -> Self {
        #[cfg(feature = "bench-support")]
        if use_legacy_scene_snapshots() {
            return Self::Legacy(computed.clone());
        }
        Self::Cursor(computed.cursor(), std::marker::PhantomData)
    }

    fn delta(self, computed: &ComputedScene<VM>) -> ComputedScene<VM> {
        match self {
            Self::Cursor(cursor, _) => computed.delta_since_cursor(&cursor),
            #[cfg(feature = "bench-support")]
            Self::Legacy(base) => computed.delta_since(&base),
        }
    }
}

pub(super) enum ScenePrefixSnapshot<VM> {
    Cursor(
        ComputedScenePrefixCursor,
        std::marker::PhantomData<fn() -> VM>,
    ),
    #[cfg(feature = "bench-support")]
    Legacy(ComputedScene<VM>),
}

impl<VM> ScenePrefixSnapshot<VM> {
    pub(super) fn capture(computed: &ComputedScene<VM>) -> Self {
        #[cfg(feature = "bench-support")]
        if use_legacy_scene_snapshots() {
            return Self::Legacy(computed.clone());
        }
        Self::Cursor(computed.prefix_cursor(), std::marker::PhantomData)
    }

    pub(super) fn materialize(self, computed: &ComputedScene<VM>) -> ComputedScene<VM> {
        match self {
            Self::Cursor(cursor, _) => computed.prefix_at_cursor(&cursor),
            #[cfg(feature = "bench-support")]
            Self::Legacy(prefix) => prefix,
        }
    }
}

mod chrome;
mod controls;
mod drawer;
mod layout_media;
mod menu;
mod modal;
mod popover;
pub(crate) mod portal;
pub(in crate::ui::widget::core) mod toast;
mod tooltip;

struct CollectResolvedStyles {
    button_style: Option<ResolvedButtonStyle>,
    select_style: Option<ResolvedSelectStyle>,
    slider_style: Option<ResolvedSliderStyle>,
    progress_bar_style: Option<crate::ui::widget::style::ProgressBarStyle>,
    spinner_style: Option<crate::ui::widget::style::SpinnerStyle>,
    divider_style: Option<crate::ui::widget::style::DividerStyle>,
    switch_style: Option<crate::ui::widget::style::SwitchStyle>,
    input_style: Option<ResolvedInputStyle>,
    checkbox_style: Option<ResolvedCheckboxStyle>,
    radio_style: Option<ResolvedRadioStyle>,
}

struct CollectVisualState {
    frame: Rect,
    background_frame: Rect,
    background_radius: Dp,
    runtime_visual: VisualStyle,
    offset: Point,
    reactive_offset: bool,
    primitive_clip: Option<Rect>,
    overflow_clip: Option<Rect>,
    primitive_clip_mask: Option<ClipMask>,
    disabled: bool,
    widget_state: WidgetState,
    opacity: f32,
    border_width: Dp,
    border_radius: Dp,
    border_color: Color,
    background: Color,
    has_surface_background: bool,
    reactive_background: bool,
    reactive_border_color: bool,
    reactive_opacity: bool,
    styles: CollectResolvedStyles,
}

struct PreparedCollectVisualRuntime {
    disabled: bool,
    widget_state: WidgetState,
    runtime_background: Option<Value<Color>>,
    runtime_visual: VisualStyle,
}

enum PlainContainerDirectResolve {
    Resolved(ReactiveScenePropertyValue),
    PreparedFallback(PreparedCollectVisualRuntime),
    Ineligible,
}

enum SliderValueDirectResolve {
    Resolved(ReactiveScenePropertyValue),
    StickyPreparedFallback(PreparedCollectVisualRuntime),
    TransientPreparedFallback {
        prepared: PreparedCollectVisualRuntime,
        slider_style: Option<ResolvedSliderStyle>,
    },
    Ineligible,
}

fn slider_surface_is_static_default(
    surface: &crate::ui::widget::style::WidgetSurfaceStyle,
) -> bool {
    surface.background.is_none()
        && surface.background_brush.is_none()
        && surface.background_image.is_none()
        && matches!(&surface.background_blur, Value::Static(value) if *value == Dp::ZERO)
        && surface.shadow.is_none()
        && surface.border_color.is_none()
        && surface.border_radius.is_none()
        && surface.border_width.is_none()
        && matches!(&surface.opacity, Value::Static(value) if *value == 1.0)
        && matches!(&surface.offset, Value::Static(value) if *value == Point::ZERO)
}

fn has_static_fixed_frame(layout: &LayoutStyle) -> bool {
    let fixed_width = matches!(
        &layout.width,
        Some(Value::Static(Length::Px(width))) if *width > Dp::ZERO
    );
    let fixed_height = matches!(
        &layout.height,
        Some(Value::Static(Length::Px(height))) if *height > Dp::ZERO
    );
    fixed_width && fixed_height
}

struct CollectCaches<'a, VM> {
    lifecycle_states: &'a mut HashMap<WidgetId, LifecycleEventState<VM>>,
    chunks: &'a mut HashMap<WidgetId, ComputedScene<VM>>,
    chunk_parts: &'a mut HashMap<WidgetId, SceneChunkParts<VM>>,
    visual_contexts: &'a mut HashMap<WidgetId, VisualContextSnapshot>,
}

pub(super) fn prepare_nested_scene_root<VM>(
    root: &mut Element<VM>,
    context: &CollectContext<'_, '_>,
    fallback_viewport: Rect,
) {
    apply_virtual_runtime_state_to_element(
        root,
        context.scroll_offsets,
        context.virtual_states,
        VirtualViewportHint {
            width: fallback_viewport.width,
            height: fallback_viewport.height,
        },
    );
}

impl<VM: 'static> ResolvedElement<VM> {
    pub(in crate::ui::widget::core) fn resolve_reactive_transform_offset(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<Point> {
        let visual = self.resolve_collect_visual_state(layout_node, visual_context, context);
        visual.reactive_offset.then_some(visual.offset)
    }

    pub(in crate::ui::widget::core) fn resolve_reactive_scene_property_value(
        &self,
        property: PropertySlot,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        let mut prepared_collect_runtime = None;
        let mut prepared_slider_style = None;
        if property == PropertySlot::TextureMaskTint
            && !legacy_texture_mask_tint_reactive_resolve_enabled()
        {
            // Image contributes no disabled/selected/open specialization in
            // `collect_widget_state`; its value is exactly the retained interaction map entry.
            // The tint resolver also consumes `self.visual` directly, so Taffy geometry,
            // runtime image-surface merging, offset/scale/opacity, borders and background are
            // unrelated work. If any prerequisite is absent, retain the complete resolver below
            // as the correctness fallback.
            if let ResolvedWidgetKind::Image { image, .. } = &self.kind {
                if let Some(resolver) = image.runtime_mask_tint.as_ref() {
                    let widget_state = context.widget_states.get(self.id);
                    return Some(ReactiveScenePropertyValue::TextureMaskTint {
                        color: resolver(
                            &context.style_context,
                            context.style_sheet,
                            &self.visual,
                            widget_state,
                            context.animations,
                            self.id,
                            context.now,
                        ),
                    });
                }
            }
        }
        if property == PropertySlot::TextContent && !legacy_text_content_reactive_resolve_enabled()
        {
            // A fixed, non-selectable, single-line Text retains its geometry and every visual
            // field. Resolving content therefore does not need Taffy, runtime surface merging,
            // interaction state, transforms, opacity, borders, backgrounds, or component styles.
            // TextEditor and every ineligible Text continue through the complete resolver below.
            if let ResolvedWidgetKind::Text { text, .. } = &self.kind {
                if let Some(value) = self.resolve_fixed_text_content(text, context) {
                    return Some(value);
                }
            }
        }
        if property == PropertySlot::TextColor && !legacy_text_color_reactive_resolve_enabled() {
            // A retained TextColor write cannot change geometry or primitive structure. Resolve
            // only the stateful Text surface opacity and the explicit text color; Taffy, offset,
            // scale, component styles, borders and backgrounds are unrelated work. The complete
            // resolver below remains the fallback for every non-Text or implicit-color target.
            if let ResolvedWidgetKind::Text { text, .. } = &self.kind {
                if let Some(value) = self.resolve_text_color(text, visual_context, context) {
                    return Some(value);
                }
            }
        }
        if property == PropertySlot::Opacity
            && !legacy_text_opacity_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(TEXT_OPACITY_DIRECT_FALLBACK_BIT)
        {
            let outcome = match &self.kind {
                ResolvedWidgetKind::Text { text, .. } => {
                    self.resolve_plain_text_opacity(text, visual_context, context)
                }
                _ => PlainContainerDirectResolve::Ineligible,
            };
            match outcome {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    text_opacity_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(TEXT_OPACITY_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    text_opacity_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(TEXT_OPACITY_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::Opacity
            && !legacy_container_opacity_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(CONTAINER_OPACITY_DIRECT_FALLBACK_BIT)
        {
            // An empty, role-free Container with one static solid surface retains its background
            // and optional border slots while opacity stays on the same side of zero. Resolve
            // only those static inputs and the opacity signal. The occluder guard below records
            // the zero-crossing topology bit, so a changed bit rejects the slot write and falls
            // back to the existing bounded subtree recollection.
            match self.resolve_plain_container_opacity(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    container_opacity_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(CONTAINER_OPACITY_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    container_opacity_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(CONTAINER_OPACITY_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::Background
            && !legacy_background_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BACKGROUND_DIRECT_FALLBACK_BIT)
        {
            // A plain Container/Virtual retained background write changes one existing solid-fill
            // color. The complete visual resolver also samples border color/radius, component
            // styles, validation, clipping metadata, and every unrelated paint field. Keep this
            // deliberately narrow: complex surfaces and special row/sticky geometry continue
            // through the complete resolver below.
            match self.resolve_plain_container_background(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => return Some(value),
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BACKGROUND_DIRECT_FALLBACK_BIT);
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BACKGROUND_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::BackgroundBrush
            && !legacy_background_brush_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BACKGROUND_BRUSH_DIRECT_FALLBACK_BIT)
        {
            // A reactive brush on one empty, otherwise static Container always retains exactly
            // one Brush command. Resolve only its fixed geometry, effective opacity and clip;
            // every composite surface or special role continues through the complete resolver.
            match self.resolve_plain_container_background_brush(
                layout_node,
                visual_context,
                context,
            ) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    background_brush_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BACKGROUND_BRUSH_DIRECT_FALLBACK_BIT);
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BACKGROUND_BRUSH_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::BackgroundBlur
            && !legacy_background_blur_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BACKGROUND_BLUR_DIRECT_FALLBACK_BIT)
        {
            // A reactive blur on one empty, otherwise static Container retains exactly one
            // BackdropBlur primitive. Resolve only its fixed surface geometry and paint-topology
            // guard; any complex surface or role continues through the complete resolver below.
            match self.resolve_plain_container_background_blur(layout_node, visual_context, context)
            {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    background_blur_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BACKGROUND_BLUR_DIRECT_FALLBACK_BIT);
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BACKGROUND_BLUR_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::Offset
            && !legacy_offset_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(OFFSET_DIRECT_FALLBACK_BIT)
        {
            // Default-hidden, empty solid Containers cannot use the retained transform-record
            // path. Their Offset slot only moves one shape rect and its fallback Occluder; keep
            // every composite surface, semantic role, or non-unit transform on the full path.
            match self.resolve_plain_container_offset(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    offset_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(OFFSET_DIRECT_FALLBACK_BIT);
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(OFFSET_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::Scale
            && !legacy_scale_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(SCALE_DIRECT_FALLBACK_BIT)
        {
            // A fixed, default-hidden empty solid Container cannot retain a transform record.
            // Its Scale slot only resizes one centered fill rect and the matching fallback
            // Occluder. Keep every composite surface, semantic role, clip mask, scroll target,
            // or independently reactive transform on the complete resolver below.
            match self.resolve_plain_container_scale(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    scale_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(SCALE_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    scale_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(SCALE_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::BorderColor
            && !legacy_border_color_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BORDER_COLOR_DIRECT_FALLBACK_BIT)
        {
            // A plain Container with a stable solid background keeps its fallback occluder across
            // every border-alpha revision. Resolve only the explicit border signal plus static
            // geometry/opacity inputs; all component styles and topology-sensitive surfaces keep
            // using the complete resolver and its existing hit-topology guard below.
            match self.resolve_plain_container_border_color(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    border_color_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BORDER_COLOR_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    border_color_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BORDER_COLOR_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::BorderRadius
            && !legacy_border_radius_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BORDER_RADIUS_DIRECT_FALLBACK_BIT)
        {
            match self.resolve_plain_container_border_radius(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    border_radius_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BORDER_RADIUS_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    border_radius_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BORDER_RADIUS_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::BorderWidth
            && !legacy_border_width_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(BORDER_WIDTH_DIRECT_FALLBACK_BIT)
        {
            match self.resolve_plain_container_border_width(layout_node, visual_context, context) {
                PlainContainerDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    border_width_direct_probe::record_hit();
                    return Some(value);
                }
                PlainContainerDirectResolve::PreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(BORDER_WIDTH_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    border_width_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                PlainContainerDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(BORDER_WIDTH_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::SliderValue
            && !legacy_slider_value_reactive_resolve_enabled()
            && !self.reactive_direct_fallback_cached(SLIDER_VALUE_DIRECT_FALLBACK_BIT)
        {
            match self.resolve_plain_slider_value(layout_node, visual_context, context) {
                SliderValueDirectResolve::Resolved(value) => {
                    #[cfg(all(test, feature = "bench-support"))]
                    slider_value_direct_probe::record_hit();
                    return Some(value);
                }
                SliderValueDirectResolve::StickyPreparedFallback(prepared) => {
                    self.cache_reactive_direct_fallback(SLIDER_VALUE_DIRECT_FALLBACK_BIT);
                    #[cfg(all(test, feature = "bench-support"))]
                    slider_value_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                }
                SliderValueDirectResolve::TransientPreparedFallback {
                    prepared,
                    slider_style,
                } => {
                    #[cfg(all(test, feature = "bench-support"))]
                    slider_value_direct_probe::record_prepared_fallback();
                    prepared_collect_runtime = Some(prepared);
                    prepared_slider_style = slider_style;
                }
                SliderValueDirectResolve::Ineligible => {
                    self.cache_reactive_direct_fallback(SLIDER_VALUE_DIRECT_FALLBACK_BIT);
                }
            }
        }
        if property == PropertySlot::ProgressValue
            && !legacy_progress_value_reactive_resolve_enabled()
        {
            // A determinate ProgressBar value changes only the existing fill rect and, when the
            // percentage label is implicit, its text payload. Keep this path deliberately narrow:
            // indeterminate bars, custom scene structure, and any unsupported geometry continue
            // through the complete resolver below as a correctness fallback.
            if let Some(value) = self.resolve_progress_value(layout_node, visual_context, context) {
                return Some(value);
            }
        }
        let visual = match prepared_collect_runtime {
            Some(prepared) => match prepared_slider_style {
                Some(slider_style) => self
                    .resolve_collect_visual_state_with_runtime_and_slider_style(
                        layout_node,
                        visual_context,
                        context,
                        prepared,
                        slider_style,
                    ),
                None => self.resolve_collect_visual_state_with_runtime(
                    layout_node,
                    visual_context,
                    context,
                    prepared,
                ),
            },
            None => self.resolve_collect_visual_state(layout_node, visual_context, context),
        };
        match property {
            PropertySlot::Background => {
                if visual.background_frame.is_empty()
                    || (visual.background.a == 0 && !visual.reactive_background)
                {
                    return None;
                }
                let preserve_solid_background =
                    matches!(self.kind, ResolvedWidgetKind::Switch { .. });
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none()
                    || preserve_solid_background;
                draws_base_background.then_some(ReactiveScenePropertyValue::ShapeFillColor {
                    rect: visual.background_frame,
                    color: visual.background,
                    container_occluder: self.container_background_occluder_state(&visual, context),
                })
            }
            PropertySlot::BackgroundBrush => {
                if visual.background_frame.is_empty() {
                    return None;
                }
                let brush = visual
                    .runtime_visual
                    .background_brush
                    .as_ref()?
                    .resolve_widget()
                    .with_alpha_factor(visual.opacity);
                Some(ReactiveScenePropertyValue::Brush(BrushPrimitive {
                    rect: visual.background_frame,
                    brush,
                    corner_radius: visual.background_radius.get(),
                    clip_rect: visual.primitive_clip,
                    clip_mask: visual.primitive_clip_mask,
                }))
            }
            PropertySlot::BackgroundBlur => {
                if visual.background_frame.is_empty() {
                    return None;
                }
                let blur_radius = visual
                    .runtime_visual
                    .background_blur
                    .resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BackgroundBlur,
                        context.now,
                        context.units,
                    )
                    .max(0.0);
                Some(ReactiveScenePropertyValue::BackdropBlur {
                    primitive: BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    },
                    container_occluder: self.container_surface_occluder_state(
                        &visual,
                        blur_radius,
                        context,
                    ),
                })
            }
            PropertySlot::BorderColor => {
                if visual.frame.is_empty()
                    || (visual.border_color.a == 0 && !visual.reactive_border_color)
                {
                    return None;
                }
                if !self.border_color_slot_preserves_hit_topology(&visual) {
                    return None;
                }
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                (stroke_width > 0.0).then_some(ReactiveScenePropertyValue::ShapeStrokeColor {
                    rect: visual.frame,
                    stroke_width,
                    color: visual.border_color,
                })
            }
            PropertySlot::BorderWidth => {
                if !matches!(
                    &self.kind,
                    ResolvedWidgetKind::Container { children, .. } if children.is_empty()
                ) {
                    return None;
                }
                if visual.runtime_visual.shadow.is_some()
                    || visual.runtime_visual.background_brush.is_some()
                    || visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                {
                    return None;
                }
                let background = if !visual.background_frame.is_empty() && visual.background.a > 0 {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((visual.frame, visual.border_color, stroke_width))
                } else {
                    None
                };
                if background.is_none() && border.is_none() {
                    return None;
                }
                Some(ReactiveScenePropertyValue::BorderWidth {
                    frame: visual.frame,
                    background,
                    border,
                })
            }
            PropertySlot::BorderRadius => {
                if !matches!(
                    &self.kind,
                    ResolvedWidgetKind::Container { children, .. } if children.is_empty()
                ) {
                    return None;
                }
                if visual.runtime_visual.shadow.is_some()
                    || visual.runtime_visual.background_brush.is_some()
                    || visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                {
                    return None;
                }

                let background = if !visual.background_frame.is_empty() && visual.background.a > 0 {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((
                        visual.frame,
                        stroke_width,
                        visual.border_color,
                        visual.border_radius.get(),
                    ))
                } else {
                    None
                };
                if background.is_none() && border.is_none() {
                    return None;
                }
                Some(ReactiveScenePropertyValue::BorderRadius { background, border })
            }
            PropertySlot::Opacity => match &self.kind {
                ResolvedWidgetKind::Text { text, .. } => {
                    if text.user_select {
                        return None;
                    }
                    // The retained Text opacity binding only owns the text primitive's color.
                    // A decorated Text surface also contributes shapes, a shadow texture, brush,
                    // image, or backdrop blur whose alpha must change with the same opacity.
                    // Keep the one-slot path limited to a genuinely plain Text; every decorated
                    // surface falls back to the normal subtree recollection.
                    if visual.has_surface_background
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                        || visual.runtime_visual.border_color.is_some()
                        || visual.runtime_visual.border_width.is_some()
                    {
                        return None;
                    }
                    let color = text
                        .color
                        .as_ref()
                        .map(|color| {
                            color.resolve_widget(
                                context.animations,
                                self.id,
                                WidgetProperty::TextColor,
                                context.now,
                            )
                        })
                        .unwrap_or(context.theme.colors.on_surface)
                        .with_alpha_factor(visual.opacity);
                    Some(ReactiveScenePropertyValue::Opacity {
                        shadow: None,
                        background: None,
                        border: None,
                        text: Some(color),
                        container_occluder: None,
                    })
                }
                ResolvedWidgetKind::Container { children, .. } if children.is_empty() => {
                    // Active Container focus rings and tree-node chrome contribute additional
                    // primitives whose alpha is also derived from the surface opacity. They do
                    // not have fixed slots in the compact opacity plan, so keep them on the
                    // bounded subtree fallback instead of leaving those primitives stale.
                    if visual.widget_state.focus_visible
                        || self.tree_node.is_some()
                        || (self.list_item.is_some() && visual.widget_state.focus_visible)
                    {
                        return None;
                    }
                    if (widget_shadow_opacity_legacy_enabled()
                        && visual.runtime_visual.shadow.is_some())
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let shadow = visual
                        .runtime_visual
                        .shadow
                        .as_ref()
                        .map(Value::resolve)
                        .and_then(|shadow| {
                            rounded_rect_shadow_texture(
                                visual.background_frame,
                                visual.background_radius.get(),
                                RoundedRectShadowSpec {
                                    shadow,
                                    opacity: visual.opacity,
                                    clip_rect: visual.primitive_clip,
                                    clip_mask: visual.primitive_clip_mask,
                                },
                                context.media,
                                context.units,
                            )
                        })
                        .map(|texture| (texture.texture.id(), texture.frame, texture.opacity));
                    let background = if !visual.background_frame.is_empty()
                        && (visual.background.a > 0 || visual.reactive_opacity)
                    {
                        Some((visual.background_frame, visual.background))
                    } else {
                        None
                    };
                    let stroke_width = visual
                        .border_width
                        .get()
                        .min((visual.frame.width * 0.5).get())
                        .min((visual.frame.height * 0.5).get())
                        .max(0.0);
                    let border = if !visual.frame.is_empty()
                        && (visual.border_color.a > 0 || visual.reactive_opacity)
                        && stroke_width > 0.0
                    {
                        Some((visual.frame, stroke_width, visual.border_color))
                    } else {
                        None
                    };
                    if shadow.is_none() && background.is_none() && border.is_none() {
                        return None;
                    }
                    Some(ReactiveScenePropertyValue::Opacity {
                        shadow,
                        background,
                        border,
                        text: None,
                        container_occluder: self
                            .container_background_occluder_state(&visual, context),
                    })
                }
                ResolvedWidgetKind::Image { image, .. } => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let source = image.source.resolve();
                    let media_layout = crate::media::MediaTextureLayout::new(
                        visual.background_frame,
                        image.fit,
                        context.units.scale_factor(),
                    );
                    let (snapshot, target_frame, _) = context
                        .media
                        .image_snapshot_for_layout(&source, media_layout);
                    snapshot
                        .texture
                        .as_ref()
                        .map(|_| ReactiveScenePropertyValue::TextureOpacity {
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: visual.opacity,
                        })
                }
                _ => None,
            },
            PropertySlot::Texture => match &self.kind {
                ResolvedWidgetKind::Image { image, .. } => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let source = image.source.resolve();
                    let media_layout = crate::media::MediaTextureLayout::new(
                        visual.background_frame,
                        image.fit,
                        context.units.scale_factor(),
                    );
                    let (snapshot, target_frame, raster_request) = context
                        .media
                        .image_snapshot_for_layout(&source, media_layout);
                    let raster_request = raster_request?;
                    snapshot.texture.map(|texture| {
                        let mask_tint = image.runtime_mask_tint.as_ref().map(|resolver| {
                            resolver(
                                &context.style_context,
                                context.style_sheet,
                                &self.visual,
                                visual.widget_state,
                                context.animations,
                                self.id,
                                context.now,
                            )
                        });
                        ReactiveScenePropertyValue::Texture {
                            texture,
                            media_key: Some(crate::media::MediaTextureKey::new(
                                source,
                                raster_request,
                            )),
                            media_layout: Some(media_layout),
                            mask_tint,
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: visual.opacity.clamp(0.0, 1.0),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        }
                    })
                }
                ResolvedWidgetKind::Container { children, .. } if children.is_empty() => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || self.interactions.has_any()
                        || self.focus.focusable.is_some()
                        || self.focus.tab_index.is_some()
                        || self.focus.scope.is_some()
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let background_image = visual
                        .runtime_visual
                        .background_image
                        .as_ref()?
                        .resolve_widget();
                    let media_layout = crate::media::MediaTextureLayout::new(
                        visual.background_frame,
                        background_image.fit,
                        context.units.scale_factor(),
                    );
                    let (snapshot, target_frame, raster_request) = context
                        .media
                        .image_snapshot_for_layout(&background_image.source, media_layout);
                    let raster_request = raster_request?;
                    snapshot
                        .texture
                        .map(|texture| ReactiveScenePropertyValue::Texture {
                            texture,
                            media_key: Some(crate::media::MediaTextureKey::new(
                                background_image.source,
                                raster_request,
                            )),
                            media_layout: Some(media_layout),
                            mask_tint: None,
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: 1.0,
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                }
                _ => None,
            },
            PropertySlot::TextureMaskTint => match &self.kind {
                ResolvedWidgetKind::Image { image, .. } => {
                    image.runtime_mask_tint.as_ref().map(|resolver| {
                        ReactiveScenePropertyValue::TextureMaskTint {
                            color: resolver(
                                &context.style_context,
                                context.style_sheet,
                                &self.visual,
                                visual.widget_state,
                                context.animations,
                                self.id,
                                context.now,
                            ),
                        }
                    })
                }
                _ => None,
            },
            PropertySlot::Offset => {
                let ResolvedWidgetKind::Container {
                    layout, children, ..
                } = &self.kind
                else {
                    return None;
                };
                if !children.is_empty()
                    || self.interactions.has_any()
                    || self.focus.focusable.is_some()
                    || self.focus.tab_index.is_some()
                    || self.focus.scope.is_some()
                    || visual.runtime_visual.shadow.is_some()
                {
                    return None;
                }
                if self
                    .container_has_stable_semantic_hit(visual.disabled, context)
                    .unwrap_or(false)
                    || layout.scroll_view.is_some()
                {
                    return None;
                }
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none();
                let background = if draws_base_background
                    && !visual.background_frame.is_empty()
                    && visual.background.a > 0
                {
                    Some((visual.background_frame, visual.background))
                } else {
                    None
                };
                let backdrop_blur = if !visual.background_frame.is_empty() {
                    let blur_radius = visual
                        .runtime_visual
                        .background_blur
                        .resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BackgroundBlur,
                            context.now,
                            context.units,
                        )
                        .max(0.0);
                    (blur_radius > 0.0
                        || matches!(&visual.runtime_visual.background_blur, Value::Signal(_)))
                    .then_some(BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    })
                } else {
                    None
                };
                let brush = if !visual.background_frame.is_empty() {
                    visual
                        .runtime_visual
                        .background_brush
                        .as_ref()
                        .map(|brush| BrushPrimitive {
                            rect: visual.background_frame,
                            brush: brush.resolve_widget().with_alpha_factor(visual.opacity),
                            corner_radius: visual.background_radius.get(),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                } else {
                    None
                };
                let texture = visual
                    .runtime_visual
                    .background_image
                    .as_ref()
                    .and_then(|image| {
                        let image = image.resolve_widget();
                        let media_layout = crate::media::MediaTextureLayout::new(
                            visual.background_frame,
                            image.fit,
                            context.units.scale_factor(),
                        );
                        let (snapshot, target_frame, raster_request) = context
                            .media
                            .image_snapshot_for_layout(&image.source, media_layout);
                        let raster_request = raster_request?;
                        snapshot.texture.map(|texture| {
                            (
                                texture,
                                Some(crate::media::MediaTextureKey::new(
                                    image.source,
                                    raster_request,
                                )),
                                Some(media_layout),
                                target_frame,
                                visual.background_radius.get(),
                                1.0,
                                visual.primitive_clip,
                                visual.primitive_clip_mask,
                            )
                        })
                    });
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((visual.frame, stroke_width, visual.border_color))
                } else {
                    None
                };
                if background.is_none()
                    && border.is_none()
                    && backdrop_blur.is_none()
                    && brush.is_none()
                    && texture.is_none()
                {
                    return None;
                }
                let paints_surface = visual.opacity > 0.0
                    && (backdrop_blur
                        .as_ref()
                        .is_some_and(|primitive| primitive.blur_radius > 0.0)
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.background.a > 0
                        || (visual.border_width > Dp::ZERO && visual.border_color.a > 0));
                Some(ReactiveScenePropertyValue::Offset {
                    background,
                    border,
                    backdrop_blur,
                    brush,
                    texture,
                    container_occluder: paints_surface.then_some((
                        self.id,
                        visual.frame,
                        visual.primitive_clip,
                    )),
                })
            }
            PropertySlot::Scale => {
                let ResolvedWidgetKind::Container {
                    layout, children, ..
                } = &self.kind
                else {
                    return None;
                };
                if !children.is_empty()
                    || self.interactions.has_any()
                    || self.focus.focusable.is_some()
                    || self.focus.tab_index.is_some()
                    || self.focus.scope.is_some()
                    || visual.runtime_visual.shadow.is_some()
                {
                    return None;
                }
                if self
                    .container_has_stable_semantic_hit(visual.disabled, context)
                    .unwrap_or(false)
                    || layout.scroll_view.is_some()
                {
                    return None;
                }
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none();
                let background = if draws_base_background
                    && !visual.background_frame.is_empty()
                    && visual.background.a > 0
                {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let backdrop_blur = if !visual.background_frame.is_empty() {
                    let blur_radius = visual
                        .runtime_visual
                        .background_blur
                        .resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BackgroundBlur,
                            context.now,
                            context.units,
                        )
                        .max(0.0);
                    (blur_radius > 0.0
                        || matches!(&visual.runtime_visual.background_blur, Value::Signal(_)))
                    .then_some(BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    })
                } else {
                    None
                };
                let brush = if !visual.background_frame.is_empty() {
                    visual
                        .runtime_visual
                        .background_brush
                        .as_ref()
                        .map(|brush| BrushPrimitive {
                            rect: visual.background_frame,
                            brush: brush.resolve_widget().with_alpha_factor(visual.opacity),
                            corner_radius: visual.background_radius.get(),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                } else {
                    None
                };
                let texture = visual
                    .runtime_visual
                    .background_image
                    .as_ref()
                    .and_then(|image| {
                        let image = image.resolve_widget();
                        let media_layout = crate::media::MediaTextureLayout::new(
                            visual.background_frame,
                            image.fit,
                            context.units.scale_factor(),
                        );
                        let (snapshot, target_frame, raster_request) = context
                            .media
                            .image_snapshot_for_layout(&image.source, media_layout);
                        let raster_request = raster_request?;
                        snapshot.texture.map(|texture| {
                            (
                                texture,
                                Some(crate::media::MediaTextureKey::new(
                                    image.source,
                                    raster_request,
                                )),
                                Some(media_layout),
                                target_frame,
                                visual.background_radius.get(),
                                1.0,
                                visual.primitive_clip,
                                visual.primitive_clip_mask,
                            )
                        })
                    });
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((
                        visual.frame,
                        stroke_width,
                        visual.border_color,
                        visual.border_radius.get(),
                    ))
                } else {
                    None
                };
                if background.is_none()
                    && border.is_none()
                    && backdrop_blur.is_none()
                    && brush.is_none()
                    && texture.is_none()
                {
                    return None;
                }
                let paints_surface = visual.opacity > 0.0
                    && (backdrop_blur
                        .as_ref()
                        .is_some_and(|primitive| primitive.blur_radius > 0.0)
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.background.a > 0
                        || (visual.border_width > Dp::ZERO && visual.border_color.a > 0));
                Some(ReactiveScenePropertyValue::Scale {
                    background,
                    border,
                    backdrop_blur,
                    brush,
                    texture,
                    container_occluder: paints_surface.then_some((
                        self.id,
                        visual.frame,
                        visual.primitive_clip,
                    )),
                })
            }
            PropertySlot::TextColor => {
                let ResolvedWidgetKind::Text { text, .. } = &self.kind else {
                    return None;
                };
                let color = text.color.as_ref()?.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::TextColor,
                    context.now,
                );
                Some(ReactiveScenePropertyValue::TextColor {
                    color: color.with_alpha_factor(visual.opacity),
                })
            }
            PropertySlot::TextContent => match &self.kind {
                ResolvedWidgetKind::Text { text, .. } => {
                    self.resolve_fixed_text_content(text, context)
                }
                ResolvedWidgetKind::TextEditor {
                    controller,
                    placeholder,
                    multiline,
                    show_scrollbar,
                    auto_wrap,
                    ..
                } => {
                    if *multiline || context.focused_input == Some(self.id) {
                        return None;
                    }
                    if !has_static_fixed_frame(&self.layout) {
                        return None;
                    }
                    let input_style = visual.styles.input_style.as_ref()?;
                    let show_scrollbar = show_scrollbar.resolve();
                    let auto_wrap = auto_wrap.resolve();
                    let padding = Insets::symmetric(input_style.padding_x, input_style.padding_y);
                    let content_viewport = text_input_content_viewport(
                        visual.frame,
                        padding,
                        *multiline,
                        show_scrollbar,
                        context.theme,
                        context.units,
                    );
                    let content = controller.text();
                    let is_placeholder = content.is_empty();
                    let display_content = if is_placeholder {
                        placeholder.resolve()
                    } else {
                        content
                    };
                    if display_content.contains('\n') {
                        return None;
                    }
                    let text = text_with_typography("", &input_style.text_style);
                    let text_color = context.animations.resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::TextColor,
                        },
                        if is_placeholder {
                            input_style.placeholder
                        } else {
                            input_style.text
                        },
                        default_state_transition(context.style_context),
                        context.now,
                    );
                    let (font_size, line_height, letter_spacing) =
                        resolved_text_metrics(&text, context.theme, context.units);
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: text.font_weight.unwrap_or(default_style.weight),
                    };
                    let layout = if auto_wrap {
                        context.font_manager.measure_text_layout_wrapped(
                            &display_content,
                            text_request.clone(),
                            font_size,
                            line_height,
                            letter_spacing,
                            text_input_layout_width(
                                content_viewport,
                                *multiline,
                                auto_wrap,
                                CARET_WIDTH,
                            ),
                        )
                    } else {
                        context.font_manager.measure_text_layout(
                            &display_content,
                            text_request.clone(),
                            font_size,
                            line_height,
                            letter_spacing,
                        )
                    };
                    let TextInputContentGeometry { content_frame, .. } =
                        text_input_content_geometry(
                            &layout,
                            line_height,
                            content_viewport,
                            *multiline,
                            auto_wrap,
                            Point::ZERO,
                            CARET_WIDTH,
                        );
                    let resolved = context
                        .font_manager
                        .resolve_text(&display_content, text_request);
                    let content_clip_rect = visual
                        .primitive_clip
                        .map(|clip| clip.intersect(content_viewport))
                        .unwrap_or(Some(content_viewport));
                    Some(ReactiveScenePropertyValue::TextInputContent(
                        TextPrimitive {
                            content: std::sync::Arc::from(display_content),
                            rich_spans: None,
                            frame: content_frame,
                            quad: None,
                            color: text_color.with_alpha_factor(visual.opacity),
                            force_color: false,
                            font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                            font_size,
                            font_weight: text.font_weight.unwrap_or(default_style.weight),
                            line_height,
                            letter_spacing,
                            wrap: crate::ui::widget::CanvasTextWrap::Word,
                            overflow: crate::ui::widget::CanvasTextOverflow::Clip,
                            horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
                            vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
                            clip_rect: content_clip_rect,
                            clip_mask: visual.primitive_clip_mask,
                        },
                    ))
                }
                _ => None,
            },
            PropertySlot::ProgressValue => {
                let ResolvedWidgetKind::ProgressBar {
                    value,
                    indeterminate,
                    show_label,
                    label,
                    ..
                } = &self.kind
                else {
                    return None;
                };
                if indeterminate.resolve() {
                    return None;
                }
                let style = visual.styles.progress_bar_style.as_ref()?;
                let progress = normalized_progress_value(value.resolve());
                let track_rect = progress_bar_track_rect(
                    visual.frame,
                    style,
                    *show_label,
                    context.theme,
                    context.units,
                );
                if track_rect.width <= Dp::ZERO {
                    return None;
                }
                let fill_width = Dp::new(track_rect.width.get() * progress).min(track_rect.width);
                let track_color = style
                    .track_color
                    .resolve()
                    .with_alpha_factor(visual.opacity);
                let fill_color = style.fill_color.resolve().with_alpha_factor(visual.opacity);
                if track_color == fill_color {
                    return None;
                }
                let label = if *show_label && label.is_none() {
                    let content = format!("{:.0}%", progress * 100.0);
                    let label_text =
                        text_with_typography(Value::Static(content.clone()), &style.text_style);
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: label_text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: label_text.font_weight.unwrap_or(default_style.weight),
                    };
                    let resolved = context.font_manager.resolve_text(&content, text_request);
                    Some(ReactiveProgressLabel {
                        frame: progress_bar_label_frame(
                            visual.frame,
                            style,
                            context.theme,
                            context.units,
                        ),
                        content: std::sync::Arc::from(content),
                        font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                    })
                } else {
                    None
                };
                Some(ReactiveScenePropertyValue::ProgressFill {
                    track_rect,
                    fill_rect: Rect::new(track_rect.x, track_rect.y, fill_width, track_rect.height),
                    track_color,
                    fill_color,
                    label,
                })
            }
            PropertySlot::SliderValue => self.resolve_slider_value_with_style(
                visual.frame,
                visual.opacity,
                visual.styles.slider_style.as_ref()?,
                context,
            ),
            PropertySlot::Width
            | PropertySlot::Height
            | PropertySlot::MinWidth
            | PropertySlot::MinHeight
            | PropertySlot::MaxWidth
            | PropertySlot::MaxHeight
            | PropertySlot::Margin
            | PropertySlot::Padding
            | PropertySlot::Grow
            | PropertySlot::Shrink
            | PropertySlot::Basis
            | PropertySlot::AspectRatio
            | PropertySlot::GridRow
            | PropertySlot::GridColumn
            | PropertySlot::Inset => None,
        }
    }

    #[inline]
    fn reactive_direct_fallback_cached(&self, bit: u16) -> bool {
        self.reactive_direct_fallback_mask.get() & bit != 0
    }

    #[inline]
    fn cache_reactive_direct_fallback(&self, bit: u16) {
        self.reactive_direct_fallback_mask
            .set(self.reactive_direct_fallback_mask.get() | bit);
    }

    fn resolve_fixed_text_content(
        &self,
        text: &Text,
        context: &CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        if text.user_select || !has_static_fixed_frame(&self.layout) {
            return None;
        }
        let content = text.content.resolve();
        if content.contains('\n') {
            return None;
        }
        let default_style = &context.theme.typography.body;
        let text_request = TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(default_style.font_family.as_deref()),
            weight: text.font_weight.unwrap_or(default_style.weight),
        };
        let resolved = context.font_manager.resolve_text(&content, text_request);
        Some(ReactiveScenePropertyValue::TextContent {
            content: std::sync::Arc::from(content),
            font_family: Some(std::sync::Arc::from(resolved.primary_font)),
        })
    }

    fn resolve_text_color(
        &self,
        text: &Text,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        let color = text.color.as_ref()?;
        // Text has no component-level disabled specialization, but its runtime surface can still
        // be stateful through the stylesheet or a local style resolver.
        let widget_state = self.collect_widget_state(false, context);
        let (_, runtime_visual) = self.resolve_runtime_visual(widget_state, context);
        let opacity = visual_context.opacity
            * track_property_scope(PropertySlot::Opacity, || {
                runtime_visual.opacity.resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Opacity,
                    context.now,
                    0.0,
                    1.0,
                )
            });
        let color = color.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::TextColor,
            context.now,
        );
        Some(ReactiveScenePropertyValue::TextColor {
            color: color.with_alpha_factor(opacity),
        })
    }

    fn resolve_plain_text_opacity(
        &self,
        text: &Text,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        if text.user_select {
            return PlainContainerDirectResolve::Ineligible;
        }
        let widget_state = self.collect_widget_state(false, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        text_opacity_direct_probe::record_attempt();
        if runtime_background.is_some()
            || !matches!(&runtime_visual.opacity, Value::Signal(_))
            || runtime_visual.shadow.is_some()
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || !matches!(&runtime_visual.background_blur, Value::Static(value) if *value <= Dp::ZERO)
            || runtime_visual.border_color.is_some()
            || runtime_visual.border_width.is_some()
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let color = text
            .color
            .as_ref()
            .map(|color| {
                color.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::TextColor,
                    context.now,
                )
            })
            .unwrap_or(context.theme.colors.on_surface)
            .with_alpha_factor(opacity);
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::Opacity {
            shadow: None,
            background: None,
            border: None,
            text: Some(color),
            container_occluder: None,
        })
    }

    fn resolve_plain_container_opacity(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &self.kind
        else {
            return PlainContainerDirectResolve::Ineligible;
        };
        if !children.is_empty()
            || container_layout.scroll_view.is_some()
            || self.interactions.has_any()
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
            || self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some()
        {
            return PlainContainerDirectResolve::Ineligible;
        }

        let widget_state = self.collect_widget_state(false, context);
        if widget_state != WidgetState::default() {
            return PlainContainerDirectResolve::Ineligible;
        }
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        container_opacity_direct_probe::record_attempt();
        if !matches!(&runtime_visual.opacity, Value::Signal(_))
            || !matches!(&runtime_background, Some(Value::Static(color)) if color.a > 0)
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(&runtime_visual.background_blur, Value::Static(value) if *value <= Dp::ZERO)
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || runtime_visual
                .border_width
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return PlainContainerDirectResolve::PreparedFallback(
                    PreparedCollectVisualRuntime {
                        disabled: false,
                        widget_state,
                        runtime_background,
                        runtime_visual,
                    },
                );
            }
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(stroke_width)));
        if background_frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let background_color = runtime_background
            .as_ref()
            .expect("plain opacity candidate requires a static background")
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Background,
                context.now,
            )
            .with_alpha_factor(opacity);
        let border_color = runtime_visual
            .border_color
            .as_ref()
            .map(|color| {
                color.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderColor,
                    context.now,
                )
            })
            .unwrap_or(Color::TRANSPARENT)
            .with_alpha_factor(opacity);
        let border = (stroke_width > 0.0).then_some((frame, stroke_width, border_color));
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::Opacity {
            shadow: None,
            background: Some((background_frame, background_color)),
            border,
            text: None,
            container_occluder: Some(
                opacity > 0.0
                    && (background_color.a > 0 || (stroke_width > 0.0 && border_color.a > 0)),
            ),
        })
    }

    fn resolve_plain_slider_value(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> SliderValueDirectResolve {
        let ResolvedWidgetKind::Slider {
            value,
            show_ticks,
            show_value_label,
            value_formatter,
            validation,
            runtime_style,
            ..
        } = &self.kind
        else {
            return SliderValueDirectResolve::Ineligible;
        };

        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        #[cfg(all(test, feature = "bench-support"))]
        slider_value_direct_probe::record_attempt();

        let plain_visual = self.background.is_none()
            && self.visual.border_color.is_none()
            && self.visual.border_radius.is_none()
            && self.visual.border_width.is_none()
            && self.visual.background_brush.is_none()
            && self.visual.background_image.is_none()
            && self.visual.shadow.is_none()
            && matches!(&self.visual.background_blur, Value::Static(value) if *value <= Dp::ZERO)
            && matches!(&self.visual.offset, Value::Static(_))
            && matches!(&self.visual.scale, Value::Static(_))
            && matches!(&self.visual.opacity, Value::Static(_));
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();
        // Slider installs a static pointer cursor by default. The retained Slider hit keeps that
        // cursor and only rewrites value/geometry, so exclude semantic handlers while allowing the
        // cursor-only default interaction record.
        let has_interaction_handlers = self.interactions.on_click.is_some()
            || self.interactions.on_double_click.is_some()
            || self.interactions.on_focus.is_some()
            || self.interactions.on_blur.is_some()
            || self.interactions.on_mouse_enter.is_some()
            || self.interactions.on_mouse_leave.is_some()
            || self.interactions.on_mouse_move.is_some()
            || self.interactions.on_file_drop.is_some()
            || self.interactions.gesture.is_some();
        if !matches!(value, Value::Signal(_)) {
            return SliderValueDirectResolve::Ineligible;
        }
        if *show_ticks
            || (*show_value_label && value_formatter.is_some())
            || !plain_visual
            || has_interaction_handlers
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || has_runtime_overlay
            || has_runtime_role
        {
            return SliderValueDirectResolve::StickyPreparedFallback(
                self.prepare_slider_value_fallback(disabled, widget_state, context),
            );
        }

        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return SliderValueDirectResolve::TransientPreparedFallback {
                    prepared: self.prepare_slider_value_fallback(disabled, widget_state, context),
                    slider_style: None,
                };
            }
        };
        let Value::Static(offset) = &self.visual.offset else {
            unreachable!("plain SliderValue requires a static offset")
        };
        let Value::Static(scale) = &self.visual.scale else {
            unreachable!("plain SliderValue requires a static scale")
        };
        let Value::Static(opacity) = &self.visual.opacity else {
            unreachable!("plain SliderValue requires a static opacity")
        };
        if !scale.is_finite()
            || !opacity.is_finite()
            || !offset.x.get().is_finite()
            || !offset.y.get().is_finite()
            || !visual_context.origin.x.get().is_finite()
            || !visual_context.origin.y.get().is_finite()
            || !visual_context.opacity.is_finite()
            || !layout.location.x.is_finite()
            || !layout.location.y.is_finite()
            || !layout.size.width.is_finite()
            || !layout.size.height.is_finite()
        {
            return SliderValueDirectResolve::TransientPreparedFallback {
                prepared: self.prepare_slider_value_fallback(disabled, widget_state, context),
                slider_style: None,
            };
        }
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = scale.clamp(0.01, 16.0);
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        let opacity =
            visual_context.opacity * opacity.clamp(0.0, 1.0) * if disabled { 0.55 } else { 1.0 };

        let mut source_style = runtime_style.base.clone();
        context.style_sheet.apply_slider_state(
            &mut source_style,
            &context.style_context,
            &self.visual,
            widget_state,
        );
        let source_style = apply_local_style_with_state(
            runtime_style.local.as_ref(),
            source_style,
            &context.style_context,
            context.style_sheet,
            &self.visual,
            widget_state,
        );
        let mut style = resolve_slider_style(&source_style, widget_state, context.theme);
        let validation = validation.resolve();
        let validation_color = if validation.invalid {
            Some(context.theme.colors.error)
        } else if validation.pending {
            Some(context.theme.colors.primary)
        } else {
            None
        };
        if let Some(color) = validation_color {
            style.active_track = color;
            style.tick = color.with_alpha_factor(0.55);
            if let Some(focus_ring) = style.focus_ring.as_mut() {
                focus_ring.color = color;
            }
        }

        // `Value<T>::PartialEq` resolves signals, so comparing the whole surface with
        // `WidgetSurfaceStyle::default()` would misclassify an equal-valued live binding as a
        // static surface and would also attribute that speculative read to SliderValue. Preserve
        // the already-resolved control style for this transient full-visual fallback.
        if !slider_surface_is_static_default(&source_style.surface) {
            return SliderValueDirectResolve::TransientPreparedFallback {
                prepared: self.prepare_slider_value_fallback(disabled, widget_state, context),
                slider_style: Some(style),
            };
        }

        match self.resolve_slider_value_with_style(frame, opacity, &style, context) {
            Some(value) => SliderValueDirectResolve::Resolved(value),
            None => SliderValueDirectResolve::TransientPreparedFallback {
                prepared: self.prepare_slider_value_fallback(disabled, widget_state, context),
                slider_style: Some(style),
            },
        }
    }

    fn prepare_slider_value_fallback(
        &self,
        disabled: bool,
        widget_state: WidgetState,
        context: &CollectContext<'_, '_>,
    ) -> PreparedCollectVisualRuntime {
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        PreparedCollectVisualRuntime {
            disabled,
            widget_state,
            runtime_background,
            runtime_visual,
        }
    }

    fn resolve_slider_value_with_style(
        &self,
        frame: Rect,
        opacity: f32,
        style: &ResolvedSliderStyle,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        let ResolvedWidgetKind::Slider {
            value,
            min,
            max,
            step,
            orientation,
            show_ticks,
            show_value_label,
            value_formatter,
            ..
        } = &self.kind
        else {
            return None;
        };
        if *show_ticks || style.thumb_shadow.is_some() {
            return None;
        }

        let mut geometry =
            slider_geometry(frame, style, *orientation, *show_value_label, context.units);
        if geometry.track_rect.width <= Dp::ZERO
            || geometry.track_rect.height <= Dp::ZERO
            || geometry.thumb_rect.width <= Dp::ZERO
            || geometry.thumb_rect.height <= Dp::ZERO
        {
            return None;
        }

        let resolved_value =
            crate::ui::widget::common::slider_resolve_value(value.resolve(), *min, *max, *step);
        let display_value = context
            .active_slider_value
            .filter(|(widget_id, _)| *widget_id == self.id)
            .map(|(_, raw_value)| {
                crate::ui::widget::common::slider_resolve_value(raw_value, *min, *max, *step)
            })
            .unwrap_or(resolved_value);
        let normalized =
            crate::ui::widget::common::slider_normalized_value(display_value, *min, *max, *step)
                .clamp(0.0, 1.0);
        let thumb_offset = if orientation.is_horizontal() {
            Dp::new(geometry.track_rect.width.get() * normalized)
        } else {
            Dp::new(geometry.track_rect.height.get() * (1.0 - normalized))
        };
        let active_extent = if orientation.is_horizontal() {
            Dp::new(geometry.track_rect.width.get() * normalized)
        } else {
            Dp::new(geometry.track_rect.height.get() * normalized)
        };

        let active_rect = if orientation.is_horizontal() {
            Rect::new(
                geometry.track_rect.x,
                geometry.track_rect.y,
                active_extent.min(geometry.track_rect.width),
                geometry.track_rect.height,
            )
        } else {
            let height = active_extent.min(geometry.track_rect.height);
            Rect::new(
                geometry.track_rect.x,
                geometry.track_rect.bottom() - height,
                geometry.track_rect.width,
                height,
            )
        };

        if orientation.is_horizontal() {
            geometry.thumb_rect.x =
                (geometry.track_rect.x + thumb_offset - (geometry.thumb_rect.width * 0.5)).clamp(
                    frame.x,
                    (frame.right() - geometry.thumb_rect.width).max(frame.x),
                );
        } else {
            let min_y = geometry.track_rect.y - (geometry.thumb_rect.height * 0.5);
            let max_y = geometry.track_rect.bottom() - (geometry.thumb_rect.height * 0.5);
            let min_y = min_y.max(frame.y);
            let max_y = max_y
                .min((frame.bottom() - geometry.thumb_rect.height).max(frame.y))
                .max(min_y);
            geometry.thumb_rect.y = (geometry.track_rect.y + thumb_offset
                - (geometry.thumb_rect.height * 0.5))
                .clamp(min_y, max_y);
        }

        let transition = default_state_transition(context.style_context);
        let track_color = context
            .animations
            .resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::SliderTrackColor,
                },
                style.track,
                transition,
                context.now,
            )
            .with_alpha_factor(opacity);
        let active_track_color = context
            .animations
            .resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::SliderActiveTrackColor,
                },
                style.active_track,
                transition,
                context.now,
            )
            .with_alpha_factor(opacity);
        if track_color == active_track_color {
            return None;
        }
        let thumb_color = context
            .animations
            .resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::SliderThumbColor,
                },
                style.thumb,
                transition,
                context.now,
            )
            .with_alpha_factor(opacity);
        let thumb_border_width = context
            .units
            .resolve_dp(style.border_width)
            .max(0.0)
            .min((geometry.thumb_rect.width.get() * 0.5).max(0.0));
        let thumb_border = (thumb_border_width > 0.0).then_some((track_color, thumb_border_width));
        let label = if *show_value_label {
            let content = value_formatter
                .as_ref()
                .map(|formatter| formatter.format(display_value))
                .unwrap_or_else(|| format!("{display_value:.2}"));
            if content.contains('\n') {
                return None;
            }
            let label_text =
                text_with_typography(Value::Static(content.clone()), &style.text_style);
            let default_style = &context.theme.typography.body;
            let text_request = TextFontRequest {
                preferred_font: label_text
                    .font_family
                    .as_deref()
                    .or(default_style.font_family.as_deref()),
                weight: label_text.font_weight.unwrap_or(default_style.weight),
            };
            let resolved = context.font_manager.resolve_text(&content, text_request);
            let (_, line_height, _) =
                resolved_text_metrics(&label_text, context.theme, context.units);
            Some(ReactiveProgressLabel {
                frame: Rect::new(frame.x, frame.y, frame.width, Dp::new(line_height)),
                content: std::sync::Arc::from(content),
                font_family: Some(std::sync::Arc::from(resolved.primary_font)),
            })
        } else {
            None
        };

        Some(ReactiveScenePropertyValue::SliderValue {
            widget_id: self.id,
            value: display_value,
            track_rect: geometry.track_rect,
            active_rect,
            thumb_rect: geometry.thumb_rect,
            track_color,
            active_track_color,
            thumb_color,
            thumb_border,
            label,
        })
    }

    fn resolve_progress_value(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        let ResolvedWidgetKind::ProgressBar {
            value,
            indeterminate,
            show_label,
            label,
            runtime_style,
            ..
        } = &self.kind
        else {
            return None;
        };

        // A progress bar is not an interactive/sticky container, but retain the conservative
        // fallback if an internal role ever attaches one to a data-grid or other transformed
        // scene so that frame/clip metadata cannot silently diverge from full recollection.
        if self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.list_item.is_some()
            || self.tree_node.is_some()
        {
            return None;
        }
        if indeterminate.resolve() {
            return None;
        }

        let layout = context.taffy.layout(layout_node.node).ok()?;
        let offset = track_property_scope(PropertySlot::Offset, || {
            self.visual.offset.resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Offset,
                context.now,
            )
        });
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = track_property_scope(PropertySlot::Scale, || {
            if context.reduced_motion {
                self.visual
                    .scale
                    .clone()
                    .with_default_transition(None)
                    .resolve_widget_clamped(
                        context.animations,
                        self.id,
                        WidgetProperty::Scale,
                        context.now,
                        0.01,
                        16.0,
                    )
            } else {
                self.visual.scale.resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Scale,
                    context.now,
                    0.01,
                    16.0,
                )
            }
        });
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        let opacity = visual_context.opacity
            * track_property_scope(PropertySlot::Opacity, || {
                self.visual.opacity.resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Opacity,
                    context.now,
                    0.0,
                    1.0,
                )
            });

        let widget_state = self.collect_widget_state(false, context);
        let mut style = runtime_style.base.clone();
        context.style_sheet.apply_progress_bar_state(
            &mut style,
            &context.style_context,
            &self.visual,
            widget_state,
        );
        let style = apply_local_style_with_state(
            runtime_style.local.as_ref(),
            style,
            &context.style_context,
            context.style_sheet,
            &self.visual,
            widget_state,
        );
        let progress = track_property_scope(PropertySlot::ProgressValue, || {
            normalized_progress_value(value.resolve())
        });
        let track_rect =
            progress_bar_track_rect(frame, &style, *show_label, context.theme, context.units);
        if track_rect.width <= Dp::ZERO {
            return None;
        }
        let fill_width = Dp::new(track_rect.width.get() * progress).min(track_rect.width);
        let track_color = style.track_color.resolve().with_alpha_factor(opacity);
        let fill_color = style.fill_color.resolve().with_alpha_factor(opacity);
        if track_color == fill_color {
            return None;
        }
        let label = if *show_label && label.is_none() {
            let content = format!("{:.0}%", progress * 100.0);
            let label_text =
                text_with_typography(Value::Static(content.clone()), &style.text_style);
            let default_style = &context.theme.typography.body;
            let text_request = TextFontRequest {
                preferred_font: label_text
                    .font_family
                    .as_deref()
                    .or(default_style.font_family.as_deref()),
                weight: label_text.font_weight.unwrap_or(default_style.weight),
            };
            let resolved = context.font_manager.resolve_text(&content, text_request);
            Some(ReactiveProgressLabel {
                frame: progress_bar_label_frame(frame, &style, context.theme, context.units),
                content: std::sync::Arc::from(content),
                font_family: Some(std::sync::Arc::from(resolved.primary_font)),
            })
        } else {
            None
        };
        Some(ReactiveScenePropertyValue::ProgressFill {
            track_rect,
            fill_rect: Rect::new(track_rect.x, track_rect.y, fill_width, track_rect.height),
            track_color,
            fill_color,
            label,
        })
    }

    fn resolve_plain_container_background(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        if !matches!(
            self.kind,
            ResolvedWidgetKind::Container { .. } | ResolvedWidgetKind::Virtual { .. }
        ) || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
        {
            return PlainContainerDirectResolve::Ineligible;
        }

        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        if !matches!(&runtime_background, Some(Value::Signal(_)))
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(_))
            || runtime_visual
                .border_width
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || !matches!(
                &runtime_visual.background_blur,
                Value::Static(value) if *value <= Dp::ZERO
            )
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let Ok(layout) = context.taffy.layout(layout_node.node) else {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let border_width = runtime_visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(border_width)));
        if background_frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let background = runtime_background
            .as_ref()
            .expect("plain background candidate requires a signal")
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Background,
                context.now,
            )
            .with_alpha_factor(opacity);
        let border_color = runtime_visual
            .border_color
            .as_ref()
            .map(|color| {
                color
                    .resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderColor,
                        context.now,
                    )
                    .with_alpha_factor(opacity)
            })
            .unwrap_or(Color::TRANSPARENT);
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::ShapeFillColor {
            rect: background_frame,
            color: background,
            container_occluder: self
                .container_has_stable_semantic_hit(disabled, context)
                .map(|has_semantic_hit| {
                    !has_semantic_hit
                        && opacity > 0.0
                        && (background.a > 0 || (border_width > 0.0 && border_color.a > 0))
                }),
        })
    }

    fn resolve_plain_container_background_blur(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &self.kind
        else {
            return PlainContainerDirectResolve::Ineligible;
        };
        let widget_state = self.collect_widget_state(false, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        if !matches!(&runtime_visual.background_blur, Value::Signal(_))
            || runtime_visual.shadow.is_some()
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(_))
            || runtime_background
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_width
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        if !children.is_empty()
            || container_layout.scroll_view.is_some()
            || widget_state != WidgetState::default()
            || self.interactions.has_any()
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
            || self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some()
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let Ok(layout) = context.taffy.layout(layout_node.node) else {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(stroke_width)));
        if background_frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let border_radius = runtime_visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0);
        let background_radius = (border_radius - stroke_width).max(0.0);
        let blur_radius = runtime_visual
            .background_blur
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BackgroundBlur,
                context.now,
                context.units,
            )
            .max(0.0);
        let background_color = runtime_background
            .as_ref()
            .map(|background| {
                background.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Background,
                    context.now,
                )
            })
            .unwrap_or(Color::TRANSPARENT)
            .with_alpha_factor(opacity);
        let border_color = runtime_visual
            .border_color
            .as_ref()
            .map(|color| {
                color.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderColor,
                    context.now,
                )
            })
            .unwrap_or(Color::TRANSPARENT)
            .with_alpha_factor(opacity);

        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::BackdropBlur {
            primitive: BackdropBlurPrimitive {
                rect: background_frame,
                corner_radius: background_radius,
                blur_radius,
                clip_rect: Some(visual_context.clip_rect),
                clip_mask: visual_context.clip_mask,
            },
            container_occluder: Some(
                opacity > 0.0
                    && (blur_radius > 0.0
                        || background_color.a > 0
                        || (stroke_width > 0.0 && border_color.a > 0)),
            ),
        })
    }

    fn resolve_plain_container_background_brush(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &self.kind
        else {
            return PlainContainerDirectResolve::Ineligible;
        };
        let widget_state = self.collect_widget_state(false, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        if !matches!(&runtime_visual.background_brush, Some(Value::Signal(_)))
            || runtime_visual.shadow.is_some()
            || runtime_visual.background_image.is_some()
            || !matches!(
                &runtime_visual.background_blur,
                Value::Static(value) if *value <= Dp::ZERO
            )
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(_))
            || runtime_background
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_width
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        if !children.is_empty()
            || container_layout.scroll_view.is_some()
            || widget_state != WidgetState::default()
            || self.interactions.has_any()
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
            || self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some()
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let Ok(layout) = context.taffy.layout(layout_node.node) else {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(stroke_width)));
        if background_frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let border_radius = runtime_visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0);
        let background_radius = (border_radius - stroke_width).max(0.0);
        let brush = runtime_visual
            .background_brush
            .as_ref()
            .expect("plain background brush candidate requires a signal")
            .resolve_widget()
            .with_alpha_factor(opacity);

        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::Brush(BrushPrimitive {
            rect: background_frame,
            brush,
            corner_radius: background_radius,
            clip_rect: Some(visual_context.clip_rect),
            clip_mask: visual_context.clip_mask,
        }))
    }

    fn resolve_plain_container_offset(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &self.kind
        else {
            return PlainContainerDirectResolve::Ineligible;
        };

        let widget_state = self.collect_widget_state(false, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        if !matches!(&runtime_visual.offset, Value::Signal(_))
            || !matches!(
                &runtime_visual.scale,
                Value::Static(value) if (*value - 1.0).abs() <= f32::EPSILON
            )
            || !matches!(&runtime_visual.opacity, Value::Static(value) if *value > 0.0)
            || !matches!(&runtime_background, Some(Value::Static(_)))
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(
                &runtime_visual.background_blur,
                Value::Static(value) if *value <= Dp::ZERO
            )
            || runtime_visual.border_width.is_some()
            || runtime_visual.border_color.is_some()
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        if !children.is_empty()
            || container_layout.overflow_x != Overflow::Hidden
            || container_layout.overflow_y != Overflow::Hidden
            || container_layout.scroll_view.is_some()
            || widget_state != WidgetState::default()
            || self.interactions.has_any()
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
            || self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some()
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let Ok(layout) = context.taffy.layout(layout_node.node) else {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let background = runtime_background
            .as_ref()
            .expect("plain offset candidate requires a static background")
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Background,
                context.now,
            )
            .with_alpha_factor(opacity);
        if opacity <= 0.0 || background.a == 0 {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled: false,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::Offset {
            background: Some((frame, background)),
            border: None,
            backdrop_blur: None,
            brush: None,
            texture: None,
            container_occluder: Some((self.id, frame, Some(visual_context.clip_rect))),
        })
    }

    fn resolve_plain_container_scale(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &self.kind
        else {
            return PlainContainerDirectResolve::Ineligible;
        };

        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        scale_direct_probe::record_attempt();

        if disabled
            || widget_state != WidgetState::default()
            || !has_static_fixed_frame(&self.layout)
            || !matches!(&runtime_visual.scale, Value::Signal(_))
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(opacity) if *opacity > 0.0)
            || !matches!(&runtime_background, Some(Value::Static(color)) if color.a > 0)
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(
                &runtime_visual.background_blur,
                Value::Static(value) if *value <= Dp::ZERO
            )
            || runtime_visual.border_width.is_some()
            || runtime_visual.border_color.is_some()
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|radius| !matches!(radius, Value::Static(_)))
            || !children.is_empty()
            || container_layout.overflow_x != Overflow::Hidden
            || container_layout.overflow_y != Overflow::Hidden
            || container_layout.scroll_view.is_some()
            || self.interactions.has_any()
            || self.lifecycle_events.has_any()
            || self.media_events.has_any()
            || self.focus.focusable.is_some()
            || self.focus.tab_index.is_some()
            || self.focus.scope.is_some()
            || self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
            || self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some()
            || visual_context.clip_mask.is_some()
            || self
                .container_has_stable_semantic_hit(disabled, context)
                .unwrap_or(false)
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return PlainContainerDirectResolve::PreparedFallback(
                    PreparedCollectVisualRuntime {
                        disabled,
                        widget_state,
                        runtime_background,
                        runtime_visual,
                    },
                );
            }
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let scale = if context.reduced_motion {
            runtime_visual
                .scale
                .clone()
                .with_default_transition(None)
                .resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Scale,
                    context.now,
                    0.01,
                    16.0,
                )
        } else {
            runtime_visual.scale.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Scale,
                context.now,
                0.01,
                16.0,
            )
        };
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let background = runtime_background
            .as_ref()
            .expect("plain scale candidate requires a static background")
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Background,
                context.now,
            )
            .with_alpha_factor(opacity);
        if opacity <= 0.0 || background.a == 0 {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let radius = runtime_visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0);

        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::Scale {
            background: Some((frame, background, radius)),
            border: None,
            backdrop_blur: None,
            brush: None,
            texture: None,
            container_occluder: Some((self.id, frame, Some(visual_context.clip_rect))),
        })
    }

    fn resolve_plain_container_border_color(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        if !matches!(self.kind, ResolvedWidgetKind::Container { .. })
            || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
        {
            return PlainContainerDirectResolve::Ineligible;
        }

        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        border_color_direct_probe::record_attempt();
        if !matches!(&runtime_visual.border_color, Some(Value::Signal(_)))
            || !matches!(&runtime_background, Some(Value::Static(color)) if color.a > 0)
            || !matches!(&runtime_visual.border_width, Some(Value::Static(width)) if *width > Dp::ZERO)
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(opacity) if *opacity > 0.0)
            || !matches!(
                &runtime_visual.background_blur,
                Value::Static(value) if *value <= Dp::ZERO
            )
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return PlainContainerDirectResolve::PreparedFallback(
                    PreparedCollectVisualRuntime {
                        disabled,
                        widget_state,
                        runtime_background,
                        runtime_visual,
                    },
                );
            }
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .expect("plain border candidate requires a static width")
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BorderWidth,
                context.now,
                context.units,
            )
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        if stroke_width <= 0.0 {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let color = runtime_visual
            .border_color
            .as_ref()
            .expect("plain border candidate requires an explicit signal")
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::BorderColor,
                context.now,
            )
            .with_alpha_factor(opacity);
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::ShapeStrokeColor {
            rect: frame,
            stroke_width,
            color,
        })
    }

    fn resolve_plain_container_border_radius(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        if !matches!(
            &self.kind,
            ResolvedWidgetKind::Container { children, .. } if children.is_empty()
        ) || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
        {
            return PlainContainerDirectResolve::Ineligible;
        }

        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        border_radius_direct_probe::record_attempt();
        if !matches!(&runtime_visual.border_radius, Some(Value::Signal(_)))
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(&runtime_visual.background_blur, Value::Static(value) if *value <= Dp::ZERO)
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(_))
            || runtime_background
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_width
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return PlainContainerDirectResolve::PreparedFallback(
                    PreparedCollectVisualRuntime {
                        disabled,
                        widget_state,
                        runtime_background,
                        runtime_visual,
                    },
                );
            }
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }

        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let radius = runtime_visual
            .border_radius
            .as_ref()
            .expect("plain radius candidate requires an explicit signal")
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BorderRadius,
                context.now,
                context.units,
            )
            .max(0.0);
        let background_frame = frame.inset(Insets::all(Dp::new(stroke_width)));
        let background = runtime_background.as_ref().and_then(|background| {
            let color = background.resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Background,
                context.now,
            );
            (!background_frame.is_empty() && color.a > 0).then_some((
                background_frame,
                color.with_alpha_factor(opacity),
                (radius - stroke_width).max(0.0),
            ))
        });
        let border = runtime_visual.border_color.as_ref().and_then(|color| {
            let color = color
                .resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderColor,
                    context.now,
                )
                .with_alpha_factor(opacity);
            (stroke_width > 0.0 && color.a > 0).then_some((frame, stroke_width, color, radius))
        });
        if background.is_none() && border.is_none() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::BorderRadius {
            background,
            border,
        })
    }

    fn resolve_plain_container_border_width(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> PlainContainerDirectResolve {
        if !matches!(
            &self.kind,
            ResolvedWidgetKind::Container { children, .. } if children.is_empty()
        ) || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
        {
            return PlainContainerDirectResolve::Ineligible;
        }
        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        #[cfg(all(test, feature = "bench-support"))]
        border_width_direct_probe::record_attempt();
        if !matches!(&runtime_visual.border_width, Some(Value::Signal(_)))
            || runtime_visual.background_brush.is_some()
            || runtime_visual.background_image.is_some()
            || runtime_visual.shadow.is_some()
            || !matches!(&runtime_visual.background_blur, Value::Static(value) if *value <= Dp::ZERO)
            || !matches!(&runtime_visual.offset, Value::Static(_))
            || !matches!(&runtime_visual.scale, Value::Static(_))
            || !matches!(&runtime_visual.opacity, Value::Static(_))
            || runtime_background
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_radius
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
            || runtime_visual
                .border_color
                .as_ref()
                .is_some_and(|value| !matches!(value, Value::Static(_)))
        {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let layout = match context.taffy.layout(layout_node.node) {
            Ok(layout) => layout,
            Err(_) => {
                return PlainContainerDirectResolve::PreparedFallback(
                    PreparedCollectVisualRuntime {
                        disabled,
                        widget_state,
                        runtime_background,
                        runtime_visual,
                    },
                );
            }
        };
        let offset = runtime_visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let mut frame = Rect::new(
            visual_context.origin.x + layout.location.x + offset.x,
            visual_context.origin.y + layout.location.y + offset.y,
            layout.size.width,
            layout.size.height,
        );
        let scale = runtime_visual.scale.resolve_widget_clamped(
            context.animations,
            self.id,
            WidgetProperty::Scale,
            context.now,
            0.01,
            16.0,
        );
        if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            frame = Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            );
        }
        if frame.is_empty() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        let opacity = visual_context.opacity
            * runtime_visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let stroke_width = runtime_visual
            .border_width
            .as_ref()
            .expect("plain width candidate requires an explicit signal")
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BorderWidth,
                context.now,
                context.units,
            )
            .max(0.0)
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let radius = runtime_visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or(0.0)
            .max(0.0);
        let background_frame = frame.inset(Insets::all(Dp::new(stroke_width)));
        let background = runtime_background.as_ref().and_then(|background| {
            let color = background
                .resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Background,
                    context.now,
                )
                .with_alpha_factor(opacity);
            (!background_frame.is_empty() && color.a > 0).then_some((
                background_frame,
                color,
                (radius - stroke_width).max(0.0),
            ))
        });
        let border = runtime_visual.border_color.as_ref().and_then(|color| {
            let color = color
                .resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderColor,
                    context.now,
                )
                .with_alpha_factor(opacity);
            (stroke_width > 0.0 && color.a > 0).then_some((frame, color, stroke_width))
        });
        if background.is_none() && border.is_none() {
            return PlainContainerDirectResolve::PreparedFallback(PreparedCollectVisualRuntime {
                disabled,
                widget_state,
                runtime_background,
                runtime_visual,
            });
        }
        PlainContainerDirectResolve::Resolved(ReactiveScenePropertyValue::BorderWidth {
            frame,
            background,
            border,
        })
    }

    fn container_background_occluder_state(
        &self,
        visual: &CollectVisualState,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<bool> {
        if !matches!(self.kind, ResolvedWidgetKind::Container { .. }) {
            return None;
        }

        let background_blur = visual
            .runtime_visual
            .background_blur
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BackgroundBlur,
                context.now,
                context.units,
            )
            .max(0.0);
        self.container_surface_occluder_state(visual, background_blur, context)
    }

    fn container_surface_occluder_state(
        &self,
        visual: &CollectVisualState,
        background_blur: f32,
        context: &CollectContext<'_, '_>,
    ) -> Option<bool> {
        if self.container_has_stable_semantic_hit(visual.disabled, context)? {
            return Some(false);
        }

        Some(
            visual.opacity > 0.0
                && (background_blur > 0.0
                    || visual.runtime_visual.shadow.is_some()
                    || visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_some()
                    || visual.background.a > 0
                    || (visual.border_width > Dp::ZERO && visual.border_color.a > 0)),
        )
    }

    fn container_has_stable_semantic_hit(
        &self,
        disabled: bool,
        context: &CollectContext<'_, '_>,
    ) -> Option<bool> {
        let ResolvedWidgetKind::Container { layout, .. } = &self.kind else {
            return None;
        };
        let fallback_focusable = layout.scroll_view.is_some()
            || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some();
        let has_focus_hit =
            context.focus.disabled_depth == 0 && self.focus.focusable.unwrap_or(fallback_focusable);
        Some(
            disabled
                || self.interactions.has_any()
                || has_focus_hit
                || self.list_item.is_some()
                || self.tree_node.is_some()
                || self.data_grid_cell.is_some()
                || self.data_grid_header.is_some()
                || self.data_grid_resize_handle.is_some(),
        )
    }

    fn border_color_slot_preserves_hit_topology(&self, visual: &CollectVisualState) -> bool {
        let ResolvedWidgetKind::Container { layout, .. } = &self.kind else {
            return true;
        };

        // A Container with semantic interaction metadata already owns a stable hit region; its
        // border alpha cannot add or remove the fallback surface occluder.
        if self.interactions.has_any()
            || layout.scroll_view.is_some()
            || self.list_item.is_some()
            || self.tree_node.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
        {
            return true;
        }

        // A fully transparent inherited/runtime surface cannot paint an occluder during this
        // BorderColor update. Opacity changes have their own dependency and fallback decision.
        if visual.opacity <= 0.0 {
            return true;
        }

        // Otherwise `push_surface_primitives_and_base_hit_regions` emits an Occluder exactly when
        // the Container paints a surface. Retaining only the stroke color is safe if another
        // stable surface contribution keeps that hit topology present across transparent/visible
        // border revisions. Without one, fall back to bounded subtree recollection so hit regions
        // stay equivalent to a fresh full recollect.
        visual.runtime_visual.shadow.is_some()
            || visual.runtime_visual.background_brush.is_some()
            || visual.runtime_visual.background_image.is_some()
            || matches!(
                &visual.runtime_visual.background_blur,
                Value::Static(value) if *value > Dp::ZERO
            )
            || (!visual.reactive_background && visual.background.a > 0)
    }

    pub(super) fn can_skip_when_fully_clipped(&self) -> bool {
        if !self.clip_cull_safe_self() {
            return false;
        }

        match &self.kind {
            ResolvedWidgetKind::Text { text, .. } => !text.user_select,
            ResolvedWidgetKind::Container {
                layout, children, ..
            } => {
                layout.overflow_x != Overflow::Scroll
                    && layout.overflow_y != Overflow::Scroll
                    && children
                        .iter()
                        .all(ResolvedElement::can_skip_when_fully_clipped)
            }
            _ => false,
        }
    }

    fn retained_transform_record_candidate(&self, visual: &CollectVisualState) -> bool {
        if !visual.reactive_offset {
            return false;
        }
        let ResolvedWidgetKind::Container { layout, .. } = &self.kind else {
            return false;
        };
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();

        layout.overflow_x == Overflow::Visible
            && layout.overflow_y == Overflow::Visible
            && layout.scroll_view.is_none()
            && !self.interactions.has_any()
            && !self.lifecycle_events.has_any()
            && !self.media_events.has_any()
            && self.focus.focusable.is_none()
            && self.focus.tab_index.is_none()
            && self.focus.scope.is_none()
            && !has_runtime_overlay
            && !has_runtime_role
            && visual.runtime_visual.shadow.is_none()
            && matches!(&visual.runtime_visual.background_blur, Value::Static(value) if *value == Dp::ZERO)
            && matches!(&visual.runtime_visual.scale, Value::Static(value) if (*value - 1.0).abs() <= f32::EPSILON)
    }

    fn collected_scene_allows_retained_transform(&self, computed: &ComputedScene<VM>) -> bool {
        let scene = &computed.scene;
        computed
            .hit_regions
            .iter()
            .all(|hit| hit.supports_retained_transform())
            && computed.overlay_hit_regions.is_empty()
            && computed.overlay_close_handlers.is_empty()
            && computed.focus_scopes.is_empty()
            && computed.carousel_auto_play.is_empty()
            && computed.overlay_anchors.is_empty()
            && computed.portal_entries.is_empty()
            && computed.external_portal_requests.is_empty()
            && computed.ime_cursor_area.is_none()
            && computed.virtual_state_updates.is_empty()
            && computed.scroll_regions.iter().all(|region| {
                region.overflow_x == Overflow::Visible && region.overflow_y == Overflow::Visible
            })
            && scene.backdrop_blurs.is_empty()
            && scene.brushes.is_empty()
            && scene.canvas_composites.is_empty()
            && scene.meshes.is_empty()
            && scene.shapes.iter().all(|shape| shape.clip_mask.is_none())
            && scene
                .textures
                .iter()
                .all(|texture| texture.clip_mask.is_none())
            && scene.texts.iter().all(|text| text.clip_mask.is_none())
            && scene
                .text_decorations
                .iter()
                .all(|decoration| decoration.clip_mask.is_none())
            && {
                #[cfg(feature = "video")]
                {
                    scene
                        .video_textures
                        .iter()
                        .all(|texture| texture.clip_mask.is_none())
                }
                #[cfg(not(feature = "video"))]
                {
                    true
                }
            }
            && scene.overlay_shapes.is_empty()
            && scene.overlay_textures.is_empty()
            && scene.overlay_meshes.is_empty()
            && scene.overlay_texts.is_empty()
            && scene.overlay_text_decorations.is_empty()
            && scene.overlay_commands.is_empty()
            && scene.overlay_command_gpu_scroll_containers.is_empty()
            && scene.overlay_command_transform_chains.is_empty()
    }

    fn may_emit_runtime_overlay(&self) -> bool {
        self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some()
    }

    fn clip_cull_safe_self(&self) -> bool {
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();

        !self.interactions.has_any()
            && !self.lifecycle_events.has_any()
            && !self.media_events.has_any()
            && self.focus.focusable.is_none()
            && self.focus.tab_index.is_none()
            && self.focus.scope.is_none()
            && !has_runtime_overlay
            && !has_runtime_role
            && self.visual.shadow.is_none()
            && matches!(&self.visual.background_blur, Value::Static(value) if *value == Dp::ZERO)
            && matches!(&self.visual.offset, Value::Static(value) if *value == Point::ZERO)
            && matches!(&self.visual.scale, Value::Static(value) if *value == 1.0)
    }

    /// 收集 `self` 子树的场景 chunk,把结果**移动**进 `chunks[self.id]`,
    /// 并返回该节点的 `WidgetId`。
    ///
    /// 旧实现这里返回整棵合并后的子树(owned `ComputedScene`),再被父节点 `extend`
    /// 后丢弃 —— 也就是说每个节点都把自己的合并子树深拷贝了两次(一次进 `chunks`,
    /// 一次作为返回值)。现在结果只在 `chunks` 里存一份:
    /// - 父节点通过 `chunks.get(&child.id)` 只读引用来 `extend`,不再深拷贝;
    /// - 需要 owned 场景的根/overlay 调用方在收集结束后 `chunks.get(&id).cloned()`,
    ///   仅根节点保留这一次必要的克隆。
    pub(in super::super) fn collect_subtree_cache(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        super::super::tree::with_widget_stack_frame(|| {
            let owner = self.id.dependency_owner(DependencyPhase::Scene);
            track_dependency_scope(owner, || {
                self.collect_subtree_cache_tracked(
                    layout_node,
                    visual_context,
                    context,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        })
    }

    fn collect_subtree_cache_tracked(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        let previous_scope_path = context.focus.scope_path.clone();
        let previous_disabled_depth = context.focus.disabled_depth;
        let (focus_scope_active, suppress_inactive_interactions) = self
            .focus
            .scope
            .as_ref()
            .map(|scope| {
                let active = scope.is_active();
                (Some(active), scope.suppresses_interactions(active))
            })
            .unwrap_or((None, false));
        let lifecycle_state_ids_before = suppress_inactive_interactions
            .then(|| lifecycle_states.keys().copied().collect::<HashSet<_>>());
        if let Some(active) = focus_scope_active {
            context.focus.scope_path.push(self.id);
            if !active {
                context.focus.disabled_depth += 1;
            }
        }
        let mut caches = CollectCaches {
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
        };
        self.collect_runtime_lifecycle_state(caches.lifecycle_states);

        let mut computed = ComputedScene::default();
        computed
            .scene
            .set_active_gpu_scroll_container(context.gpu_scroll_container);
        computed
            .scene
            .set_active_transform_chain(&context.transform_stack);
        if let Some(scope) = self.focus.scope.as_ref() {
            computed.register_focus_scope(FocusScopeState {
                scope_id: self.id,
                path: context.focus.scope_path.clone(),
                options: scope.clone(),
                active: focus_scope_active.unwrap_or(true),
            });
        }
        use super::super::collect_profile::{record_node, record_node_visible, timed, Phase};
        record_node();
        let visual = timed(Phase::VisualState, || {
            self.resolve_collect_visual_state(layout_node, visual_context, context)
        });
        if let Some(geometry) = context.portal_accessibility_geometry.as_deref_mut() {
            geometry.push(super::super::scene::PortalAccessibilityGeometryRecord {
                resolved_path: context.portal_accessibility_path.clone(),
                widget_id: self.id,
                frame: visual.frame,
                clip_rect: visual.primitive_clip,
            });
        }
        let previous_transform_stack_len = context.transform_stack.len();
        let retained_transform_candidate = context.portal_accessibility_geometry.is_none()
            && self.retained_transform_record_candidate(&visual);
        if retained_transform_candidate {
            context.transform_stack.push(self.id);
            computed
                .scene
                .set_active_transform_chain(&context.transform_stack);
        }
        // 节点 frame 与视口相交即记一次「可见」，配合 `record_node` 给出 recollect/visible 比值。
        if visual.frame.intersect(context.viewport).is_some() {
            record_node_visible();
        }
        timed(Phase::Surface, || {
            self.push_surface_primitives_and_base_hit_regions(&mut computed, context, &visual)
        });
        if let Some(auto_play) = self.carousel_auto_play.as_ref() {
            let mut auto_play = auto_play.clone();
            auto_play.frame = visual.frame;
            computed.carousel_auto_play.push(auto_play);
        }

        let kind_handled = timed(Phase::KindBody, || {
            self.collect_layout_media_kind(
                layout_node,
                visual_context,
                context,
                &mut caches,
                &mut computed,
                &visual,
            )
        });
        if !kind_handled {
            let handled = self.collect_control_kind(context, &mut computed, &visual);
            debug_assert!(
                handled,
                "unhandled widget kind in collect_subtree_cache_tracked"
            );
        }
        self.clear_closed_modal_interactions(&mut computed);
        self.clear_closed_drawer_interactions(&mut computed);
        // `before_overlays` 仅在 `Container`/`Virtual` 节点上被消费(用于把 overlay 增量
        // 并入 `chunk_parts.after_children`)。游标只记录各流长度和少量可覆盖 metadata，
        // 避免带 runtime overlay 的深层容器为一次 delta 深拷贝整棵已收集子树。
        let is_container_like = matches!(
            self.kind,
            ResolvedWidgetKind::Container { .. } | ResolvedWidgetKind::Virtual { .. }
        );
        let before_overlays = (is_container_like && self.may_emit_runtime_overlay())
            .then(|| SceneDeltaSnapshot::capture(&computed));

        self.emit_tooltip_if_visible(context, &mut computed, &visual, caches.lifecycle_states);
        self.emit_popover_overlay_if_visible(context, &mut computed, &visual);
        self.emit_menu_overlay_if_open(context, &mut computed, &visual);
        self.emit_modal_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_drawer_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_toast_overlay_if_visible(context, &mut computed, &visual);
        self.emit_portal_if_open(context, &mut computed, &visual, caches.lifecycle_states);

        if let Some(before_overlays) = before_overlays {
            let overlay_delta = before_overlays.delta(&computed);
            if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                parts.after_children.extend(&overlay_delta);
            }
        }

        if let Some(ids_before) = lifecycle_state_ids_before.as_ref() {
            // Inactive view-stack content is still collected for its exit animation. Keep it out
            // of runtime lifecycle membership, including lifecycle states emitted by nested
            // Portals that are not represented by the owning SceneLayout subtree.
            caches
                .lifecycle_states
                .retain(|widget_id, _| ids_before.contains(widget_id));
        }

        if suppress_inactive_interactions {
            // Preserve only this scope's inactive sentinel. Besides documenting
            // the logical state in the scene, its presence prevents the simple
            // splice path from bypassing this gate on a later descendant patch.
            let own_scope = computed
                .focus_scopes
                .iter()
                .find(|scope| scope.scope_id == self.id)
                .cloned();
            computed.clear_interactive_subtree_channels();
            if let Some(scope) = own_scope {
                computed.register_focus_scope(scope);
            }
        }

        timed(Phase::Bookkeeping, || {
            // `chunk_parts` 只被 `recompose_scene_chunk` 读取(scene_layout.rs),而 recompose
            // 仅在「祖先」节点上运行(scene_patch.rs / bench_support.rs 都由 `parent_of` 向上推导
            // 祖先集合),且仅对 `Container` / `Virtual` 这两种带子节点的 kind 迭代子 chunk。
            // 其余 kind 在 resolved 树里都是叶子,永远不会成为祖先 → 它们的 `chunk_parts` 是
            // 只写不读的死数据。而 `Container` / `Virtual` 收集臂(layout_media.rs)又各自显式
            // `insert` 了自己的 `chunk_parts`,所以这里的兜底 `or_insert_with` 对它们也已是空操作。
            //
            // 因此把兜底克隆限定到带子节点的 kind:对叶子(占节点总数绝大多数,且每个都要
            // `before_children: computed.clone()`)直接跳过,消除逐帧重收集里最大的单项独占开销
            // (n=1000 长列表约 22%)。即便未来出现意外读取,`recompose` 的 `chunk_parts.get(&id)?`
            // 会得到 `None` 并安全回退到整帧重收集,绝不会产生错误渲染。
            debug_assert!(
                !is_container_like || caches.chunk_parts.contains_key(&self.id),
                "container-like collection must install SceneChunkParts"
            );
            if let Some(gpu_scroll_container) = context.gpu_scroll_container {
                computed.fill_gpu_scroll_container(gpu_scroll_container);
                if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                    parts
                        .before_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                    parts
                        .after_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                }
            }
            if retained_transform_candidate
                && self.collected_scene_allows_retained_transform(&computed)
            {
                computed.transform_records.insert(
                    self.id,
                    TransformRecord {
                        id: self.id,
                        base_offset: visual.offset,
                        current_offset: visual.offset,
                    },
                );
            }
            context
                .transform_stack
                .truncate(previous_transform_stack_len);
            computed
                .scene
                .set_active_transform_chain(&context.transform_stack);
            caches
                .visual_contexts
                .insert(self.id, visual_context.into());
            context.focus.scope_path = previous_scope_path;
            context.focus.disabled_depth = previous_disabled_depth;
            // 把合并后的子树移动进 chunks(此前是 `clone` + 返回 owned 的双份拷贝)。
            // 父节点改为 `chunks.get(&child.id)` 只读引用来 extend。
            caches.chunks.insert(self.id, computed);
        });
        self.id
    }

    fn collect_runtime_lifecycle_state(
        &self,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
    ) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            lifecycle_states.insert(
                self.id,
                LifecycleEventState {
                    widget_id: self.id,
                    snapshot: lifecycle_snapshot(self),
                    handlers: self.lifecycle_events.clone(),
                },
            );
        }
    }
}
