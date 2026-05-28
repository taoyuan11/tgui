mod input;
mod progress;
mod slider;
mod spinner;

use crate::foundation::color::Color;
use crate::theme::FontWeight;
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

pub use self::input::{InputStyle, TextareaStyle};
pub use self::progress::ProgressBarStyle;
pub use self::slider::SliderStyle;
pub use self::spinner::SpinnerStyle;

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
    /// 按解析后的主题模式创建默认 tooltip 样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        // Tooltip 习惯上是"反色"——浅色主题用深色背景，深色主题用浅色背景。
        let (background, foreground) = match mode {
            ResolvedThemeMode::Light => (palette.on_surface, palette.surface),
            ResolvedThemeMode::Dark => (palette.surface_high, palette.on_surface),
        };
        Self {
            background,
            foreground,
            border: Color::TRANSPARENT,
            border_width: dp(0.0),
            radius: dp(6.0),
            padding: Insets::symmetric(dp(8.0), dp(4.0)),
            max_width: dp(240.0),
            offset: dp(8.0),
            pointer_size: dp(8.0),
            pointer_inset: dp(16.0),
            shadow: Shadow {
                offset_x: dp(0.0),
                offset_y: dp(6.0),
                blur: dp(16.0),
                spread: dp(0.0),
                color: foreground.with_alpha_factor(0.18),
            },
            text_style: label_text_style(),
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
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let menu = MenuStyle::default_for(mode);
        Self {
            surface: menu.surface,
            background: menu.background,
            border: menu.border,
            border_width: menu.border_width,
            radius: menu.radius,
            shadow: menu.shadow,
            padding: Insets::all(dp(12.0)),
            min_width: dp(220.0),
            max_width: dp(420.0),
            offset: dp(8.0),
            pointer_size: None,
            pointer_inset: dp(20.0),
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
    pub item_background: Stateful<Value<Color>>,
    pub item_foreground: Stateful<Value<Color>>,
    pub item_icon_size: Dp,
    pub item_icon_gap: Dp,
    pub shortcut_color: Value<Color>,
    pub shortcut_gap: Dp,
    pub checked_indicator_color: Value<Color>,
    pub submenu_arrow_size: Dp,
    pub submenu_arrow_color: Stateful<Value<Color>>,
    pub separator_color: Value<Color>,
    pub separator_height: Dp,
    pub separator_inset_x: Dp,
    pub text_style: TextStyle,
}

impl MenuStyle {
    /// 按解析后的主题模式创建默认菜单样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(palette.surface),
            border: Value::Static(palette.outline),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(8.0)),
            shadow: Shadow {
                offset_x: dp(0.0),
                offset_y: dp(8.0),
                blur: dp(24.0),
                spread: dp(0.0),
                color: Color::BLACK.with_alpha_factor(0.18),
            },
            min_width: dp(160.0),
            max_width: dp(360.0),
            padding: Insets::symmetric(dp(0.0), dp(4.0)),
            item_padding: Insets::symmetric(dp(12.0), dp(6.0)),
            item_min_height: dp(32.0),
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
            item_icon_size: dp(16.0),
            item_icon_gap: dp(10.0),
            shortcut_color: Value::Static(palette.on_surface_muted),
            shortcut_gap: dp(16.0),
            checked_indicator_color: Value::Static(palette.primary),
            submenu_arrow_size: dp(8.0),
            submenu_arrow_color: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            separator_color: Value::Static(palette.outline_muted),
            separator_height: dp(1.0),
            separator_inset_x: dp(8.0),
            text_style: label_text_style(),
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
    pub entry_background: Stateful<Value<Color>>,
    pub entry_foreground: Stateful<Value<Color>>,
    /// 条目高亮（active=该 entry 展开了下拉时）的强调色。
    pub entry_active_background: Value<Color>,
    pub entry_gap: Dp,
    pub text_style: TextStyle,
}

impl MenuBarStyle {
    /// 按解析后的主题模式创建默认 MenuBar 样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(palette.surface_low),
            border: Value::Static(palette.outline_muted),
            border_width: Value::Static(dp(0.0)),
            radius: Value::Static(dp(0.0)),
            padding: Insets::symmetric(dp(4.0), dp(2.0)),
            height: dp(32.0),
            entry_padding_x: dp(10.0),
            entry_min_width: dp(48.0),
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
            entry_active_background: Value::Static(palette.surface_high),
            entry_gap: dp(2.0),
            text_style: label_text_style(),
        }
    }
}

