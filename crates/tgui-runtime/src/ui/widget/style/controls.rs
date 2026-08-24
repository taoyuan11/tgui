mod avatar;
mod badge;
mod breadcrumb;
mod card;
mod carousel;
mod collapse;
mod combobox;
mod divider;
mod icon;
mod input;
mod pagination;
mod progress;
mod rating;
mod rich_text;
mod skeleton;
mod slider;
mod spinner;
mod splitter;
mod video;

use crate::foundation::color::Color;
use crate::theme::{FontWeight, ResolvedThemeMode};
use crate::ui::layout::{Insets, Value};
use crate::ui::theme::{Density, Shadow, StateValue, TextStyle, Theme};
use crate::ui::unit::{dp, Dp};

use super::super::common::ButtonVariantKind;
use super::palette::{
    border_hover_lighten, hover_lighten, palette_from_theme, stateful_colors, stateful_single,
    surface_hover_lighten,
};
use super::shared::{FocusRingOverride, WidgetSurfaceStyle};

pub use self::avatar::{AvatarShape, AvatarStyle};
pub use self::badge::{BadgeStyle, BadgeTone};
pub use self::breadcrumb::BreadcrumbStyle;
pub use self::card::CardStyle;
pub use self::carousel::CarouselStyle;
pub use self::collapse::CollapseStyle;
pub use self::combobox::ComboboxStyle;
pub use self::divider::DividerStyle;
pub use self::icon::IconStyle;
pub use self::input::{InputStyle, TextareaStyle};
pub use self::pagination::PaginationStyle;
pub use self::progress::ProgressBarStyle;
pub use self::rating::RatingStyle;
pub use self::rich_text::RichTextStyle;
pub use self::skeleton::SkeletonStyle;
pub use self::slider::SliderStyle;
pub use self::spinner::SpinnerStyle;
pub use self::splitter::SplitterStyle;
pub use self::video::VideoStyle;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ControlDensityMetrics {
    pub(crate) control_height: Dp,
    pub(crate) button_padding_x: Dp,
    pub(crate) button_padding_y: Dp,
    pub(crate) input_padding_x: Dp,
    pub(crate) input_padding_y: Dp,
    pub(crate) select_padding_x: Dp,
    pub(crate) selection_size: Dp,
    pub(crate) selection_gap: Dp,
    pub(crate) switch_padding: Dp,
    pub(crate) switch_width: Dp,
    pub(crate) switch_height: Dp,
    pub(crate) slider_track_height: Dp,
    pub(crate) slider_thumb_size: Dp,
    pub(crate) slider_tick_size: Dp,
    pub(crate) slider_label_gap: Dp,
    pub(crate) slider_min_width: Dp,
    pub(crate) slider_min_height: Dp,
}

pub(crate) fn control_density_metrics(theme: &Theme, density: Density) -> ControlDensityMetrics {
    match density {
        Density::Compact => ControlDensityMetrics {
            control_height: dp(32.0),
            button_padding_x: theme.spacing.sm,
            button_padding_y: theme.spacing.xxs,
            input_padding_x: theme.spacing.sm + theme.spacing.xxs,
            input_padding_y: theme.spacing.xs,
            select_padding_x: theme.spacing.sm + theme.spacing.xs,
            selection_size: dp(16.0),
            selection_gap: theme.spacing.sm - theme.spacing.xxs,
            switch_padding: dp(3.0),
            switch_width: dp(36.0),
            switch_height: dp(20.0),
            slider_track_height: dp(3.0),
            slider_thumb_size: theme.spacing.md,
            slider_tick_size: dp(3.0),
            slider_label_gap: theme.spacing.sm - theme.spacing.xxs,
            slider_min_width: dp(144.0),
            slider_min_height: dp(28.0),
        },
        Density::Comfortable => ControlDensityMetrics {
            control_height: dp(40.0),
            button_padding_x: theme.spacing.sm + theme.spacing.xs,
            button_padding_y: theme.spacing.xs,
            input_padding_x: theme.spacing.md - theme.spacing.xs,
            input_padding_y: theme.spacing.sm,
            select_padding_x: theme.spacing.md,
            selection_size: dp(18.0),
            selection_gap: theme.spacing.sm,
            switch_padding: theme.spacing.xs,
            switch_width: dp(40.0),
            switch_height: dp(24.0),
            slider_track_height: dp(4.0),
            slider_thumb_size: theme.spacing.md + theme.spacing.xs,
            slider_tick_size: theme.spacing.xs,
            slider_label_gap: theme.spacing.sm,
            slider_min_width: dp(160.0),
            slider_min_height: dp(32.0),
        },
        Density::Spacious => ControlDensityMetrics {
            control_height: dp(48.0),
            button_padding_x: theme.spacing.md,
            button_padding_y: theme.spacing.sm,
            input_padding_x: theme.spacing.md,
            input_padding_y: theme.spacing.sm + theme.spacing.xxs,
            select_padding_x: theme.spacing.md + theme.spacing.xs,
            selection_size: dp(20.0),
            selection_gap: theme.spacing.sm + theme.spacing.xxs,
            switch_padding: theme.spacing.xs,
            switch_width: dp(48.0),
            switch_height: dp(28.0),
            slider_track_height: dp(5.0),
            slider_thumb_size: theme.spacing.lg,
            slider_tick_size: dp(5.0),
            slider_label_gap: theme.spacing.sm + theme.spacing.xxs,
            slider_min_width: dp(184.0),
            slider_min_height: dp(40.0),
        },
    }
}

/// 按钮 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub foreground: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding_x: Dp,
    pub padding_y: Dp,
    pub min_height: Dp,
    pub text_style: TextStyle,
}

impl ButtonStyle {
    pub fn default_for_theme(theme: &Theme, variant: ButtonVariantKind) -> Self {
        Self::default_for_density(theme, theme.density, variant)
    }

