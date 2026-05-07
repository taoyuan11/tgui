use std::sync::{Arc, Mutex};

use crate::foundation::color::Color;
use crate::media::ContentFit;
use crate::theme::FocusRingStyle;
use crate::ui::layout::{Insets, ScrollbarStyle, Value};
use crate::ui::theme::{FontWeight, Stateful, TextStyle, Theme};
use crate::ui::unit::{dp, sp, Dp};

use super::background::{BackgroundBrush, BackgroundImage};
use super::common::{ButtonVariantKind, Point};
use crate::theme::ResolvedThemeMode;

const HOVER_LIGHTEN: f32 = 0.1;
const SURFACE_HOVER_LIGHTEN: f32 = 0.06;
const BORDER_HOVER_LIGHTEN: f32 = 0.12;
const SCROLLBAR_HOVER_LIGHTEN: f32 = 0.18;

#[derive(Clone, Debug, Default)]
pub struct FocusRingOverride {
    pub enabled: Option<bool>,
    pub color: Option<Color>,
    pub width: Option<Dp>,
    pub gap: Option<Dp>,
}

impl FocusRingOverride {
    pub fn resolve(&self, theme: &Theme) -> FocusRingStyle {
        FocusRingStyle {
            enabled: self.enabled.unwrap_or(theme.focus_ring.enabled),
            color: self.color.unwrap_or(theme.focus_ring.color),
            width: self.width.unwrap_or(theme.focus_ring.width),
            gap: self.gap.unwrap_or(theme.focus_ring.gap),
        }
    }
}

#[derive(Clone)]
pub(crate) struct StyleResolver<T> {
    resolver: Arc<dyn Fn(ResolvedThemeMode) -> T + Send + Sync>,
    cache: Arc<Mutex<Option<(ResolvedThemeMode, T)>>>,
}

