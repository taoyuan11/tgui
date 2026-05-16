use crate::foundation::color::Color;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Insets, Value};
use crate::ui::theme::{Shadow, Stateful, TextStyle};
use crate::ui::unit::{dp, Dp};

use super::super::common::ButtonVariantKind;
use super::palette::{
    body_text_style, border_hover_lighten, hover_lighten, label_text_style, palette,
    stateful_colors, stateful_single, surface_hover_lighten,
};
use super::shared::{FocusRingOverride, WidgetSurfaceStyle};

/// 按钮 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub foreground: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl ButtonStyle {
    /// 按解析后的主题模式和按钮变体创建默认样式。
    pub fn default_for(mode: ResolvedThemeMode, variant: ButtonVariantKind) -> Self {
        let palette = palette(mode);
        let (background, foreground, border, border_width) = match variant {
            ButtonVariantKind::Primary => (
                stateful_colors(
                    palette.primary,
                    palette.primary.lighten(hover_lighten()),
                    palette.primary.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                stateful_single(
                    palette.on_primary,
                    palette.on_primary,
                    palette.on_primary,
                    palette.disabled_content,
                ),
                stateful_colors(
                    palette.primary,
                    palette.primary.lighten(hover_lighten()),
                    palette.primary.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                dp(0.0),
            ),
            ButtonVariantKind::Secondary => (
                stateful_colors(
                    palette.surface,
                    palette.surface.lighten(surface_hover_lighten()),
                    palette.surface.darken(surface_hover_lighten()),
                    palette.disabled_surface,
                ),
                stateful_single(
                    palette.on_surface,
                    palette.primary.lighten(hover_lighten()),
                    palette.primary.darken(hover_lighten()),
                    palette.disabled_content,
                ),
                stateful_colors(
                    palette.outline,
                    palette.primary.lighten(hover_lighten()),
                    palette.primary.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                dp(1.0),
            ),
            ButtonVariantKind::Ghost => (
                stateful_colors(
                    Color::TRANSPARENT,
                    palette.surface_high.lighten(surface_hover_lighten()),
                    palette.surface_high.darken(surface_hover_lighten()),
                    Color::TRANSPARENT,
                ),
                stateful_single(
                    palette.on_surface,
                    palette.on_surface,
                    palette.on_surface,
                    palette.disabled_content,
                ),
                stateful_colors(
                    Color::TRANSPARENT,
                    Color::TRANSPARENT,
                    Color::TRANSPARENT,
                    Color::TRANSPARENT,
                ),
                dp(0.0),
            ),
            ButtonVariantKind::Danger => (
                stateful_colors(
                    palette.error,
                    palette.error.lighten(hover_lighten()),
                    palette.error.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                stateful_single(
                    palette.on_error,
                    palette.on_error,
                    palette.on_error,
                    palette.disabled_content,
                ),
                stateful_colors(
                    palette.error,
                    palette.error.lighten(hover_lighten()),
                    palette.error.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                dp(0.0),
            ),
        };

        Self {
            surface: WidgetSurfaceStyle::default(),
            background,
            foreground,
            border,
            focus_ring: None,
            border_width: Value::Static(border_width),
            radius: Value::Static(dp(8.0)),
            padding_x: dp(8.0),
            padding_y: dp(4.0),
            min_height: dp(32.0),
            text_style: label_text_style(),
        }
    }
}

/// 复选框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub background_checked: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub border_checked: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub checkmark: Stateful<Value<Color>>,
    pub label: Stateful<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

impl CheckboxStyle {
    /// 按解析后的主题模式创建默认复选框样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_single(
                palette.surface_low,
                palette.surface_low,
                palette.surface_low,
                palette.disabled_surface,
            ),
            background_checked: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            border: stateful_single(
                palette.outline,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            border_checked: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            focus_ring: None,
            checkmark: stateful_single(
                palette.on_primary,
                palette.on_primary,
                palette.on_primary,
                palette.disabled_content,
            ),
            label: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(8.0)),
            size: dp(16.0),
            label_gap: dp(8.0),
            text_style: label_text_style(),
        }
    }
}

/// 单选框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct RadioStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub background_checked: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub border_checked: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub indicator: Stateful<Value<Color>>,
    pub label: Stateful<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

impl RadioStyle {
    /// 按解析后的主题模式创建默认单选框样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_low,
                palette.surface_low.darken(surface_hover_lighten()),
                palette.disabled_surface,
            ),
            background_checked: stateful_colors(
                palette.surface_low,
                palette.surface_low.lighten(surface_hover_lighten()),
                palette.surface_low.darken(surface_hover_lighten()),
                palette.disabled_surface,
            ),
            border: stateful_colors(
                palette.outline,
                palette.outline.lighten(border_hover_lighten()),
                palette.outline.darken(border_hover_lighten()),
                palette.disabled_surface,
            ),
            border_checked: stateful_colors(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_surface,
            ),
            focus_ring: None,
            indicator: stateful_colors(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_content,
            ),
            label: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(999.0)),
            size: dp(16.0),
            label_gap: dp(8.0),
            text_style: label_text_style(),
        }
    }
}

/// 开关 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStyle {
    pub surface: WidgetSurfaceStyle,
    pub track: Stateful<Value<Color>>,
    pub track_checked: Stateful<Value<Color>>,
    pub thumb: Stateful<Value<Color>>,
    pub thumb_checked: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub border_checked: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding: Insets,
    pub width: Dp,
    pub height: Dp,
}

