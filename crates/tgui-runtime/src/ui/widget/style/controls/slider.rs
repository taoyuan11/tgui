use super::*;

/// 滑块 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct SliderStyle {
    pub surface: WidgetSurfaceStyle,
    pub track: StateValue<Value<Color>>,
    pub active_track: StateValue<Value<Color>>,
    pub thumb: StateValue<Value<Color>>,
    pub thumb_shadow: Option<Shadow>,
    pub tick: StateValue<Value<Color>>,
    pub label: StateValue<Value<Color>>,
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle::default(),
            track: stateful_single(
                palette.surface_high,
                palette.surface_high,
                palette.surface_high,
                palette.disabled_surface,
            ),
            active_track: stateful_single(
                palette.primary,
                palette.primary.lighten(hover_lighten()),
                palette.primary.darken(hover_lighten()),
                palette.disabled_surface,
            ),
            thumb: stateful_single(
                palette.surface,
                palette.surface,
                palette.surface_high,
                palette.disabled_content,
            ),
            thumb_shadow: Some(theme.elevation.sm.clone()),
            tick: stateful_single(
                palette.outline,
                palette.outline,
                palette.outline,
                palette.disabled_surface,
            ),
            label: stateful_single(
                palette.on_surface_muted,
                palette.on_surface,
                palette.on_surface,
                palette.disabled_content,
            ),
            focus_ring: None,
            track_height: dp(4.0),
            thumb_size: theme.spacing.md + theme.spacing.xs,
            radius: Value::Static(theme.radius.full),
            border_width: Value::Static(theme.border.none),
            tick_size: theme.spacing.xs,
            label_gap: theme.spacing.sm,
            min_width: dp(160.0),
            min_height: dp(32.0),
            text_style: theme.typography.label.clone(),
        }
    }
}