impl<T: Clone> StyleResolver<T> {
    pub(crate) fn new(resolver: impl Fn(ResolvedThemeMode) -> T + Send + Sync + 'static) -> Self {
        Self {
            resolver: Arc::new(resolver),
            cache: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn resolve(&self, mode: ResolvedThemeMode) -> T {
        let mut cache = self
            .cache
            .lock()
            .expect("style resolver cache lock should not be poisoned");
        if let Some((cached_mode, cached_value)) = cache.as_ref() {
            if *cached_mode == mode {
                return cached_value.clone();
            }
        }
        let value = (self.resolver)(mode);
        *cache = Some((mode, value.clone()));
        value
    }
}

pub(crate) fn infer_theme_mode(theme: &Theme) -> ResolvedThemeMode {
    let luminance = (0.299 * theme.colors.background.r as f32)
        + (0.587 * theme.colors.background.g as f32)
        + (0.114 * theme.colors.background.b as f32);
    if luminance >= 140.0 {
        ResolvedThemeMode::Light
    } else {
        ResolvedThemeMode::Dark
    }
}

#[derive(Clone, Debug)]
pub struct WidgetSurfaceStyle {
    pub background: Option<Value<Color>>,
    pub background_brush: Option<Value<BackgroundBrush>>,
    pub background_image: Option<Value<BackgroundImage>>,
    pub background_blur: Value<Dp>,
    pub border_color: Option<Value<Color>>,
    pub border_radius: Option<Value<Dp>>,
    pub border_width: Option<Value<Dp>>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for WidgetSurfaceStyle {
    fn default() -> Self {
        Self {
            background: None,
            background_brush: None,
            background_image: None,
            background_blur: Value::Static(Dp::ZERO),
            border_color: None,
            border_radius: None,
            border_width: None,
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextWidgetStyle {
    pub surface: WidgetSurfaceStyle,
    pub color: Value<Color>,
    pub typography: TextStyle,
}

impl TextWidgetStyle {
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            color: Value::Static(palette.text_primary),
            typography: body_text_style(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContainerStyle {
    pub surface: WidgetSurfaceStyle,
    pub scrollbar: ScrollbarStyle,
}

impl ContainerStyle {
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            scrollbar: ScrollbarStyle {
                thumb_color: Some(palette.scrollbar_thumb.normal),
                hover_thumb_color: Some(palette.scrollbar_thumb.hovered),
                active_thumb_color: Some(palette.scrollbar_thumb.pressed),
                track_color: Some(palette.scrollbar_track),
                thickness: Some(dp(5.0)),
                radius: Some(dp(999.0)),
                insets: None,
                min_thumb_length: Some(dp(12.0)),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImageStyle {
    pub surface: WidgetSurfaceStyle,
    pub fit: ContentFit,
}

impl ImageStyle {
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            fit: ContentFit::Contain,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CanvasStyle {
    pub surface: WidgetSurfaceStyle,
}

impl CanvasStyle {
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VideoSurfaceStyle {
    pub surface: WidgetSurfaceStyle,
    pub fit: ContentFit,
}

impl VideoSurfaceStyle {
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            fit: ContentFit::Contain,
        }
    }
}

#[derive(Clone, Debug)]
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
    pub fn default_for(mode: ResolvedThemeMode, variant: ButtonVariantKind) -> Self {
        let palette = palette(mode);
        let (background, foreground, border, border_width) = match variant {
            ButtonVariantKind::Primary => (
                stateful_colors(
                    palette.primary,
                    palette.primary.lighten(HOVER_LIGHTEN),
                    palette.primary.darken(HOVER_LIGHTEN),
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
                    palette.primary.lighten(HOVER_LIGHTEN),
                    palette.primary.darken(HOVER_LIGHTEN),
                    palette.disabled_surface,
                ),
                dp(0.0),
            ),
            ButtonVariantKind::Secondary => (
                stateful_colors(
                    palette.surface,
                    palette.surface.lighten(SURFACE_HOVER_LIGHTEN),
                    palette.surface.darken(SURFACE_HOVER_LIGHTEN),
                    palette.disabled_surface,
                ),
                stateful_single(
                    palette.on_surface,
                    palette.primary.lighten(HOVER_LIGHTEN),
                    palette.primary.darken(HOVER_LIGHTEN),
                    palette.disabled_content,
                ),
                stateful_colors(
                    palette.outline,
                    palette.primary.lighten(HOVER_LIGHTEN),
                    palette.primary.darken(HOVER_LIGHTEN),
                    palette.disabled_surface,
                ),
                dp(1.0),
            ),
            ButtonVariantKind::Ghost => (
                stateful_colors(
                    Color::TRANSPARENT,
                    palette.surface_high.lighten(SURFACE_HOVER_LIGHTEN),
                    palette.surface_high.darken(SURFACE_HOVER_LIGHTEN),
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
                    palette.error.lighten(HOVER_LIGHTEN),
                    palette.error.darken(HOVER_LIGHTEN),
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
                    palette.error.lighten(HOVER_LIGHTEN),
                    palette.error.darken(HOVER_LIGHTEN),
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_low.lighten(SURFACE_HOVER_LIGHTEN),
                palette.surface_low.darken(SURFACE_HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            background_checked: stateful_colors(
                palette.surface_low,
                palette.surface_low.lighten(SURFACE_HOVER_LIGHTEN),
                palette.surface_low.darken(SURFACE_HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            border: stateful_colors(
                palette.outline,
                palette.outline.lighten(BORDER_HOVER_LIGHTEN),
                palette.outline.darken(BORDER_HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            border_checked: stateful_colors(
                palette.primary,
                palette.primary.lighten(HOVER_LIGHTEN),
                palette.primary.darken(HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            focus_ring: None,
            indicator: stateful_colors(
                palette.primary,
                palette.primary.lighten(HOVER_LIGHTEN),
                palette.primary.darken(HOVER_LIGHTEN),
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

#[derive(Clone, Debug)]
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
                palette.outline_muted.lighten(BORDER_HOVER_LIGHTEN),
                palette.outline_muted.darken(BORDER_HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            border_checked: stateful_colors(
                palette.primary,
                palette.primary.lighten(HOVER_LIGHTEN),
                palette.primary.darken(HOVER_LIGHTEN),
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

#[derive(Clone, Debug)]
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
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            background: stateful_colors(
                palette.surface_low,
                palette.surface_low.lighten(SURFACE_HOVER_LIGHTEN),
                palette.surface_low.darken(SURFACE_HOVER_LIGHTEN),
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
                palette.outline.lighten(BORDER_HOVER_LIGHTEN),
                palette.outline.darken(BORDER_HOVER_LIGHTEN),
                palette.disabled_surface,
            ),
            focus_ring: None,
            arrow: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            menu_background: Value::Static(palette.surface),
            option_background: stateful_colors(
                Color::TRANSPARENT,
                palette.surface_high.lighten(SURFACE_HOVER_LIGHTEN),
                palette.surface_high.darken(SURFACE_HOVER_LIGHTEN),
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

#[derive(Clone)]
struct Palette {
    /// 主题主色，用于 Primary 按钮、选中态控件、强调性操作等核心视觉焦点。
    primary: Color,
    /// 放在主色背景上的前景色，保证主按钮文字、图标、勾选标记与主色背景之间有足够对比度。
    on_primary: Color,
    /// 危险语义色，用于删除、报错、破坏性确认等需要明确风险提示的场景。
    error: Color,
    /// 放在危险语义色背景上的前景色，用于 Danger 按钮文字或图标的可读性。
    on_error: Color,
    /// 默认表面色，作为常规容器、菜单面板和次级按钮底色的基础层。
    surface: Color,
    /// 比默认表面更弱一层的表面色，常用于输入框、幽灵按钮 hover 底色、未选中控件背景等浅层承载面。
    surface_low: Color,
    /// 比默认表面更强调一层的表面色，常用于下拉选项 hover/selected 等需要轻度突出但不抢主色的区域。
    surface_high: Color,
    /// 放在普通表面上的主要前景色，通常用于正文、标签、箭头和普通控件内容。
    on_surface: Color,
    /// 放在普通表面上的弱化前景色，通常用于 placeholder、次要说明和非激活图标。
    on_surface_muted: Color,
    /// 标准描边色，用于输入框、次级按钮、单选框等常规边框。
    outline: Color,
    /// 更弱的描边色，主要用于像 Switch 这种需要更轻边界感的控件边框。
    outline_muted: Color,
    /// 禁用态表面色，控件不可交互时的背景或边框降级颜色。
    disabled_surface: Color,
    /// 禁用态内容色，用于禁用控件中的文字、图标、勾选标记等前景元素。
    disabled_content: Color,
    /// 文本组件默认主文字色，给纯文本类 widget 作为基础前景色使用。
    text_primary: Color,
    /// 滚动条轨道颜色，用于容器滚动区域的底轨视觉。
    scrollbar_track: Color,
    /// 滚动条滑块在 normal/hover/pressed/disabled 各状态下的颜色集合。
    scrollbar_thumb: Stateful<Color>,
    /// Switch 未选中时轨道的基础颜色。
    switch_track: Color,
}

fn palette(mode: ResolvedThemeMode) -> Palette {
    match mode {
        ResolvedThemeMode::Light => Palette {
            primary: Color::hexa(0x2563EBFF),
            on_primary: Color::WHITE,
            error: Color::hexa(0xDC2626FF),
            on_error: Color::WHITE,
            surface: Color::TRANSPARENT,
            surface_low: Color::hexa(0xFAFAFAFF),
            surface_high: Color::hexa(0xF0F0F0FF),
            on_surface: Color::hexa(0x262626FF),
            on_surface_muted: Color::hexa(0x737373FF),
            outline: Color::hexa(0xD9D9D9FF),
            outline_muted: Color::hexa(0xEDEDEDFF),
            disabled_surface: Color::hexa(0xF0F0F0FF),
            disabled_content: Color::hexa(0xBFBFBFFF),
            text_primary: Color::BLACK,
            scrollbar_track: Color::hexa(0xF5F5F5FF),
            scrollbar_thumb: stateful_colors(
                Color::hexa(0xBFBFBFB8),
                Color::hexa(0xBFBFBFB8).lighten(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0xBFBFBFB8).darken(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0xF0F0F0FF),
            )
            .map(|value| value.resolve()),
            switch_track: Color::hexa(0xBFBFBFFF),
        },
        ResolvedThemeMode::Dark => Palette {
            primary: Color::hexa(0x3B82F6FF),
            on_primary: Color::WHITE,
            error: Color::hexa(0xEF4444FF),
            on_error: Color::WHITE,
            surface: Color::TRANSPARENT,
            surface_low: Color::hexa(0x262626FF),
            surface_high: Color::hexa(0x303030FF),
            on_surface: Color::hexa(0xF5F5F5FF),
            on_surface_muted: Color::hexa(0xA6A6A6FF),
            outline: Color::hexa(0x424242FF),
            outline_muted: Color::hexa(0x595959FF),
            disabled_surface: Color::hexa(0x303030FF),
            disabled_content: Color::hexa(0x8C8C8CFF),
            text_primary: Color::WHITE,
            scrollbar_track: Color::hexa(0x1F1F1FFF),
            scrollbar_thumb: stateful_colors(
                Color::hexa(0x595959B8),
                Color::hexa(0x595959B8).lighten(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0x595959B8).darken(SCROLLBAR_HOVER_LIGHTEN),
                Color::hexa(0x303030FF),
            )
            .map(|value| value.resolve()),
            switch_track: Color::hexa(0x595959FF),
        },
    }
}

fn body_text_style() -> TextStyle {
    TextStyle {
        font_family: None,
        size: sp(16.0),
        line_height: Some(sp(22.0)),
        weight: FontWeight::Regular,
        letter_spacing: Some(sp(0.0)),
    }
}

fn label_text_style() -> TextStyle {
    TextStyle {
        font_family: None,
        size: sp(14.0),
        line_height: Some(sp(18.0)),
        weight: FontWeight::Medium,
        letter_spacing: Some(sp(0.0)),
    }
}

fn stateful_single(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> Stateful<Value<Color>> {
    stateful_colors(normal, hovered, pressed, disabled)
}

fn stateful_colors(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> Stateful<Value<Color>> {
    Stateful {
        normal: Value::Static(normal),
        hovered: Value::Static(hovered),
        pressed: Value::Static(pressed),
        disabled: Value::Static(disabled),
    }
}

trait MapStateful<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> Stateful<U>;
}

impl<T> MapStateful<T> for Stateful<T> {
    fn map<U>(self, mapper: impl Fn(T) -> U) -> Stateful<U> {
        Stateful {
            normal: mapper(self.normal),
            hovered: mapper(self.hovered),
            pressed: mapper(self.pressed),
            disabled: mapper(self.disabled),
        }
    }
}