impl SwitchStyle {
    /// 按解析后的主题模式创建默认开关样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track: stateful_single(
                palette.switch_track,
                palette.switch_track,
                palette.switch_track,
                palette.disabled_surface,
            ),
            track_checked: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            thumb: stateful_single(
                Color::WHITE,
                Color::WHITE,
                Color::WHITE,
                palette.disabled_content,
            ),
            thumb_checked: stateful_single(
                Color::WHITE,
                Color::WHITE,
                Color::WHITE,
                palette.disabled_content,
            ),
            border: stateful_colors(
                palette.outline_muted,
                palette.outline_muted.lighten(border_hover_lighten()),
                palette.outline_muted.darken(border_hover_lighten()),
                palette.disabled_surface,
            ),
            border_checked: stateful_colors(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_surface,
            ),
            focus_ring: None,
            border_width: Value::Static(dp(0.0)),
            radius: Value::Static(dp(999.0)),
            padding: Insets::all(dp(4.0)),
            width: dp(42.0),
            height: dp(24.0),
        }
    }
}

/// 下拉选择 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SelectStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub text: Stateful<Value<Color>>,
    pub placeholder: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub arrow: Stateful<Value<Color>>,
    pub menu_background: Value<Color>,
    pub option_background: Stateful<Value<Color>>,
    pub selected_option_background: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub option_height: Dp,
    pub menu_gap: Dp,
    pub text_style: TextStyle,
}

impl SelectStyle {
    /// 按解析后的主题模式创建默认下拉选择样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_low,
                palette.surface_low,
                palette.disabled_surface,
            ),
            text: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            placeholder: stateful_single(
                palette.on_surface_muted,
                palette.on_surface_muted,
                palette.on_surface_muted,
                palette.disabled_content,
            ),
            border: stateful_colors(
                palette.outline,
                palette.outline,
                palette.outline.darken(border_hover_lighten()),
                palette.disabled_surface,
            ),
            focus_ring: None,
            arrow: stateful_single(
                palette.on_surface_muted,
                palette.on_surface_muted,
                palette.on_surface,
                palette.disabled_content,
            ),
            menu_background: Value::Static(palette.surface),
            option_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            selected_option_background: Value::Static(palette.surface_high),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(12.0)),
            padding_x: dp(16.0),
            padding_y: dp(0.0),
            min_height: dp(40.0),
            option_height: dp(40.0),
            menu_gap: dp(2.0),
            text_style: body_text_style(),
        }
    }
}

/// 滑块 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SliderStyle {
    pub surface: WidgetSurfaceStyle,
    pub track: Stateful<Value<Color>>,
    pub active_track: Stateful<Value<Color>>,
    pub thumb: Stateful<Value<Color>>,
    pub thumb_shadow: Option<Shadow>,
    pub tick: Stateful<Value<Color>>,
    pub label: Stateful<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub track_height: Dp,
    pub thumb_size: Dp,
    pub radius: Value<Dp>,
    pub border_width: Value<Dp>,
    pub tick_size: Dp,
    pub label_gap: Dp,
    pub min_width: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl SliderStyle {
    /// 按解析后的主题模式创建默认滑块样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track: stateful_colors(
                palette.outline_muted,
                palette.outline,
                palette.outline,
                palette.disabled_surface,
            ),
            active_track: stateful_colors(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_content,
            ),
            thumb: stateful_colors(
                palette.surface,
                palette.surface,
                palette.surface,
                palette.disabled_surface,
            ),
            thumb_shadow: None,
            tick: stateful_colors(
                palette.outline_muted,
                palette.outline,
                palette.outline,
                palette.disabled_surface,
            ),
            label: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            focus_ring: None,
            track_height: dp(4.0),
            thumb_size: dp(18.0),
            radius: Value::Static(dp(999.0)),
            border_width: Value::Static(dp(4.5)),
            tick_size: dp(6.0),
            label_gap: dp(8.0),
            min_width: dp(160.0),
            min_height: dp(32.0),
            text_style: label_text_style(),
        }
    }
}

/// 单行输入框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct InputStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub text: Stateful<Value<Color>>,
    pub placeholder: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub selection: Option<Value<Color>>,
    pub caret: Option<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl InputStyle {
    /// 按解析后的主题模式创建默认输入框样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_low,
                palette.surface_low,
                palette.disabled_surface,
            ),
            text: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            placeholder: stateful_single(
                palette.on_surface_muted,
                palette.on_surface_muted,
                palette.on_surface_muted,
                palette.disabled_content,
            ),
            border: stateful_colors(
                palette.outline,
                palette.outline,
                palette.outline,
                palette.disabled_surface,
            ),
            selection: None,
            caret: None,
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(12.0)),
            padding_x: dp(12.0),
            padding_y: dp(8.0),
            min_height: dp(40.0),
            text_style: body_text_style(),
        }
    }
}

/// 多行文本框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TextareaStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Stateful<Value<Color>>,
    pub text: Stateful<Value<Color>>,
    pub placeholder: Stateful<Value<Color>>,
    pub border: Stateful<Value<Color>>,
    pub selection: Option<Value<Color>>,
    pub caret: Option<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl TextareaStyle {
    /// 按解析后的主题模式创建默认多行文本框样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let mut style = InputStyle::default_for(mode);
        style.min_height = dp(96.0);
        Self {
            surface: style.surface,
            background: style.background,
            text: style.text,
            placeholder: style.placeholder,
            border: style.border,
            selection: style.selection,
            caret: style.caret,
            border_width: style.border_width,
            radius: style.radius,
            padding_x: style.padding_x,
            padding_y: style.padding_y,
            min_height: style.min_height,
            text_style: style.text_style,
        }
    }
}
