use super::{
    apply_text_widget_style, centered_text_frame, media_loading_fill_color,
    resolve_stateful_widget_color, resolved_text_metrics, text_with_typography, HitInteraction,
    ResolvedWidgetKind, TextInputLayoutOverride, CARET_WIDTH, CHECKBOX_CHECKMARK_ICON,
    SELECT_ARROW_ICON,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::animation::{AnimationCoordinator, AnimationEngine};
use crate::foundation::binding::{
    DependencyOwner, DependencyPhase, InvalidationSignal, ViewModelContext,
};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::media::{MediaManager, MediaSource};
use crate::text::font::{FontCatalog, FontManager, TextFontRequest};
use crate::ui::layout::{Axis, Insets, Overflow};
use crate::ui::theme::{Shadow, Stateful, Theme};
use crate::ui::unit::{dp, sp, Dp, UnitContext};
use crate::ui::widget::common::{ContainerKind, Rect, WidgetKind};
use crate::ui::widget::style::infer_theme_mode;
use crate::ui::widget::{
    BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient, BackgroundRadialGradient,
};
use crate::ui::widget::{
    ButtonStyle, Canvas, CanvasParagraphStyle, CanvasRecorder, CanvasStroke, CanvasStyle,
    CanvasTextHorizontalAlign, CanvasTextVerticalAlign, CanvasTextWrap, Checkbox, ClipMask,
    ContainerStyle, Element, Image, Input, InputStyle, Point, Radio, RadioGroup, RadioOption,
    ScrollbarAxis, ScrollbarHandle, Select, SelectOption, Slider, SliderStyle, Stack, Switch,
    SwitchStyle, Text, TextEditState, TextWidgetStyle, Textarea, TextareaStyle, WidgetStateMap,
    WidgetTree,
};
#[cfg(feature = "video")]
use crate::video::backend::{
    BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
#[cfg(feature = "video")]
use crate::video::{PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSurface};

const ONE_BY_ONE_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x4C,
    0x00, 0x3B,
];

fn stateful<T: Clone>(value: T) -> Stateful<T> {
    Stateful {
        normal: value.clone(),
        hovered: value.clone(),
        pressed: value.clone(),
        disabled: value,
    }
}

fn text_style(
    mode: crate::theme::ResolvedThemeMode,
    size: Option<crate::ui::unit::Sp>,
) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    if let Some(size) = size {
        style.typography.size = size;
    }
    style
}

fn container_style(
    mode: crate::theme::ResolvedThemeMode,
    background: Option<Color>,
    brush: Option<crate::ui::widget::BackgroundBrush>,
    image: Option<BackgroundImage>,
    blur: Option<Dp>,
    shadow: Option<Shadow>,
    border: Option<(Dp, Color)>,
    radius: Option<Dp>,
    offset: Option<Point>,
) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = background.map(Into::into);
    style.surface.background_brush = brush.map(Into::into);
    style.surface.background_image = image.map(Into::into);
    if let Some(blur) = blur {
        style.surface.background_blur = blur.into();
    }
    if let Some(shadow) = shadow {
        style.surface.shadow = Some(shadow.into());
    }
    if let Some((width, color)) = border {
        style.surface.border_width = Some(width.into());
        style.surface.border_color = Some(color.into());
    }
    if let Some(radius) = radius {
        style.surface.border_radius = Some(radius.into());
    }
    if let Some(offset) = offset {
        style.surface.offset = offset.into();
    }
    style
}

fn canvas_style(mode: crate::theme::ResolvedThemeMode, radius: Dp) -> CanvasStyle {
    let mut style = CanvasStyle::default_for(mode);
    style.surface.border_radius = Some(radius.into());
    style
}

fn test_shadow() -> Shadow {
    Shadow {
        offset_x: dp(3.0),
        offset_y: dp(5.0),
        blur: dp(12.0),
        spread: Dp::ZERO,
        color: Color::hexa(0x00000044),
    }
}

fn button_style(
    mode: crate::theme::ResolvedThemeMode,
    radius: Option<Dp>,
    border_width: Option<Dp>,
    border_color: Option<Color>,
) -> ButtonStyle {
    let mut style =
        ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Primary);
    if let Some(radius) = radius {
        style.radius = radius.into();
    }
    if let Some(border_width) = border_width {
        style.border_width = border_width.into();
    }
    if let Some(border_color) = border_color {
        style.border = stateful(border_color.into());
    }
    style
}

