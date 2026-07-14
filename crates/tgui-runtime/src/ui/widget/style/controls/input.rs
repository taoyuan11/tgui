use super::*;

/// 单行输入框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct InputStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub text: StateValue<Value<Color>>,
    pub placeholder: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let palette = palette_from_theme(theme);
        let metrics = control_density_metrics(theme, density);
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
            border: stateful_colors(
                palette.outline_muted,
                palette.outline,
                palette.primary,
                palette.disabled_surface,
            ),
            selection: Some(Value::Static(theme.colors.selection)),
            caret: Some(Value::Static(theme.colors.primary)),
            border_width: Value::Static(theme.border.thin),
            radius: Value::Static(theme.radius.lg),
            padding_x: metrics.input_padding_x,
            padding_y: metrics.input_padding_y,
            min_height: metrics.control_height,
            text_style: theme.typography.body.clone(),
        }
    }
}

/// 多行文本框 widget 的样式定义。
#[derive(Clone, Debug, PartialEq)]
pub struct TextareaStyle {
    pub surface: WidgetSurfaceStyle,
    pub background: StateValue<Value<Color>>,
    pub text: StateValue<Value<Color>>,
    pub placeholder: StateValue<Value<Color>>,
    pub border: StateValue<Value<Color>>,
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
    pub fn default_for_theme(theme: &Theme) -> Self {
        Self::default_for_density(theme, theme.density)
    }

    pub(crate) fn default_for_density(theme: &Theme, density: Density) -> Self {
        let mut style = InputStyle::default_for_density(theme, density);
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