    pub(crate) fn default_for_density(
        theme: &Theme,
        density: Density,
        variant: ButtonVariantKind,
    ) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        Self::from_palette(
            palette,
            variant,
            theme.radius.lg,
            metrics.button_padding_x,
            metrics.button_padding_y,
            metrics.control_height,
            theme.typography.label.clone(),
        )
    }

    fn from_palette(
        palette: super::palette::Palette,
        variant: ButtonVariantKind,
        radius: Dp,
        padding_x: Dp,
        padding_y: Dp,
        min_height: Dp,
        text_style: TextStyle,
    ) -> Self {
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
                    palette.surface_high,
                    palette.surface_high.darken(surface_hover_lighten()),
                    palette.disabled_surface,
                ),
                stateful_single(
                    palette.on_surface,
                    // The surface and border already carry the interaction cue.
                    // Switching label text to the accent color drops below 4.5:1
                    // against `surface_high` in both default themes.
                    palette.on_surface,
                    palette.on_surface,
                    palette.disabled_content,
                ),
                stateful_colors(
                    palette.outline_muted,
                    palette.primary.lighten(hover_lighten()),
                    palette.primary.darken(hover_lighten()),
                    palette.disabled_surface,
                ),
                dp(1.0),
            ),
            ButtonVariantKind::Ghost => (
                stateful_colors(
                    Color::TRANSPARENT,
                    palette.surface_low,
                    palette.surface_high,
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
                    // Keep light-theme danger labels above WCAG AA. Lightening the
                    // default red makes `on_error` fall below 4.5:1 on hover.
                    palette.error.darken(surface_hover_lighten()),
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
                    palette.error.darken(surface_hover_lighten()),
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
            radius: Value::Static(radius),
            padding_x,
            padding_y,
            min_height,
            text_style,
        }
    }
}

/// 复选框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub background_checked: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
    pub border_checked: StateValue<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub checkmark: StateValue<Value<Color>>,
    pub label: StateValue<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

impl CheckboxStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_high,
                palette.surface_low.darken(surface_hover_lighten()),
                palette.disabled_surface,
            ),
            background_checked: stateful_colors(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_surface,
            ),
            border: stateful_colors(
                palette.outline_muted,
                palette.outline.lighten(border_hover_lighten()),
                palette.outline.darken(border_hover_lighten()),
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
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.sm),
            size: metrics.selection_size,
            label_gap: metrics.selection_gap,
            text_style: theme.typography.label.clone(),
        }
    }
}

/// 单选框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct RadioStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub background_checked: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
    pub border_checked: StateValue<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub indicator: StateValue<Value<Color>>,
    pub label: StateValue<Value<Color>>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub size: Dp,
    pub label_gap: Dp,
    pub text_style: TextStyle,
}

impl RadioStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        Self {
            surface: WidgetSurfaceStyle::default(),
            // A quiet outline + dot is both clearer at small sizes and cheaper than stacking an
            // opaque fill under every radio. Filled variants remain available through the public
            // style fields; hover/press feedback stays on the animated outline.
            background: stateful_single(
                Color::TRANSPARENT,
                Color::TRANSPARENT,
                Color::TRANSPARENT,
                Color::TRANSPARENT,
            ),
            background_checked: stateful_single(
                Color::TRANSPARENT,
                Color::TRANSPARENT,
                Color::TRANSPARENT,
                Color::TRANSPARENT,
            ),
            border: stateful_colors(
                palette.outline_muted,
                palette.outline.lighten(border_hover_lighten()),
                palette.outline.darken(border_hover_lighten()),
                palette.disabled_surface,
            ),
            border_checked: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            focus_ring: None,
            indicator: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_content,
            ),
            label: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.full),
            size: metrics.selection_size,
            label_gap: metrics.selection_gap,
            text_style: theme.typography.label.clone(),
        }
    }
}

/// 开关 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStyle {
    pub surface: WidgetSurfaceStyle,
    pub track: StateValue<Value<Color>>,
    pub track_checked: StateValue<Value<Color>>,
    pub thumb: StateValue<Value<Color>>,
    pub thumb_checked: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
    pub border_checked: StateValue<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub padding: Insets,
    pub width: Dp,
    pub height: Dp,
}

impl SwitchStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track: stateful_colors(
                palette.switch_track,
                palette.switch_track.lighten(surface_hover_lighten()),
                palette.switch_track.darken(surface_hover_lighten()),
                palette.disabled_surface,
            ),
            track_checked: stateful_single(
                palette.primary,
                palette.primary,
                palette.primary,
                palette.disabled_surface,
            ),
            thumb: stateful_single(
                palette.surface,
                palette.surface,
                palette.surface,
                palette.disabled_content,
            ),
            thumb_checked: stateful_single(
                palette.on_primary,
                palette.on_primary,
                palette.on_primary,
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
            border_width: Value::Static(theme.border.none),
            radius: Value::Static(theme.radius.full),
            padding: Insets::all(metrics.switch_padding),
            width: metrics.switch_width,
            height: metrics.switch_height,
        }
    }
}

