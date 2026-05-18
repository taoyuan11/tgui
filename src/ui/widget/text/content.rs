use std::fmt::Display;

use crate::foundation::binding::Signal;
use crate::ui::layout::Value;

/// 定义可转换为文本内容的输入类型。
pub trait IntoTextContent {
    /// 将输入转换为文本组件可消费的内容值。
    ///
    /// # 返回值
    /// 返回静态或响应式的 `Value<String>`。
    fn into_text_content(self) -> Value<String>;
}

impl<T> IntoTextContent for T
where
    T: Display,
{
    fn into_text_content(self) -> Value<String> {
        Value::Static(self.to_string())
    }
}

impl<T> IntoTextContent for Signal<T>
where
    T: Display + Clone + Send + Sync + 'static,
{
    fn into_text_content(self) -> Value<String> {
        Value::Signal(self.project(|value| value.to_string()))
    }
}

impl<T> IntoTextContent for Value<T>
where
    T: Display + Clone + Send + Sync + 'static,
{
    fn into_text_content(self) -> Value<String> {
        match self {
            Value::Static(value) => Value::Static(value.to_string()),
            Value::Signal(signal) => Value::Signal(signal.project(|value| value.to_string())),
        }
    }
}