fn switch_style(
    mode: crate::theme::ResolvedThemeMode,
    active_background: Color,
    inactive_background: Color,
    active_thumb: Option<Color>,
    inactive_thumb: Option<Color>,
) -> SwitchStyle {
    let mut style = SwitchStyle::default_for(mode);
    style.track_checked = stateful(active_background.into());
    style.track = stateful(inactive_background.into());
    if let Some(active_thumb) = active_thumb {
        style.thumb_checked = stateful(active_thumb.into());
    }
    if let Some(inactive_thumb) = inactive_thumb {
        style.thumb = stateful(inactive_thumb.into());
    }
    style
}

fn resolved_theme_mode(theme: &Theme) -> crate::theme::ResolvedThemeMode {
    infer_theme_mode(theme)
}

fn default_checkbox_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    checked: bool,
) -> super::ResolvedCheckboxStyle {
    super::resolve_checkbox_style(
        &super::WidgetCheckboxStyle::default_for(resolved_theme_mode(theme)),
        state,
        checked,
        theme,
    )
}

fn default_radio_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    checked: bool,
) -> super::ResolvedRadioStyle {
    super::resolve_radio_style(
        &super::WidgetRadioStyle::default_for(resolved_theme_mode(theme)),
        state,
        checked,
        theme,
    )
}

fn default_button_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
    variant: crate::ui::widget::common::ButtonVariantKind,
) -> super::ResolvedButtonStyle {
    super::resolve_button_style(
        &ButtonStyle::default_for(resolved_theme_mode(theme), variant),
        state,
        theme,
    )
}

fn default_switch_style(theme: &Theme) -> super::WidgetSwitchStyle {
    super::WidgetSwitchStyle::default_for(resolved_theme_mode(theme))
}

fn default_select_style(
    theme: &Theme,
    state: crate::ui::theme::WidgetState,
) -> super::ResolvedSelectStyle {
    super::resolve_select_style(
        &super::WidgetSelectStyle::default_for(resolved_theme_mode(theme)),
        state,
        theme,
    )
}

mod binding_scope_tests;
mod button_switch_slider_tests;
mod canvas_tests;
mod command_video_tests;
mod controls_tests;
mod dependency_tests;
mod layout_scroll_tests;
mod select_tests;
mod text_and_background;
mod text_input_tests;

fn test_media() -> MediaManager {
    MediaManager::new(InvalidationSignal::new())
}

fn wait_for_rendered_output(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
) -> super::RenderedWidgetScene {
    for _ in 0..150 {
        let rendered = tree.render_output(
            font_manager,
            theme,
            media,
            animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        );
        if !rendered.primitives.textures.is_empty() {
            return rendered;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    tree.render_output(
        font_manager,
        theme,
        media,
        animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    )
}

fn test_context() -> ViewModelContext {
    ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
}

#[cfg(feature = "video")]
fn test_video_controller(snapshot: crate::video::VideoSurfaceSnapshot) -> VideoController {
    struct StaticVideoBackend;

    impl VideoBackend for StaticVideoBackend {
        fn load(&self, _source: crate::video::VideoSource) -> Result<(), crate::core::TguiError> {
            Ok(())
        }

        fn play(&self) {}
        fn pause(&self) {}
        fn seek(&self, _position: std::time::Duration) {}
        fn set_volume(&self, _volume: f32) {}
        fn set_muted(&self, _muted: bool) {}
        fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}
        fn set_target_raster(&self, _raster: Option<crate::media::RasterRequest>) {}
        fn current_frame(&self) -> Option<std::sync::Arc<crate::media::TextureFrame>> {
            None
        }
        fn shutdown(&self) {}
    }

    let ctx = test_context();
    let shared = BackendSharedState {
        playback_state: ctx.state(PlaybackState::Ready),
        metrics: ctx.state(VideoMetrics {
            duration: Some(std::time::Duration::from_secs(30)),
            position: std::time::Duration::ZERO,
            buffered: Some(std::time::Duration::from_secs(30)),
            video_width: snapshot.intrinsic_size.width as u32,
            video_height: snapshot.intrinsic_size.height as u32,
        }),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        metrics_observed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: snapshot.intrinsic_size.width as u32,
            height: snapshot.intrinsic_size.height as u32,
        }),
        error: ctx.state(snapshot.error.clone()),
        surface: ctx.state(snapshot),
    };
    VideoController::from_parts(shared, std::sync::Arc::new(StaticVideoBackend))
}

#[derive(Default)]
struct ScopeChildVm {
    count: i32,
    checked: bool,
    selected_key: String,
    selected_value: String,
    canvas_hits: usize,
    context_hits: usize,
}

#[derive(Default)]
struct ScopeRootVm {
    child: ScopeChildVm,
    other: ScopeChildVm,
    root_count: i32,
}

fn scope_child(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.child
}

fn scope_other(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.other
}
