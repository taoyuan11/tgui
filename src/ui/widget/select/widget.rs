use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    SelectOptionState, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::super::container::{
    set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue,
};
use super::super::core::Element;
use super::super::style::{SelectStyle, StyleResolver};
use super::SelectOption;

/// 下拉选择组件。
pub struct Select<VM, K, V> {
    key: Option<WidgetKey>,
    options: Vec<SelectOption<K, V>>,
    selected_key: Value<Option<K>>,
    placeholder: Value<String>,
    open: Option<Value<bool>>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, (K, V)>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    lifecycle_events: LifecycleEventHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    background: Option<Value<Color>>,
    style: Option<StyleResolver<SelectStyle>>,
}

macro_rules! impl_select_layout_api {
    () => {
        /// 设置组件宽高。
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.layout, width, height);
            self
        }

        /// 设置组件宽度。
        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.width, width);
            self
        }

        /// 设置组件高度。
        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.height, height);
            self
        }

        /// 设置最小宽度。
        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_width, width);
            self
        }

        /// 设置最小高度。
        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_height, height);
            self
        }

        /// 设置最大宽度。
        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_width, width);
            self
        }

        /// 设置最大高度。
        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_height, height);
            self
        }

        /// 设置宽高比。
        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        /// 设置外边距。
        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        /// 设置内边距。
        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
            self
        }

        /// 设置弹性增长值。
        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.layout.grow = grow.into();
            self
        }

        /// 设置弹性收缩值。
        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.layout.shrink = shrink.into();
            self
        }

        /// 设置基础尺寸。
        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        /// 设置自身在交叉轴上的对齐方式。
        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        /// 设置自身在主轴上的对齐方式。
        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
            self
        }

        /// 设置网格列起始位置。
        pub fn column(mut self, start: usize) -> Self {
            self.layout.column_start = Some(start.max(1));
            self
        }

        /// 设置网格行起始位置。
        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(start.max(1));
            self
        }

        /// 设置横跨列数。
        pub fn column_span(mut self, span: usize) -> Self {
            self.layout.column_span = span.max(1);
            self
        }

        /// 设置横跨行数。
        pub fn row_span(mut self, span: usize) -> Self {
            self.layout.row_span = span.max(1);
            self
        }

        /// 切换为绝对定位。
        pub fn position_absolute(mut self) -> Self {
            self.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        /// 设置左侧偏移。
        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            self
        }

        /// 设置顶部偏移。
        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.top, value);
            self
        }

        /// 设置右侧偏移。
        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.right, value);
            self
        }

        /// 设置底部偏移。
        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }

        /// 同时设置四边偏移。
        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl<VM, K, V> Select<VM, K, V> {
    /// 创建下拉选择组件。
    ///
    /// # 参数
    /// - `options`：候选选项列表。
    /// - `selected_key`：当前选中的 key。
    ///
    /// # 返回值
    /// 返回新的下拉选择组件。
    pub fn new<O>(options: Vec<O>, selected_key: impl Into<Value<Option<K>>>) -> Self
    where
        O: Into<SelectOption<K, V>>,
    {
        let mut interactions = InteractionHandlers::default();
        interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

        Self {
            key: None,
            options: options.into_iter().map(Into::into).collect(),
            selected_key: selected_key.into(),
            placeholder: Value::Static(String::new()),
            open: None,
            disabled: Value::Static(false),
            on_change: None,
            on_open_change: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: None,
            style: None,
        }
    }

    impl_select_layout_api!();

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置禁用状态。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    /// 设置展开状态。
    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = Some(open.into());
        self
    }

    /// 设置选中值变更回调。
    pub fn on_change(mut self, command: ValueCommand<VM, (K, V)>) -> Self {
        self.on_change = Some(command);
        self
    }

    /// 设置展开状态变更回调。
    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    /// 设置样式解析器。
    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> SelectStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::new(resolver));
        self
    }

    /// 设置组件 key。
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 设置点击命令。
    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_click = Some(command);
        self
    }

    /// 设置双击命令。
    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_double_click = Some(command);
        self
    }

    /// 设置聚焦命令。
    pub fn on_focus(mut self, command: Command<VM>) -> Self {
        self.interactions.on_focus = Some(command);
        self
    }

    /// 设置失焦命令。
    pub fn on_blur(mut self, command: Command<VM>) -> Self {
        self.interactions.on_blur = Some(command);
        self
    }

    /// 设置鼠标进入命令。
    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_enter = Some(command);
        self
    }

    /// 设置鼠标离开命令。
    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_leave = Some(command);
        self
    }

    /// 设置鼠标移动命令。
    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.interactions.on_mouse_move = Some(command);
        self
    }

    /// 设置挂载命令。
    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_mount = Some(command);
        self
    }

    /// 设置卸载命令。
    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_unmount = Some(command);
        self
    }

    /// 设置更新命令。
    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_update = Some(command);
        self
    }

    /// 设置鼠标指针样式。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM, K, V> From<Select<VM, K, V>> for Element<VM>