/// 下拉选择 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SelectStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub text: StateValue<Value<Color>>,
    pub placeholder: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
    pub focus_ring: Option<FocusRingOverride>,
    pub arrow: StateValue<Value<Color>>,
    pub menu_background: Value<Color>,
    pub menu_border: Value<Color>,
    pub menu_border_width: Value<Dp>,
    pub menu_radius: Value<Dp>,
    pub option_background: StateValue<Value<Color>>,
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        let mut border = stateful_colors(
            palette.outline_muted,
            palette.outline,
            palette.primary,
            palette.disabled_surface,
        );
        border.focused = Some(Value::Static(palette.primary));
        border.open = Some(Value::Static(palette.primary));
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface,
                palette.surface,
                palette.surface,
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
            border,
            focus_ring: None,
            arrow: stateful_single(
                palette.on_surface_muted,
                palette.on_surface_muted,
                // The border already carries hover/press/open feedback. Keeping the built-in
                // SVG chevron neutral avoids baking every intermediate transition tint into a
                // new SVG source and raster texture during interaction.
                palette.on_surface_muted,
                palette.disabled_content,
            ),
            menu_background: Value::Static(theme.colors.surface_overlay),
            menu_border: Value::Static(theme.colors.outline_muted),
            menu_border_width: Value::Static(theme.border.thin),
            menu_radius: Value::Static(theme.radius.xl),
            option_background: stateful_colors(
                Color::TRANSPARENT,
                palette.primary_container.with_alpha_factor(0.46),
                palette.primary_container.with_alpha_factor(0.62),
                Color::TRANSPARENT,
            ),
            selected_option_background: Value::Static(theme.colors.primary_container),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            padding_x: metrics.select_padding_x,
            padding_y: Dp::ZERO,
            min_height: metrics.control_height,
            option_height: metrics.control_height,
            menu_gap: theme.spacing.xxs,
            text_style: theme.typography.body.clone(),
        }
    }
}

/// Tooltip widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipStyle {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub border_width: Dp,
    pub radius: Dp,
    pub padding: Insets,
    pub max_width: Dp,
    pub offset: Dp,
    pub pointer_size: Dp,
    pub pointer_inset: Dp,
    pub shadow: Shadow,
    pub text_style: TextStyle,
}

impl TooltipStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        let (background, foreground) = match theme.mode {
            ResolvedThemeMode::Light => (palette.on_surface, palette.surface),
            ResolvedThemeMode::Dark => (palette.surface_high, palette.on_surface),
        };
        Self {
            background,
            foreground,
            border: Color::TRANSPARENT,
            border_width: theme.border.none,
            radius: theme.radius.sm,
            padding: Insets::symmetric(theme.spacing.sm, theme.spacing.xs),
            max_width: dp(240.0),
            offset: theme.spacing.sm,
            pointer_size: theme.spacing.sm,
            pointer_inset: theme.spacing.md,
            // Tooltips already use an opaque, high-contrast surface. The text-tooltip renderer
            // interprets this field as a backdrop blur, so a default elevation would add a scene
            // copy plus horizontal, vertical, and composite GPU passes beneath pixels that the
            // opaque bubble immediately covers. Keep the default surface flat and let glassy or
            // elevated variants opt in through `Tooltip::style`.
            shadow: theme.elevation.none.clone(),
            text_style: theme.typography.label.clone(),
        }
    }
}

/// Popover widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct PopoverStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub shadow: Shadow,
    pub padding: Insets,
    pub min_width: Dp,
    pub max_width: Dp,
    pub offset: Dp,
    pub pointer_size: Option<Dp>,
    pub pointer_inset: Dp,
}

impl PopoverStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let menu = MenuStyle::default_for_density(theme, density);
        let (padding, min_width, max_width, offset) = match density {
            Density::Compact => (
                theme.spacing.sm + theme.spacing.xs,
                dp(200.0),
                dp(360.0),
                theme.spacing.sm - theme.spacing.xxs,
            ),
            Density::Comfortable => (theme.spacing.md, dp(220.0), dp(420.0), theme.spacing.sm),
            Density::Spacious => (
                theme.spacing.md + theme.spacing.xs,
                dp(240.0),
                dp(480.0),
                theme.spacing.sm + theme.spacing.xs,
            ),
        };
        Self {
            surface: menu.surface,
            background: menu.background,
            border: menu.border,
            border_width: menu.border_width,
            radius: menu.radius,
            shadow: theme.elevation.md.clone(),
            padding: Insets::all(padding),
            min_width,
            max_width,
            offset,
            pointer_size: None,
            pointer_inset: theme.spacing.lg,
        }
    }
}

/// Menu / ContextMenu widget 共用的样式定义。
///
/// MenuBar 走单独的 [`MenuBarStyle`]——条目水平排布、视觉层级更浅，
/// 与下拉/上下文菜单的浮层语义不同。
#[derive(Clone, Debug, PartialEq)]
pub struct MenuStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub shadow: Shadow,
    /// 浮层最小宽度（不足时按内容撑开到这个值）。
    pub min_width: Dp,
    /// 浮层最大宽度（超过时 label 自动截断）。
    pub max_width: Dp,
    /// 浮层内侧 padding（上下左右）。
    pub padding: Insets,
    /// 单个菜单项内部 padding。
    pub item_padding: Insets,
    pub item_min_height: Dp,
    pub item_background: StateValue<Value<Color>>,
    pub item_foreground: StateValue<Value<Color>>,
    pub item_icon_size: Dp,
    pub item_icon_gap: Dp,
    pub shortcut_color: Value<Color>,
    pub shortcut_gap: Dp,
    pub checked_indicator_color: Value<Color>,
    pub submenu_arrow_size: Dp,
    pub submenu_arrow_color: StateValue<Value<Color>>,
    pub separator_color: Value<Color>,
    pub separator_height: Dp,
    pub separator_inset_x: Dp,
    pub text_style: TextStyle,
}

