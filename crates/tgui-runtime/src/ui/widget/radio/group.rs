use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::{Axis, Value};
use crate::ui::unit::dp;
use std::sync::Arc;

use super::super::common::{RadioGroupInteraction, WidgetId};
use super::super::container::Flex;
use super::super::core::Element;
use super::super::For;
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
    id: WidgetId,
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
            id: WidgetId::next(),
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

#[derive(Clone, Copy)]
struct RadioGroupEntry {
    index: usize,
    tab_stop: bool,
}

impl<VM, K, V> From<RadioGroup<VM, K, V>> for Element<VM>
where
    VM: 'static,
    K: Clone + PartialEq + Send + Sync + 'static,
    V: Clone + Into<Value<String>> + Send + Sync + 'static,
{
    fn from(group: RadioGroup<VM, K, V>) -> Self {
        let RadioGroup {
            id,
            options,
            selected_key,
            direction,
            on_change,
        } = group;
        let options = Arc::new(
            options
                .into_iter()
                .map(|option| {
                    let selected = radio_option_selected(&selected_key, option.key.clone());
                    (option, selected)
                })
                .collect::<Vec<_>>(),
        );
        let entry_options = Arc::clone(&options);
        let render_options = Arc::clone(&options);
        let children = For::new_with_resolver(
            move || {
                let enabled = entry_options
                    .iter()
                    .map(|(option, _)| !option.disabled.resolve())
                    .collect::<Vec<_>>();
                let tab_stop = entry_options
                    .iter()
                    .enumerate()
                    .find_map(|(index, (_, selected))| {
                        (enabled[index] && selected.resolve()).then_some(index)
                    })
                    .or_else(|| enabled.iter().position(|enabled| *enabled));
                (0..entry_options.len())
                    .map(|index| RadioGroupEntry {
                        index,
                        tab_stop: Some(index) == tab_stop,
                    })
                    .collect()
            },
            |entry| entry.index,
            move |_position, entry| {
                let (option, selected) = &render_options[entry.index];
                let label = option
                    .label
                    .clone()
                    .unwrap_or_else(|| option.value.clone().into());
                let mut radio = Radio::new(selected.clone())
                    .label(label)
                    .disable(option.disabled.clone())
                    .group_item(
                        RadioGroupInteraction {
                            group_id: id,
                            index: entry.index,
                            direction,
                        },
                        entry.tab_stop,
                    );

                if let Some(command) = on_change.clone() {
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

                Element::from(radio)
            },
        );

        let mut element: Element<VM> = Flex::new(direction).gap(dp(8.0)).child(children).into();
        element.id = id;
        element
    }
}
