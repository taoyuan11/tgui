use crate::ui::layout::Value;

/// 下拉选择组件的选项定义。
#[derive(Clone)]
pub struct SelectOption<K, V> {
    pub(super) key: K,
    pub(super) value: V,
    pub(super) label: Option<Value<String>>,
    pub(super) disabled: Value<bool>,
}

impl<K, V> SelectOption<K, V> {
    /// 创建一个新的下拉选项。
    ///
    /// # 参数
    /// - `key`：选项唯一标识。
    /// - `value`：选项对应值。
    ///
    /// # 返回值
    /// 返回新的选项定义。
    pub fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            label: None,
            disabled: Value::Static(false),
        }
    }

    /// 设置选项显示文本。
    ///
    /// # 参数
    /// - `label`：选项显示标签。
    ///
    /// # 返回值
    /// 返回更新后的选项定义。
    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置选项禁用状态。
    ///
    /// # 参数
    /// - `disable`：是否禁用该选项。
    ///
    /// # 返回值
    /// 返回更新后的选项定义。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }
}

impl<K, V> From<(K, V)> for SelectOption<K, V> {
    fn from((key, value): (K, V)) -> Self {
        Self::new(key, value)
    }
}