/// Tabs / TabView widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TabsStyle {
    pub surface: WidgetSurfaceStyle,
    pub tab_bar_background: Value<Color>,
    pub panel_background: Value<Color>,
    pub tab_background: Stateful<Value<Color>>,
    pub active_tab_background: Value<Color>,
    pub tab_foreground: Stateful<Value<Color>>,
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
    /// 按解析后的主题模式创建默认 Tabs 样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            tab_bar_background: Value::Static(palette.surface_low),
            panel_background: Value::Static(palette.surface),
            tab_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(surface_hover_lighten()),
                palette.surface_high.darken(surface_hover_lighten()),
                Color::TRANSPARENT,
            ),
            active_tab_background: Value::Static(palette.surface_high),
            tab_foreground: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            active_tab_foreground: Value::Static(palette.primary),
            indicator_color: Value::Static(palette.primary),
            border: Value::Static(palette.outline_muted),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(8.0)),
            tab_padding: Insets::symmetric(dp(12.0), dp(6.0)),
            tab_min_height: dp(36.0),
            tab_min_width: dp(72.0),
            tab_gap: dp(4.0),
            panel_padding: Insets::all(dp(12.0)),
            indicator_thickness: dp(3.0),
            text_style: label_text_style(),
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
}

impl ModalStyle {
    /// 按解析后的主题模式创建默认样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        // backdrop alpha 在 light/dark 下都用 ~50% 黑遮罩，足够把主界面"压暗"。
        let backdrop = Color::rgba(0, 0, 0, 0x80);
        Self {
            backdrop_color: Value::Static(backdrop),
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(palette.surface),
            border: Value::Static(palette.outline_muted),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(12.0)),
            shadow: Shadow {
                offset_x: dp(0.0),
                offset_y: dp(16.0),
                blur: dp(40.0),
                spread: dp(0.0),
                color: Color::BLACK.with_alpha_factor(0.32),
            },
            min_width: dp(280.0),
            max_width: dp(560.0),
            max_height: dp(640.0),
            margin: Insets::all(dp(24.0)),
            padding: Insets::all(dp(0.0)),
            title_text_style: {
                let mut style = label_text_style();
                style.size = crate::ui::unit::sp(18.0);
                style
            },
            title_padding: Insets::symmetric(dp(20.0), dp(16.0)),
            content_padding: Insets::symmetric(dp(20.0), dp(12.0)),
            actions_gap: dp(8.0),
            actions_padding: Insets::symmetric(dp(20.0), dp(16.0)),
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
    pub success_background: Value<Color>,
    pub success_foreground: Value<Color>,
    pub error_background: Value<Color>,
    pub error_foreground: Value<Color>,
    pub warning_background: Value<Color>,
    pub warning_foreground: Value<Color>,
    pub info_background: Value<Color>,
    pub info_foreground: Value<Color>,
}

impl ToastStyle {
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        let mut action_button =
            ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Ghost);
        action_button.min_height = dp(28.0);
        action_button.padding_x = dp(6.0);
        action_button.padding_y = dp(2.0);
        action_button.radius = Value::Static(dp(6.0));

        let mut close_button =
            ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Ghost);
        close_button.min_height = dp(24.0);
        close_button.padding_x = dp(4.0);
        close_button.padding_y = dp(2.0);
        close_button.radius = Value::Static(dp(999.0));

        Self {
            surface: WidgetSurfaceStyle::default(),
            background: Value::Static(palette.surface_high),
            foreground: Value::Static(palette.on_surface),
            border: Value::Static(palette.outline_muted),
            border_width: Value::Static(dp(1.0)),
            radius: Value::Static(dp(12.0)),
            shadow: Shadow {
                offset_x: dp(0.0),
                offset_y: dp(10.0),
                blur: dp(28.0),
                spread: dp(0.0),
                color: Color::BLACK.with_alpha_factor(0.2),
            },
            padding: Insets::all(dp(12.0)),
            gap: dp(10.0),
            title_text_style: {
                let mut style = label_text_style();
                style.weight = FontWeight::Medium;
                style
            },
            body_text_style: label_text_style(),
            icon_size: dp(18.0),
            min_width: dp(240.0),
            max_width: dp(420.0),
            margin: dp(16.0),
            stack_gap: dp(12.0),
            action_button,
            close_button,
            success_background: Value::Static(Color::hexa(0xDCFCE7FF)),
            success_foreground: Value::Static(Color::hexa(0x166534FF)),
            error_background: Value::Static(Color::hexa(0xFEE2E2FF)),
            error_foreground: Value::Static(Color::hexa(0x991B1BFF)),
            warning_background: Value::Static(Color::hexa(0xFEF3C7FF)),
            warning_foreground: Value::Static(Color::hexa(0x92400EFF)),
            info_background: Value::Static(Color::hexa(0xDBEAFEFF)),
            info_foreground: Value::Static(Color::hexa(0x1D4ED8FF)),
        }
    }
}