impl MenuStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let (min_width, max_width, padding, item_padding, item_min_height, icon_size, icon_gap) =
            match density {
                Density::Compact => (
                    dp(144.0),
                    dp(320.0),
                    Insets::all(theme.spacing.xxs),
                    Insets::symmetric(theme.spacing.sm, theme.spacing.xxs),
                    dp(28.0),
                    dp(14.0),
                    theme.spacing.sm - theme.spacing.xxs,
                ),
                Density::Comfortable => (
                    dp(160.0),
                    dp(360.0),
                    Insets::all(theme.spacing.xs),
                    Insets::symmetric(theme.spacing.sm + theme.spacing.xxs, theme.spacing.xs),
                    dp(32.0),
                    theme.spacing.md,
                    theme.spacing.sm,
                ),
                Density::Spacious => (
                    dp(176.0),
                    dp(400.0),
                    Insets::all(theme.spacing.sm - theme.spacing.xxs),
                    Insets::symmetric(
                        theme.spacing.sm + theme.spacing.xs,
                        theme.spacing.sm - theme.spacing.xxs,
                    ),
                    dp(40.0),
                    theme.spacing.md + theme.spacing.xxs,
                    theme.spacing.sm + theme.spacing.xxs,
                ),
            };
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.xl),
            shadow: theme.elevation.md.clone(),
            min_width,
            max_width,
            padding,
            item_padding,
            item_min_height,
            item_background: stateful_colors(
                Color::TRANSPARENT,
                palette.primary_container.with_alpha_factor(0.42),
                palette.primary_container.with_alpha_factor(0.58),
                Color::TRANSPARENT,
            ),
            item_foreground: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            item_icon_size: icon_size,
            item_icon_gap: icon_gap,
            shortcut_color: Value::Static(palette.on_surface_muted),
            shortcut_gap: theme.spacing.lg,
            checked_indicator_color: Value::Static(theme.colors.primary),
            submenu_arrow_size: theme.spacing.sm,
            submenu_arrow_color: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            separator_color: Value::Static(theme.colors.outline_muted),
            separator_height: theme.border.thin,
            separator_inset_x: theme.spacing.sm,
            text_style: theme.typography.label.clone(),
        }
    }
}

/// MenuBar widget 的样式定义。
///
/// 条目在水平条上排布。当 MenuBar 上某个 entry 展开下拉时，下拉菜单本体
/// 走 [`MenuStyle`]——所以 MenuBarStyle 只描述条形外壳和顶级条目本身。
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBarStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    /// 条形整体内 padding。
    pub padding: Insets,
    /// 条形整体高度。
    pub height: Dp,
    /// 顶级条目左右 padding。
    pub entry_padding_x: Dp,
    pub entry_min_width: Dp,
    pub entry_background: StateValue<Value<Color>>,
    pub entry_foreground: StateValue<Value<Color>>,
    /// 条目高亮（active=该 entry 展开了下拉时）的强调色。
    pub entry_active_background: Value<Color>,
    pub entry_gap: Dp,
    pub text_style: TextStyle,
}

impl MenuBarStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
        let (bar_padding_x, entry_gap) = match density {
            Density::Compact => (theme.spacing.xs, theme.spacing.xxs),
            Density::Comfortable => (theme.spacing.sm, theme.spacing.xs),
            Density::Spacious => (theme.spacing.sm + theme.spacing.xs, theme.spacing.sm),
        };
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_low),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.none),
            radius: Value::Static(theme.radius.none),
            // Keep the entry's full control-height hit target. Vertical root padding would be
            // counted on top of an equally tall child and either overflow or shrink that target.
            padding: Insets::symmetric(bar_padding_x, Dp::ZERO),
            height: metrics.control_height,
            entry_padding_x: metrics.button_padding_x,
            entry_min_width: theme.spacing.xxl + theme.spacing.sm,
            entry_background: stateful_colors(
                Color::TRANSPARENT,
                palette.primary_container.with_alpha_factor(0.34),
                palette.primary_container.with_alpha_factor(0.5),
                Color::TRANSPARENT,
            ),
            entry_foreground: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            entry_active_background: Value::Static(theme.colors.surface_high),
            entry_gap,
            text_style: theme.typography.label.clone(),
        }
    }
}

/// Tabs / TabView widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TabsStyle {
    pub surface: WidgetSurfaceStyle,
    pub tab_bar_background: Value<Color>,
    pub panel_background: Value<Color>,
    pub tab_background: StateValue<Value<Color>>,
    pub active_tab_background: Value<Color>,
    pub tab_foreground: StateValue<Value<Color>>,
    pub active_tab_foreground: Value<Color>,
    pub indicator_color: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub tab_padding: Insets,
    pub tab_min_height: Dp,
    pub tab_min_width: Dp,
    pub tab_gap: Dp,
    pub panel_padding: Insets,
    pub indicator_thickness: Dp,
    pub text_style: TextStyle,
}

impl TabsStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let control_metrics = control_density_metrics(theme, density);
        let (tab_min_width, tab_gap, panel_padding) = match density {
            Density::Compact => (dp(64.0), theme.spacing.xxs, theme.spacing.sm),
            Density::Comfortable => (dp(72.0), theme.spacing.xs, theme.spacing.md),
            Density::Spacious => (dp(80.0), theme.spacing.sm, theme.spacing.lg),
        };
        Self {
            surface: WidgetSurfaceStyle::default(),
            tab_bar_background: Value::Static(theme.colors.surface_low),
            panel_background: Value::Static(theme.colors.surface),
            tab_background: stateful_colors(
                Color::TRANSPARENT,
                palette.primary_container.with_alpha_factor(0.34),
                palette.primary_container.with_alpha_factor(0.5),
                Color::TRANSPARENT,
            ),
            active_tab_background: Value::Static(theme.colors.surface),
            tab_foreground: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            active_tab_foreground: Value::Static(theme.colors.primary),
            indicator_color: Value::Static(theme.colors.primary),
            border: Value::Static(theme.colors.outline_muted),
            // The selected tab already has a restrained accent outline. Keeping
            // the strip and panel borderless avoids the old nested-card look.
            border_width: Value::Static(theme.border.none),
            radius: Value::Static(theme.radius.lg),
            tab_padding: Insets::symmetric(
                control_metrics.button_padding_x,
                control_metrics.button_padding_y,
            ),
            tab_min_height: control_metrics.control_height,
            tab_min_width,
            tab_gap,
            panel_padding: Insets::all(panel_padding),
            indicator_thickness: theme.border.thin,
            text_style: theme.typography.label.clone(),
        }
    }
}

