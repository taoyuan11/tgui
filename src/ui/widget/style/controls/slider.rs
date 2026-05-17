use super::*;

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
