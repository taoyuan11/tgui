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

use crate::foundation::color::Color;
use crate::theme::{FontWeight, ResolvedThemeMode};
use crate::ui::layout::{Insets, Value};
use crate::ui::theme::{Shadow, StateValue, TextStyle, Theme};
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
        let palette = palette_from_theme(theme);
        Self::from_palette(
            palette,
            variant,
            theme.radius.md,
            theme.spacing.sm,
            theme.spacing.xs,
            dp(32.0),
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
        let palette = palette_from_theme(theme);
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
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.md),
            size: theme.spacing.md,
            label_gap: theme.spacing.sm,
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
        let palette = palette_from_theme(theme);
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
            size: theme.spacing.md,
            label_gap: theme.spacing.sm,
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
        let palette = palette_from_theme(theme);
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
            padding: Insets::all(theme.spacing.xs),
            width: dp(42.0),
            height: dp(24.0),
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
        let palette = palette_from_theme(theme);
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
            selected_option_background: Value::Static(theme.colors.primary_container),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            padding_x: theme.spacing.md,
            padding_y: Dp::ZERO,
            min_height: dp(40.0),
            option_height: dp(40.0),
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
            shadow: theme.elevation.sm.clone(),
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
        let menu = MenuStyle::default_for_theme(theme);
        Self {
            surface: menu.surface,
            background: menu.background,
            border: menu.border,
            border_width: menu.border_width,
            radius: menu.radius,
            shadow: menu.shadow,
            padding: Insets::all(theme.spacing.md),
            min_width: dp(220.0),
            max_width: dp(420.0),
            offset: theme.spacing.sm,
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
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface),
            border: Value::Static(theme.colors.outline),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.md),
            shadow: theme.elevation.md.clone(),
            min_width: dp(160.0),
            max_width: dp(360.0),
            padding: Insets::symmetric(Dp::ZERO, theme.spacing.xs),
            item_padding: Insets::symmetric(theme.spacing.md, theme.spacing.xs + theme.spacing.xxs),
            item_min_height: theme.spacing.xl,
            item_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            item_foreground: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            item_icon_size: theme.spacing.md,
            item_icon_gap: theme.spacing.sm,
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
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_low),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.none),
            radius: Value::Static(theme.radius.none),
            padding: Insets::symmetric(theme.spacing.xs, theme.spacing.xxs),
            height: theme.spacing.xl,
            entry_padding_x: theme.spacing.sm + theme.spacing.xxs,
            entry_min_width: theme.spacing.xxl + theme.spacing.sm,
            entry_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            entry_foreground: stateful_single(
                palette.on_surface,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            entry_active_background: Value::Static(theme.colors.surface_high),
            entry_gap: theme.spacing.xxs,
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
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            tab_bar_background: Value::Static(theme.colors.surface_low),
            panel_background: Value::Static(theme.colors.surface),
            tab_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            active_tab_background: Value::Static(theme.colors.surface_high),
            tab_foreground: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            active_tab_foreground: Value::Static(theme.colors.primary),
            indicator_color: Value::Static(theme.colors.primary),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            tab_padding: Insets::symmetric(theme.spacing.md, theme.spacing.xs + theme.spacing.xxs),
            tab_min_height: theme.spacing.xl + theme.spacing.xs,
            tab_min_width: dp(72.0),
            tab_gap: theme.spacing.xs,
            panel_padding: Insets::all(theme.spacing.md),
            indicator_thickness: theme.border.normal,
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
        Self {
            backdrop_color: Value::Static(Color::rgba(0, 0, 0, 0x80)),
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            shadow: theme.elevation.lg.clone(),
            min_width: dp(280.0),
            max_width: dp(560.0),
            max_height: dp(640.0),
            margin: Insets::all(theme.spacing.lg),
            padding: Insets::all(Dp::ZERO),
            title_text_style: {
                let mut style = theme.typography.label.clone();
                style.size = crate::ui::unit::sp(18.0);
                style
            },
            title_padding: Insets::symmetric(theme.spacing.lg, theme.spacing.md),
            content_padding: Insets::symmetric(theme.spacing.lg, theme.spacing.sm),
            actions_gap: theme.spacing.sm,
            actions_padding: Insets::symmetric(theme.spacing.lg, theme.spacing.md),
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
        let palette = palette_from_theme(theme);
        let mut action_button = ButtonStyle::default_for_theme(
            theme,
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );
        action_button.min_height = theme.spacing.lg;
        action_button.padding_x = theme.spacing.xs;
        action_button.padding_y = theme.spacing.xxs;
        action_button.radius = Value::Static(theme.radius.sm);

        let mut close_button = ButtonStyle::default_for_theme(
            theme,
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );
        close_button.min_height = theme.spacing.md + theme.spacing.xs;
        close_button.padding_x = theme.spacing.xxs;
        close_button.padding_y = theme.spacing.xxs;
        close_button.radius = Value::Static(theme.radius.full);

        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface_high),
            foreground: Value::Static(theme.colors.on_surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.md),
            shadow: theme.elevation.sm.clone(),
            padding: Insets::symmetric(theme.spacing.md, theme.spacing.sm),
            gap: theme.spacing.sm,
            title_text_style: {
                let mut style = theme.typography.label.clone();
                style.weight = FontWeight::Medium;
                style
            },
            body_text_style: theme.typography.label.clone(),
            icon_size: theme.spacing.md,
            min_width: dp(200.0),
            max_width: dp(280.0),
            margin: theme.spacing.md,
            stack_gap: theme.spacing.sm,
            action_button,
            close_button,
            success_icon_background: Value::Static(Color::hexa(0x10B981FF)),
            success_icon_foreground: Value::Static(Color::WHITE),
            error_icon_background: Value::Static(theme.colors.error),
            error_icon_foreground: Value::Static(theme.colors.on_error),
            warning_icon_background: Value::Static(Color::hexa(0xF59E0BFF)),
            warning_icon_foreground: Value::Static(Color::WHITE),
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
        Self {
            backdrop_color: Value::Static(Color::rgba(0, 0, 0, 0x80)),
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(theme.colors.surface),
            border: Value::Static(theme.colors.outline_muted),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.none),
            shadow: theme.elevation.md.clone(),
            width: dp(280.0),
            height: dp(240.0),
            padding: Insets::all(theme.spacing.lg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::unit::sp;

    #[test]
    fn button_default_for_theme_uses_theme_tokens() {
        let mut theme = Theme::light();
        theme.colors.primary = Color::hexa(0xCC3366FF);
        theme.radius.md = dp(6.0);
        theme.typography.label.size = sp(13.0);

        let style = ButtonStyle::default_for_theme(&theme, ButtonVariantKind::Primary);

        assert_eq!(style.background.normal.resolve(), theme.colors.primary);
        assert_eq!(style.border.normal.resolve(), theme.colors.primary);
        assert_eq!(style.radius.resolve(), theme.radius.md);
        assert_eq!(style.text_style.size, theme.typography.label.size);
    }

    #[test]
    fn input_and_select_defaults_use_theme_tokens() {
        let mut theme = Theme::dark();
        theme.colors.surface_low = Color::hexa(0x102030FF);
        theme.colors.primary = Color::hexa(0x44DD99FF);
        theme.colors.selection = Color::hexa(0x44DD9966);
        theme.radius.lg = dp(10.0);

        let input = InputStyle::default_for_theme(&theme);
        assert_eq!(input.background.normal.resolve(), theme.colors.surface_low);
        assert_eq!(
            input.caret.as_ref().map(Value::resolve),
            Some(theme.colors.primary)
        );
        assert_eq!(
            input.selection.as_ref().map(Value::resolve),
            Some(theme.colors.selection)
        );
        assert_eq!(input.radius.resolve(), theme.radius.lg);

        let select = SelectStyle::default_for_theme(&theme);
        assert_eq!(select.background.normal.resolve(), theme.colors.surface_low);
        assert_eq!(
            select.selected_option_background.resolve(),
            theme.colors.primary_container
        );
        assert_eq!(select.radius.resolve(), theme.radius.lg);
    }
}