/// Modal / Dialog widget 的样式定义。
///
/// Modal 由全屏 backdrop（半透明 scrim）+ 居中 card 组成。card 内有
/// 标题区 / 内容区 / 动作区三段。`enter` / `exit` 过渡走默认
/// `Transition::ease_in_out`（由 collect 阶段统一应用）。
#[derive(Clone, Debug, PartialEq)]
pub struct ModalStyle {
    /// 半透明遮罩颜色。alpha 通道决定基础不透明度，最终 alpha 还会乘以动画 visibility。
    pub backdrop_color: Value<Color>,
    pub surface: WidgetSurfaceStyle,
    /// Card 背景色。
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub shadow: Shadow,
    /// Card 最小宽度。
    pub min_width: Dp,
    /// Card 最大宽度。
    pub max_width: Dp,
    /// Card 最大高度。
    pub max_height: Dp,
    /// Card 外边距（card 与 viewport 边缘的最小留空）。
    pub margin: Insets,
    /// Card 内边距（card 内部所有段落的统一 padding）。
    pub padding: Insets,
    /// 标题文本样式。
    pub title_text_style: TextStyle,
    /// 标题区独立 padding（继承在 card padding 之内）。
    pub title_padding: Insets,
    /// 内容区独立 padding。
    pub content_padding: Insets,
    /// 动作按钮之间的水平间距。
    pub actions_gap: Dp,
    /// 动作区独立 padding。
    pub actions_padding: Insets,
    /// Card 进入动画的起始缩放值。
    pub enter_scale: f32,
}

impl ModalStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let (
            min_width,
            max_width,
            max_height,
            margin,
            title_padding,
            content_padding,
            actions_gap,
            actions_padding,
        ) = match density {
            Density::Compact => (
                dp(272.0),
                dp(480.0),
                dp(560.0),
                theme.spacing.md,
                Insets::symmetric(theme.spacing.md, theme.spacing.sm + theme.spacing.xxs),
                Insets::symmetric(theme.spacing.md, theme.spacing.sm),
                theme.spacing.sm,
                Insets::symmetric(theme.spacing.md, theme.spacing.sm + theme.spacing.xxs),
            ),
            Density::Comfortable => (
                dp(320.0),
                dp(560.0),
                dp(640.0),
                theme.spacing.lg,
                Insets::symmetric(theme.spacing.lg, theme.spacing.md),
                Insets::symmetric(theme.spacing.lg, theme.spacing.sm),
                theme.spacing.sm,
                Insets::symmetric(theme.spacing.lg, theme.spacing.md),
            ),
            Density::Spacious => (
                dp(360.0),
                dp(640.0),
                dp(720.0),
                theme.spacing.xl,
                Insets::symmetric(theme.spacing.xl, theme.spacing.lg),
                Insets::symmetric(theme.spacing.xl, theme.spacing.md),
                theme.spacing.sm + theme.spacing.xs,
                Insets::symmetric(theme.spacing.xl, theme.spacing.lg),
            ),
        };
        Self {
            backdrop_color: Value::Static(theme.colors.scrim),
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.xl),
            shadow: theme.elevation.xl.clone(),
            min_width,
            max_width,
            max_height,
            margin: Insets::all(margin),
            padding: Insets::all(Dp::ZERO),
            title_text_style: theme.typography.title.clone(),
            title_padding,
            content_padding,
            actions_gap,
            actions_padding,
            enter_scale: 0.96,
        }
    }
}

/// Toast / Snackbar widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct ToastStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: Value<Color>,
    pub foreground: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub shadow: Shadow,
    pub padding: Insets,
    pub gap: Dp,
    pub title_text_style: TextStyle,
    pub body_text_style: TextStyle,
    pub icon_size: Dp,
    pub min_width: Dp,
    pub max_width: Dp,
    pub margin: Dp,
    pub stack_gap: Dp,
    pub action_button: ButtonStyle,
    pub close_button: ButtonStyle,
    pub success_icon_background: Value<Color>,
    pub success_icon_foreground: Value<Color>,
    pub error_icon_background: Value<Color>,
    pub error_icon_foreground: Value<Color>,
    pub warning_icon_background: Value<Color>,
    pub warning_icon_foreground: Value<Color>,
    pub info_icon_background: Value<Color>,
    pub info_icon_foreground: Value<Color>,
}

impl ToastStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let (
            padding,
            gap,
            icon_size,
            min_width,
            max_width,
            margin,
            stack_gap,
            action_height,
            close_size,
        ) = match density {
            Density::Compact => (
                Insets::symmetric(theme.spacing.sm + theme.spacing.xs, theme.spacing.sm),
                theme.spacing.sm - theme.spacing.xxs,
                dp(16.0),
                dp(240.0),
                dp(280.0),
                theme.spacing.sm + theme.spacing.xs,
                theme.spacing.sm,
                dp(28.0),
                dp(28.0),
            ),
            Density::Comfortable => (
                Insets::symmetric(theme.spacing.md, theme.spacing.sm + theme.spacing.xs),
                theme.spacing.sm,
                dp(18.0),
                dp(280.0),
                dp(320.0),
                theme.spacing.md,
                theme.spacing.sm + theme.spacing.xs,
                dp(32.0),
                dp(32.0),
            ),
            Density::Spacious => (
                Insets::symmetric(theme.spacing.lg, theme.spacing.md),
                theme.spacing.sm + theme.spacing.xs,
                dp(20.0),
                dp(320.0),
                dp(360.0),
                theme.spacing.lg,
                theme.spacing.md,
                dp(40.0),
                dp(36.0),
            ),
        };
        let mut action_button = ButtonStyle::default_for_theme(
            theme,
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );
        action_button.min_height = action_height;
        action_button.padding_x = theme.spacing.xs;
        action_button.padding_y = theme.spacing.xxs;
        action_button.radius = Value::Static(theme.radius.sm);
        action_button.foreground = stateful_single(
            palette.primary,
            palette.primary.lighten(hover_lighten()),
            palette.primary.darken(hover_lighten()),
            palette.disabled_content,
        );

        let mut close_button = ButtonStyle::default_for_theme(
            theme,
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );
        close_button.min_height = close_size;
        close_button.padding_x = theme.spacing.xxs;
        close_button.padding_y = theme.spacing.xxs;
        close_button.radius = Value::Static(theme.radius.full);
        close_button.foreground = stateful_single(
            palette.on_surface_muted,
            palette.on_surface,
            palette.on_surface,
            palette.disabled_content,
        );

        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            foreground: Value::Static(theme.colors.on_surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            shadow: theme.elevation.lg.clone(),
            padding,
            gap,
            title_text_style: {
                let mut style = theme.typography.label.clone();
                style.weight = FontWeight::Medium;
                style
            },
            body_text_style: {
                let mut style = theme.typography.body_small.clone();
                style.weight = FontWeight::Regular;
                style
            },
            icon_size,
            min_width,
            max_width,
            margin,
            stack_gap,
            action_button,
            close_button,
            success_icon_background: Value::Static(theme.colors.success),
            success_icon_foreground: Value::Static(theme.colors.on_success),
            error_icon_background: Value::Static(theme.colors.error),
            error_icon_foreground: Value::Static(theme.colors.on_error),
            warning_icon_background: Value::Static(theme.colors.warning),
            warning_icon_foreground: Value::Static(theme.colors.on_warning),
            info_icon_background: Value::Static(palette.primary),
            info_icon_foreground: Value::Static(palette.on_primary),
        }
    }
}

