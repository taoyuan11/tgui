use std::sync::Arc;

use crate::foundation::color::Color;
use crate::media::ContentFit;
use crate::theme::{FocusRingStyle, StyleContext, WidgetState};
use crate::ui::layout::{ScrollbarStyle, Value};
use crate::ui::theme::{Shadow, TextStyle, Theme};
use crate::ui::unit::{dp, Dp};

use super::super::background::{BackgroundBrush, BackgroundImage};
use super::super::common::{Point, VisualStyle};
use super::palette::palette_from_theme;
use super::sheet::StyleSheet;

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
    kind: StyleResolverKind<T>,
}

#[derive(Clone)]
enum StyleResolverKind<T> {
    Full(Arc<dyn Fn(&StyleContext<'_>) -> T + Send + Sync>),
    FullWithStyleSheet(
        Arc<dyn Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> T + Send + Sync>,
    ),
    Mutator(Arc<dyn Fn(&mut T, &StyleContext<'_>) + Send + Sync>),
}

impl<T: Clone> StyleResolver<T> {
    pub(crate) fn full(resolver: impl Fn(&StyleContext<'_>) -> T + Send + Sync + 'static) -> Self {
        Self {
            kind: StyleResolverKind::Full(Arc::new(resolver)),
        }
    }

    pub(crate) fn full_with_style_sheet(
        resolver: impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> T
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self {
            kind: StyleResolverKind::FullWithStyleSheet(Arc::new(resolver)),
        }
    }

    pub(crate) fn mutate(
        _default: impl Fn(&StyleContext<'_>) -> T + Send + Sync + 'static,
        mutator: impl Fn(&mut T, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind: StyleResolverKind::Mutator(Arc::new(mutator)),
        }
    }

    pub(crate) fn resolve_from(&self, mut base: T, context: &StyleContext<'_>) -> T {
        match &self.kind {
            StyleResolverKind::Full(resolver) => resolver(context),
            StyleResolverKind::FullWithStyleSheet(resolver) => {
                let style_sheet = StyleSheet::default();
                resolver(
                    context,
                    &style_sheet,
                    &VisualStyle::default(),
                    WidgetState::default(),
                )
            }
            StyleResolverKind::Mutator(mutator) => {
                mutator(&mut base, context);
                base
            }
        }
    }

    pub(crate) fn resolve_with(
        &self,
        base: T,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
        visual: &VisualStyle,
    ) -> T {
        self.resolve_with_state(base, context, style_sheet, visual, WidgetState::default())
    }

    pub(crate) fn resolve_with_state(
        &self,
        mut base: T,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
        visual: &VisualStyle,
        state: WidgetState,
    ) -> T {
        match &self.kind {
            StyleResolverKind::Full(resolver) => resolver(context),
            StyleResolverKind::FullWithStyleSheet(resolver) => {
                resolver(context, style_sheet, visual, state)
            }
            StyleResolverKind::Mutator(mutator) => {
                mutator(&mut base, context);
                base
            }
        }
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

pub(crate) fn merge_surface_style(
    background: &mut Option<Value<Color>>,
    visual: &mut VisualStyle,
    surface: &WidgetSurfaceStyle,
) {
    if background.is_none() {
        *background = surface.background.clone();
    }
    if visual.background_brush.is_none() {
        visual.background_brush = surface.background_brush.clone();
    }
    if visual.background_image.is_none() {
        visual.background_image = surface.background_image.clone();
    }
    if matches!(&visual.background_blur, Value::Static(value) if *value == Dp::ZERO) {
        visual.background_blur = surface.background_blur.clone();
    }
    if visual.shadow.is_none() {
        visual.shadow = surface.shadow.clone();
    }
    if visual.border_color.is_none() {
        visual.border_color = surface.border_color.clone();
    }
    if visual.border_radius.is_none() {
        visual.border_radius = surface.border_radius.clone();
    }
    if visual.border_width.is_none() {
        visual.border_width = surface.border_width.clone();
    }
    if matches!(&visual.opacity, Value::Static(value) if (*value - 1.0).abs() <= f32::EPSILON) {
        visual.opacity = surface.opacity.clone();
    }
    if matches!(&visual.offset, Value::Static(value) if *value == Point::ZERO) {
        visual.offset = surface.offset.clone();
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            color: Value::Static(palette.text_primary),
            typography: theme.typography.body.clone(),
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            scrollbar: ScrollbarStyle {
                thumb_color: Some(palette.scrollbar_thumb.normal),
                hover_thumb_color: Some(palette.scrollbar_thumb.hovered),
                active_thumb_color: Some(palette.scrollbar_thumb.pressed),
                track_color: Some(palette.scrollbar_track),
                thickness: Some(dp(5.0)),
                radius: Some(theme.radius.full),
                insets: None,
                min_thumb_length: Some(theme.spacing.sm + theme.spacing.xs),
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
    pub fn default_for_theme(_: &Theme) -> Self {
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
    pub fn default_for_theme(_: &Theme) -> Self {
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
    pub fn default_for_theme(_: &Theme) -> Self {
        Self {
            surface: WidgetSurfaceStyle::default(),
            fit: ContentFit::Contain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::unit::{dp, sp};

    #[test]
    fn text_default_for_theme_uses_typography_and_surface_colors() {
        let mut theme = Theme::light();
        theme.colors.on_surface = Color::hexa(0x123456FF);
        theme.typography.body.size = sp(18.0);

        let style = TextWidgetStyle::default_for_theme(&theme);

        assert_eq!(style.color.resolve(), theme.colors.on_surface);
        assert_eq!(style.typography.size, theme.typography.body.size);
    }

    #[test]
    fn container_default_for_theme_uses_scrollbar_tokens() {
        let mut theme = Theme::dark();
        theme.colors.surface_low = Color::hexa(0x223344FF);
        theme.colors.outline = Color::hexa(0x778899FF);
        theme.radius.full = dp(99.0);

        let style = ContainerStyle::default_for_theme(&theme);

        assert_eq!(
            style.scrollbar.track_color,
            Some(theme.colors.surface_low.with_alpha_factor(0.72))
        );
        assert_eq!(
            style.scrollbar.thumb_color,
            Some(theme.colors.outline.with_alpha_factor(0.64))
        );
        assert_eq!(style.scrollbar.radius, Some(theme.radius.full));
    }
}