where
    VM: 'static,
    K: Clone + PartialEq + Send + Sync + 'static,
    V: Clone + Into<Value<String>> + Send + Sync + 'static,
{
    fn from(select: Select<VM, K, V>) -> Self {
        let label_options = select
            .options
            .iter()
            .map(|option| {
                let label = option
                    .label
                    .clone()
                    .unwrap_or_else(|| option.value.clone().into());
                (option.key.clone(), label)
            })
            .collect::<Vec<_>>();
        let selected_label = select_selected_label(&select.selected_key, label_options.clone());
        let options = select
            .options
            .into_iter()
            .zip(label_options)
            .map(|(option, (key, label))| {
                let selected = select_option_selected(&select.selected_key, key);
                let on_select = select.on_change.clone().map(|command| {
                    let key = option.key.clone();
                    let value = option.value.clone();
                    Command::new_with_context(move |view_model: &mut VM, context| {
                        command.execute_with_context(
                            view_model,
                            (key.clone(), value.clone()),
                            context,
                        );
                    })
                });
                SelectOptionState {
                    label,
                    selected,
                    disabled: option.disabled,
                    on_select,
                }
            })
            .collect();

        Element {
            id: WidgetId::next(),
            key: select.key,
            layout: select.layout,
            visual: select.visual,
            interactions: select.interactions,
            lifecycle_events: select.lifecycle_events,
            media_events: select.media_events,
            background: select.background,
            kind: WidgetKind::Select {
                selected_label,
                placeholder: select.placeholder,
                options,
                open: select.open,
                on_open_change: select.on_open_change,
                disabled: select.disabled,
                style: select.style,
            },
        }
    }
}

fn select_option_selected<K>(selected_key: &Value<Option<K>>, option_key: K) -> Value<bool>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    match selected_key {
        Value::Static(current) => Value::Static(current.as_ref() == Some(&option_key)),
        Value::Signal(signal) => signal
            .map(move |current| current.as_ref() == Some(&option_key))
            .into(),
    }
}

fn select_selected_label<K>(
    selected_key: &Value<Option<K>>,
    options: Vec<(K, Value<String>)>,
) -> Value<Option<String>>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    match selected_key {
        Value::Static(current) => Value::Static(
            current
                .as_ref()
                .and_then(|key| selected_label_for_key(key, &options)),
        ),
        Value::Signal(signal) => signal
            .map(move |current| {
                current
                    .as_ref()
                    .and_then(|key| selected_label_for_key(key, &options))
            })
            .into(),
    }
}

fn selected_label_for_key<K>(key: &K, options: &[(K, Value<String>)]) -> Option<String>
where
    K: PartialEq,
{
    options
        .iter()
        .find(|(option_key, _)| option_key == key)
        .map(|(_, label)| label.resolve())
}