/// Drawer / Sidebar widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct DrawerStyle {
    /// 半透明遮罩颜色。alpha 通道决定基础不透明度，最终 alpha 还会乘以动画 visibility。
    pub backdrop_color: Value<Color>,
    pub surface: WidgetSurfaceStyle,
    /// Panel 背景色。
    pub background: Value<Color>,
    pub border: Value<Color>,
    pub border_width: Value<Dp>,
    pub radius: Value<Dp>,
    pub shadow: Shadow,
    /// Panel 宽度（Left/Right placement）。
    pub width: Dp,
    /// Panel 高度（Top/Bottom placement）。
    pub height: Dp,
    /// Panel 内边距。
    pub padding: Insets,
}

impl DrawerStyle {
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let (width, height, padding) = match density {
            Density::Compact => (dp(264.0), dp(208.0), theme.spacing.md),
            Density::Comfortable => (dp(288.0), dp(240.0), theme.spacing.lg),
            Density::Spacious => (dp(320.0), dp(280.0), theme.spacing.xl),
        };
        Self {
            backdrop_color: Value::Static(theme.colors.scrim),
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_overlay),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            // Drawers are viewport-attached sheets. A uniform radius rounds the
            // attached edge too, leaving transparent notches in viewport corners.
            radius: Value::Static(theme.radius.none),
            shadow: theme.elevation.xl.clone(),
            width,
            height,
            padding: Insets::all(padding),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::theme::WidgetState;
    use crate::ui::unit::sp;

