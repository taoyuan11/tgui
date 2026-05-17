use super::*;

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
