use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::{Axis, Value};
use crate::ui::unit::dp;

use super::super::container::Flex;
use super::super::core::Element;
use super::widget::{radio_option_selected, Radio};

/// 单选项定义。
#[derive(Clone)]
pub struct RadioOption<K, V> {
    key: K,
    value: V,
    label: Option<Value<String>>,
    disabled: Value<bool>,
}

impl<K, V> RadioOption<K, V> {
    /// 创建一个新的单选项。
    pub fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            label: None,
            disabled: Value::Static(false),
        }
    }

    /// 设置单选项标签。
    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置单选项禁用状态。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }
}

impl<K, V> From<(K, V)> for RadioOption<K, V> {
    fn from((key, value): (K, V)) -> Self {
        Self::new(key, value)
    }
}

/// 单选按钮组。
pub struct RadioGroup<VM, K, V> {
    options: Vec<RadioOption<K, V>>,
    selected_key: Value<K>,
    direction: Axis,
    on_change: Option<ValueCommand<VM, (K, V)>>,
}

impl<VM, K, V> RadioGroup<VM, K, V> {
    /// 创建单选按钮组。
    pub fn new<O>(options: Vec<O>, selected_key: impl Into<Value<K>>) -> Self
    where
        O: Into<RadioOption<K, V>>,
    {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected_key: selected_key.into(),
            direction: Axis::Vertical,
            on_change: None,
        }
    }

    /// 设置组选中变更回调。
    pub fn on_change(mut self, command: ValueCommand<VM, (K, V)>) -> Self {
        self.on_change = Some(command);
        self
    }

    /// 设置排列方向。
    pub fn direction(mut self, direction: Axis) -> Self {
        self.direction = direction;
        self
    }

    /// 设置为水平方向排列。
    pub fn horizontal(self) -> Self {
        self.direction(Axis::Horizontal)
    }

    /// 设置为垂直方向排列。
    pub fn vertical(self) -> Self {
        self.direction(Axis::Vertical)
    }
}

impl<VM, K, V> From<RadioGroup<VM, K, V>> for Element<VM>
where
    VM: 'static,
    K: Clone + PartialEq + Send + Sync + 'static,
    V: Clone + Into<Value<String>> + Send + Sync + 'static,
{
    fn from(group: RadioGroup<VM, K, V>) -> Self {
        let mut children = Vec::with_capacity(group.options.len());
        for option in group.options {
            let selected = radio_option_selected(&group.selected_key, option.key.clone());
            let label = option
                .label
                .clone()
                .unwrap_or_else(|| option.value.clone().into());
            let mut radio = Radio::new(selected).label(label).disable(option.disabled);

            if let Some(command) = group.on_change.clone() {
                let key = option.key.clone();
                let value = option.value.clone();
                radio = radio.on_change(ValueCommand::new_with_context(
                    move |view_model: &mut VM, checked, context| {
                        if checked {
                            command.execute_with_context(
                                view_model,
                                (key.clone(), value.clone()),
                                context,
                            );
                        }
                    },
                ));
            }

            children.push(Element::from(radio));
        }

        Flex::new(group.direction)
            .gap(dp(8.0))
            .child(children)
            .into()
    }
}