    fn relative_luminance(color: Color) -> f32 {
        let linear = |channel: u8| {
            let channel = channel as f32 / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }

    fn contrast_ratio(a: Color, b: Color) -> f32 {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn button_default_for_theme_uses_theme_tokens() {
        let mut theme = Theme::light();
        theme.density = Density::Comfortable;
        theme.colors.primary = Color::hexa(0xCC3366FF);
        theme.radius.lg = dp(10.0);
        theme.typography.label.size = sp(13.0);

        let style = ButtonStyle::default_for_theme(&theme, ButtonVariantKind::Primary);

        assert_eq!(style.background.normal.resolve(), theme.colors.primary);
        assert_eq!(style.border.normal.resolve(), theme.colors.primary);
        assert_eq!(style.radius.resolve(), theme.radius.lg);
        assert_eq!(style.padding_x, dp(12.0));
        assert_eq!(style.min_height, dp(40.0));
        assert_eq!(style.text_style.size, theme.typography.label.size);
    }

    #[test]
    fn button_interaction_states_keep_wcag_aa_text_contrast() {
        for theme in [Theme::light(), Theme::dark()] {
            for variant in [
                ButtonVariantKind::Primary,
                ButtonVariantKind::Secondary,
                ButtonVariantKind::Danger,
            ] {
                let style = ButtonStyle::default_for_theme(&theme, variant);
                for state in [
                    WidgetState::default(),
                    WidgetState {
                        hovered: true,
                        ..WidgetState::default()
                    },
                    WidgetState {
                        pressed: true,
                        ..WidgetState::default()
                    },
                ] {
                    let foreground = style.foreground.resolve(state).resolve();
                    let background = style.background.resolve(state).resolve();
                    assert!(
                        contrast_ratio(foreground, background) >= 4.5,
                        "{variant:?} foreground {foreground:?} on {background:?} misses WCAG AA"
                    );
                }
            }
        }
    }

    #[test]
    fn input_and_select_defaults_use_theme_tokens() {
        let mut theme = Theme::dark();
        theme.density = Density::Comfortable;
        theme.colors.surface = Color::hexa(0x102030FF);
        theme.colors.primary = Color::hexa(0x44DD99FF);
        theme.colors.selection = Color::hexa(0x44DD9966);
        theme.radius.lg = dp(10.0);

        let input = InputStyle::default_for_theme(&theme);
        assert_eq!(input.background.normal.resolve(), theme.colors.surface);
        assert_eq!(input.border.normal.resolve(), theme.colors.outline_muted);
        assert_eq!(input.border.hovered.resolve(), theme.colors.outline);
        assert_eq!(
            input
                .border
                .resolve(WidgetState {
                    focused: true,
                    ..Default::default()
                })
                .resolve(),
            theme.colors.primary
        );
        assert_eq!(
            input.caret.as_ref().map(Value::resolve),
            Some(theme.colors.primary)
        );
        assert_eq!(
            input.selection.as_ref().map(Value::resolve),
            Some(theme.colors.selection)
        );
        assert_eq!(input.radius.resolve(), theme.radius.lg);
        assert_eq!(input.min_height, dp(40.0));

        let select = SelectStyle::default_for_theme(&theme);
        assert_eq!(select.background.normal.resolve(), theme.colors.surface);
        for state in [
            WidgetState {
                focused: true,
                ..Default::default()
            },
            WidgetState {
                open: true,
                ..Default::default()
            },
        ] {
            assert_eq!(select.border.resolve(state).resolve(), theme.colors.primary);
        }
        assert_eq!(
            select.selected_option_background.resolve(),
            theme.colors.primary_container
        );
        assert_eq!(select.radius.resolve(), theme.radius.lg);
        assert_eq!(select.min_height, dp(40.0));
    }

    #[test]
    fn light_and_dark_control_geometry_matches_each_density() {
        let expected = [
            (
                Density::Compact,
                ControlDensityMetrics {
                    control_height: dp(32.0),
                    button_padding_x: dp(8.0),
                    button_padding_y: dp(2.0),
                    input_padding_x: dp(10.0),
                    input_padding_y: dp(4.0),
                    select_padding_x: dp(12.0),
                    selection_size: dp(16.0),
                    selection_gap: dp(6.0),
                    switch_padding: dp(3.0),
                    switch_width: dp(36.0),
                    switch_height: dp(20.0),
                    slider_track_height: dp(3.0),
                    slider_thumb_size: dp(16.0),
                    slider_tick_size: dp(3.0),
                    slider_label_gap: dp(6.0),
                    slider_min_width: dp(144.0),
                    slider_min_height: dp(28.0),
                },
            ),
            (
                Density::Comfortable,
                ControlDensityMetrics {
                    control_height: dp(40.0),
                    button_padding_x: dp(12.0),
                    button_padding_y: dp(4.0),
                    input_padding_x: dp(12.0),
                    input_padding_y: dp(8.0),
                    select_padding_x: dp(16.0),
                    selection_size: dp(18.0),
                    selection_gap: dp(8.0),
                    switch_padding: dp(4.0),
                    switch_width: dp(40.0),
                    switch_height: dp(24.0),
                    slider_track_height: dp(4.0),
                    slider_thumb_size: dp(20.0),
                    slider_tick_size: dp(4.0),
                    slider_label_gap: dp(8.0),
                    slider_min_width: dp(160.0),
                    slider_min_height: dp(32.0),
                },
            ),
            (
                Density::Spacious,
                ControlDensityMetrics {
                    control_height: dp(48.0),
                    button_padding_x: dp(16.0),
                    button_padding_y: dp(8.0),
                    input_padding_x: dp(16.0),
                    input_padding_y: dp(10.0),
                    select_padding_x: dp(20.0),
                    selection_size: dp(20.0),
                    selection_gap: dp(10.0),
                    switch_padding: dp(4.0),
                    switch_width: dp(48.0),
                    switch_height: dp(28.0),
                    slider_track_height: dp(5.0),
                    slider_thumb_size: dp(24.0),
                    slider_tick_size: dp(5.0),
                    slider_label_gap: dp(10.0),
                    slider_min_width: dp(184.0),
                    slider_min_height: dp(40.0),
                },
            ),
        ];

        for mut theme in [Theme::light(), Theme::dark()] {
            for &(density, metrics) in &expected {
                assert_eq!(control_density_metrics(&theme, density), metrics);
                theme.density = density;

                let button = ButtonStyle::default_for_theme(&theme, ButtonVariantKind::Primary);
                assert_eq!(button.min_height, metrics.control_height);
                assert_eq!(button.padding_x, metrics.button_padding_x);
                assert_eq!(button.padding_y, metrics.button_padding_y);

                let input = InputStyle::default_for_theme(&theme);
                assert_eq!(input.min_height, metrics.control_height);
                assert_eq!(input.padding_x, metrics.input_padding_x);
                assert_eq!(input.padding_y, metrics.input_padding_y);

                let select = SelectStyle::default_for_theme(&theme);
                assert_eq!(select.min_height, metrics.control_height);
                assert_eq!(select.option_height, metrics.control_height);
                assert_eq!(select.padding_x, metrics.select_padding_x);

                let checkbox = CheckboxStyle::default_for_theme(&theme);
                let radio = RadioStyle::default_for_theme(&theme);
                assert_eq!(checkbox.size, metrics.selection_size);
                assert_eq!(radio.size, metrics.selection_size);
                assert_eq!(checkbox.label_gap, metrics.selection_gap);
                assert_eq!(radio.label_gap, metrics.selection_gap);

                let switch = SwitchStyle::default_for_theme(&theme);
                assert_eq!(switch.padding, Insets::all(metrics.switch_padding));
                assert_eq!(switch.width, metrics.switch_width);
                assert_eq!(switch.height, metrics.switch_height);

                let slider = SliderStyle::default_for_theme(&theme);
                assert_eq!(slider.track_height, metrics.slider_track_height);
                assert_eq!(slider.thumb_size, metrics.slider_thumb_size);
                assert_eq!(slider.tick_size, metrics.slider_tick_size);
                assert_eq!(slider.label_gap, metrics.slider_label_gap);
                assert_eq!(slider.min_width, metrics.slider_min_width);
                assert_eq!(slider.min_height, metrics.slider_min_height);
            }
        }
    }

    #[test]
    fn theme_default_remains_dark_and_compact() {
        let theme = Theme::default();
        assert_eq!(theme, Theme::dark());
        assert_eq!(theme.density, Density::Compact);
    }

    #[test]
    fn menu_bar_defaults_follow_control_density_without_vertical_root_padding() {
        for mut theme in [Theme::light(), Theme::dark()] {
            for (density, expected_height, expected_gap) in [
                (Density::Compact, dp(32.0), theme.spacing.xxs),
                (Density::Comfortable, dp(40.0), theme.spacing.xs),
                (Density::Spacious, dp(48.0), theme.spacing.sm),
            ] {
                theme.density = density;
                let metrics = control_density_metrics(&theme, density);
                let style = MenuBarStyle::default_for_theme(&theme);

                assert_eq!(style.height, expected_height);
                assert_eq!(style.height, metrics.control_height);
                assert_eq!(style.entry_padding_x, metrics.button_padding_x);
                assert_eq!(style.padding.top, Dp::ZERO);
                assert_eq!(style.padding.bottom, Dp::ZERO);
                assert_eq!(style.entry_gap, expected_gap);
                assert!(
                    contrast_ratio(
                        style.entry_foreground.normal.resolve(),
                        style.background.resolve(),
                    ) >= 4.5
                );
            }
        }
    }

    #[test]
    fn modal_title_uses_the_theme_title_token_with_readable_line_height() {
        for mut theme in [Theme::light(), Theme::dark()] {
            for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
                theme.density = density;
                let style = ModalStyle::default_for_theme(&theme);
                assert_eq!(style.title_text_style, theme.typography.title);
                assert!(style
                    .title_text_style
                    .line_height
                    .is_some_and(|line_height| line_height > style.title_text_style.size));
            }
        }
    }

    #[test]
    fn toast_typography_separates_title_and_body_using_theme_tokens() {
        for mut theme in [Theme::light(), Theme::dark()] {
            for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
                theme.density = density;
                let style = ToastStyle::default_for_theme(&theme);
                assert_eq!(style.title_text_style, theme.typography.label);
                assert_eq!(style.title_text_style.weight, FontWeight::Medium);
                assert_eq!(style.body_text_style, theme.typography.body_small);
                assert_eq!(style.body_text_style.weight, FontWeight::Regular);
                assert!(style.body_text_style.line_height > style.title_text_style.line_height);
                assert!(
                    contrast_ratio(style.foreground.resolve(), style.background.resolve()) >= 4.5
                );
            }
        }
    }

