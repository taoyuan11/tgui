use std::sync::{Arc, Mutex};

use crate::foundation::color::Color;
use crate::media::ContentFit;
use crate::theme::{FocusRingStyle, ResolvedThemeMode};
use crate::ui::layout::{ScrollbarStyle, Value};
use crate::ui::theme::{Shadow, TextStyle, Theme};
use crate::ui::unit::{dp, Dp};

use super::super::background::{BackgroundBrush, BackgroundImage};
use super::super::common::Point;
use super::palette::{body_text_style, palette};

/// 覆盖主题默认焦点环配置的样式。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusRingOverride {
    pub enabled: Option<bool>,
    pub color: Option<Color>,
    pub width: Option<Dp>,
    pub gap: Option<Dp>,
}

impl FocusRingOverride {
    /// 基于当前主题解析出最终焦点环样式。
    ///
    /// 参数：
    /// - `theme`：当前生效主题。
    ///
    /// 返回值：
    /// - 返回合并 override 之后的焦点环样式。
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

/// 所有承载类 widget 共享的表面样式。
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetSurfaceStyle {
    pub background: Option<Value<Color>>,
    pub background_brush: Option<Value<BackgroundBrush>>,
    pub background_image: Option<Value<BackgroundImage>>,
    pub background_blur: Value<Dp>,
    pub shadow: Option<Value<Shadow>>,
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
            shadow: None,
            border_color: None,
            border_radius: None,
            border_width: None,
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
        }
    }
}

/// 纯文本 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TextWidgetStyle {
    pub surface: WidgetSurfaceStyle,
    pub color: Value<Color>,
    pub typography: TextStyle,
}

impl TextWidgetStyle {
    /// 按解析后的主题模式创建默认文本样式。
    pub fn default_for(mode: ResolvedThemeMode) -> Self {
        let palette = palette(mode);
        Self {
            surface: WidgetSurfaceStyle::default(),
            color: Value::Static(palette.text_primary),
            typography: body_text_style(),
        }
    }
}

/// 容器类 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerStyle {
    pub surface: WidgetSurfaceStyle,
    pub scrollbar: ScrollbarStyle,
}

impl ContainerStyle {
    /// 按解析后的主题模式创建默认容器样式。
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

/// 图片 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct ImageStyle {
    pub surface: WidgetSurfaceStyle,
    pub fit: ContentFit,
}

impl ImageStyle {
    /// 按解析后的主题模式创建默认图片样式。
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            fit: ContentFit::Contain,
        }
    }
}

/// 画布 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasStyle {
    pub surface: WidgetSurfaceStyle,
}

impl CanvasStyle {
    /// 按解析后的主题模式创建默认画布样式。
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
        }
    }
}

/// 视频表面的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct VideoSurfaceStyle {
    pub surface: WidgetSurfaceStyle,
    pub fit: ContentFit,
}

impl VideoSurfaceStyle {
    /// 按解析后的主题模式创建默认视频样式。
    pub fn default_for(_: ResolvedThemeMode) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            fit: ContentFit::Contain,
        }
    }
}
