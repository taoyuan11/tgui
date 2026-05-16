use crate::foundation::binding::Signal;
use crate::ui::layout::{LayoutStyle, Length, Value};
use crate::ui::unit::Dp;

/// 定义可转换为布局长度值的输入类型。
///
/// 该 trait 统一处理静态值、`Signal` 和 `Value`，用于 widget 的尺寸与定位 builder。
pub trait IntoLengthValue {
    /// 将输入转换为布局系统使用的长度值。
    ///
    /// # 返回值
    /// 返回标准化后的 `Value<Length>`，供布局样式直接消费。
    fn into_length_value(self) -> Value<Length>;
}

impl IntoLengthValue for Length {
    fn into_length_value(self) -> Value<Length> {
        self.into()
    }
}

impl IntoLengthValue for Dp {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for f32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for f64 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for i32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for u32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for Value<Length> {
    fn into_length_value(self) -> Value<Length> {
        self
    }
}

impl IntoLengthValue for Signal<Length> {
    fn into_length_value(self) -> Value<Length> {
        self.into()
    }
}

impl IntoLengthValue for Signal<Dp> {
    fn into_length_value(self) -> Value<Length> {
        self.map(Length::from).into()
    }
}

impl IntoLengthValue for Value<Dp> {
    fn into_length_value(self) -> Value<Length> {
        match self {
            Value::Static(value) => Length::from(value).into(),
            Value::Signal(signal) => signal.map(Length::from).into(),
        }
    }
}

impl IntoLengthValue for Value<f32> {
    fn into_length_value(self) -> Value<Length> {
        match self {
            Value::Static(value) => Length::from(value).into(),
            Value::Signal(signal) => signal.map(Length::from).into(),
        }
    }
}

pub(crate) fn set_layout_length(target: &mut Option<Value<Length>>, value: impl IntoLengthValue) {
    *target = Some(value.into_length_value());
}

pub(crate) fn set_layout_lengths(
    layout: &mut LayoutStyle,
    width: impl IntoLengthValue,
    height: impl IntoLengthValue,
) {
    set_layout_length(&mut layout.width, width);
    set_layout_length(&mut layout.height, height);
}

pub(crate) fn set_layout_inset(target: &mut Option<Value<Length>>, value: impl IntoLengthValue) {
    *target = Some(value.into_length_value());
}