    #[test]
    fn floating_surfaces_follow_theme_density() {
        let mut theme = Theme::light();
        let expected = [
            (Density::Compact, dp(28.0), dp(144.0), dp(200.0), dp(12.0)),
            (
                Density::Comfortable,
                dp(32.0),
                dp(160.0),
                dp(220.0),
                dp(16.0),
            ),
            (Density::Spacious, dp(40.0), dp(176.0), dp(240.0), dp(20.0)),
        ];

        for (density, row_height, menu_width, popover_width, popover_padding) in expected {
            theme.density = density;
            let menu = MenuStyle::default_for_theme(&theme);
            let popover = PopoverStyle::default_for_theme(&theme);
            assert_eq!(menu.item_min_height, row_height);
            assert_eq!(menu.min_width, menu_width);
            assert_eq!(popover.min_width, popover_width);
            assert_eq!(popover.padding, Insets::all(popover_padding));
            assert_eq!(popover.radius, menu.radius);
            assert_eq!(popover.background, menu.background);
            assert_eq!(popover.shadow, theme.elevation.md);
            assert_eq!(popover.shadow, menu.shadow);
        }
    }

    #[test]
    fn blocking_and_transient_layers_follow_theme_density() {
        let mut theme = Theme::light();
        let expected = [
            (Density::Compact, dp(272.0), dp(264.0), dp(280.0), dp(16.0)),
            (
                Density::Comfortable,
                dp(320.0),
                dp(288.0),
                dp(320.0),
                dp(18.0),
            ),
            (Density::Spacious, dp(360.0), dp(320.0), dp(360.0), dp(20.0)),
        ];

        for (density, modal_width, drawer_width, toast_width, toast_icon_size) in expected {
            theme.density = density;
            let modal = ModalStyle::default_for_theme(&theme);
            let drawer = DrawerStyle::default_for_theme(&theme);
            let toast = ToastStyle::default_for_theme(&theme);

            assert_eq!(modal.min_width, modal_width);
            assert_eq!(drawer.width, drawer_width);
            assert_eq!(drawer.radius.resolve(), theme.radius.none);
            assert_eq!(toast.max_width, toast_width);
            assert_eq!(toast.icon_size, toast_icon_size);
            assert_eq!(toast.background.resolve(), theme.colors.surface_overlay);
            assert_eq!(toast.foreground.resolve(), theme.colors.on_surface);
            assert_eq!(toast.border_width.resolve(), theme.border.thin);
        }
    }

    #[test]
    fn selection_controls_share_modern_geometry_and_interaction_states() {
        let mut theme = Theme::light();
        theme.density = Density::Comfortable;
        let palette = palette_from_theme(&theme);
        let hovered = WidgetState {
            hovered: true,
            ..WidgetState::default()
        };
        let pressed = WidgetState {
            pressed: true,
            ..WidgetState::default()
        };

        let checkbox = CheckboxStyle::default_for_theme(&theme);
        let radio = RadioStyle::default_for_theme(&theme);
        assert_eq!(checkbox.size, dp(18.0));
        assert_eq!(radio.size, dp(18.0));
        assert_eq!(checkbox.radius.resolve(), theme.radius.sm);
        assert_eq!(radio.radius.resolve(), theme.radius.full);
        assert_eq!(
            checkbox.background.resolve(hovered).resolve(),
            palette.surface_high
        );
        assert_eq!(
            radio.background.resolve(hovered).resolve(),
            Color::TRANSPARENT
        );
        assert_eq!(
            radio.background.resolve(pressed).resolve(),
            Color::TRANSPARENT
        );
        assert_eq!(
            checkbox.border.resolve(hovered).resolve(),
            radio.border.resolve(hovered).resolve()
        );

        let switch = SwitchStyle::default_for_theme(&theme);
        assert_ne!(
            switch.track.resolve(hovered).resolve(),
            switch.track.normal.resolve()
        );
        assert_ne!(
            switch.track.resolve(pressed).resolve(),
            switch.track.normal.resolve()
        );
    }
}
