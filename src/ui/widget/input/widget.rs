use crate::foundation::binding::{TextChangeSet, TextController};
use crate::foundation::form::ValidationVisualState;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    VisualStyle, WidgetId, WidgetKind,
};
use super::super::container::{
    set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue,
};
use super::super::core::Element;
use super::super::style::{InputStyle, StyleResolver};

/// 单行文本输入组件。
pub struct Input<VM> {
    element: Element<VM>,
}

macro_rules! impl_input_layout_api {
    () => {
        /// 设置组件宽高。
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.element.layout, width, height);
            self
        }

        /// 设置组件宽度。
        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.width, width);
            self
        }

        /// 设置组件高度。
        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.height, height);
            self
        }

        /// 设置最小宽度。
        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_width, width);
            self
        }

        /// 设置最小高度。
        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_height, height);
            self
        }

        /// 设置最大宽度。
        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_width, width);
            self
        }

        /// 设置最大高度。
        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_height, height);
            self
        }

        /// 设置宽高比。
        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.element.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        /// 设置外边距。
        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.margin = insets.into();
            self
        }

        /// 设置内边距。
        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.padding = Some(insets.into());
            self
        }

        /// 设置弹性增长值。
        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.element.layout.grow = grow.into();
            self
        }

        /// 设置弹性收缩值。
        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.element.layout.shrink = shrink.into();
            self
        }

        /// 设置基础尺寸。
        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.element.layout.basis = Some(basis.into_length_value());
            self
        }

        /// 设置自身在交叉轴上的对齐方式。
        pub fn align_self(mut self, align: Align) -> Self {
            self.element.layout.align_self = Some(align);
            self
        }

        /// 设置自身在主轴上的对齐方式。
        pub fn justify_self(mut self, align: Align) -> Self {
            self.element.layout.justify_self = Some(align);
            self
        }

        /// 设置网格列起始位置。
        pub fn column(mut self, start: usize) -> Self {
            self.element.layout.column_start = Some(start.max(1));
            self
        }

        /// 设置网格行起始位置。
        pub fn row(mut self, start: usize) -> Self {
            self.element.layout.row_start = Some(start.max(1));
            self
        }

        /// 设置横跨列数。
        pub fn column_span(mut self, span: usize) -> Self {
            self.element.layout.column_span = span.max(1);
            self
        }

        /// 设置横跨行数。
        pub fn row_span(mut self, span: usize) -> Self {
            self.element.layout.row_span = span.max(1);
            self
        }

        /// 切换为绝对定位。
        pub fn position_absolute(mut self) -> Self {
            self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        /// 设置左侧偏移。
        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            self
        }

        /// 设置顶部偏移。
        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.top, value);
            self
        }

        /// 设置右侧偏移。
        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.right, value);
            self
        }

        /// 设置底部偏移。
        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }

        /// 同时设置四边偏移。
        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            set_layout_inset(&mut self.element.layout.top, value);
            set_layout_inset(&mut self.element.layout.right, value);
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }
    };
}

impl<VM> Input<VM> {
    /// 创建单行文本输入组件。
    ///
    /// # 参数
    /// - `controller`：用于读写文本内容和光标状态的文本控制器。
    ///
    /// # 返回值
    /// 返回新的单行输入组件实例。
    pub fn new(controller: impl Into<TextController>) -> Self {
        let interactions = InteractionHandlers {
            cursor_style: Some(Value::Static(CursorStyle::Text)),
            ..Default::default()
        };
        let controller = controller.into();

        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions,
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                tooltip: None,
                popover: None,
                menu: None,
                context_menu: None,
                modal: None,
                drawer: None,
                tab_trigger: None,
                list_item: None,
                tree_root: None,
                tree_node: None,
                data_grid_root: None,
                data_grid_cell: None,
                data_grid_header: None,
                data_grid_resize_handle: None,
                splitter_handle: None,
                carousel_auto_play: None,
                kind: WidgetKind::TextEditor {
                    controller,
                    placeholder: Value::Static(String::new()),
                    on_change: None,
                    on_change_set: None,
                    disabled: Value::Static(false),
                    input_style: None,
                    textarea_style: None,
                    multiline: false,
                    show_scrollbar: Value::Static(false),
                    auto_wrap: Value::Static(false),
                    validation: Value::Static(ValidationVisualState::default()),
                },
            },
        }
    }

    /// 从静态值或响应式值创建单行输入组件。
    ///
    /// # 参数
    /// - `value`：用于初始化并绑定输入内容的文本值。
    ///
    /// # 返回值
    /// 返回新的单行输入组件实例。
    pub fn from_value(value: impl Into<Value<String>>) -> Self {
        Self::new(TextController::new_legacy(value.into()))
    }

    impl_input_layout_api!();

    /// 设置组件 key。
    pub fn key(mut self, key: impl Into<super::super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    /// 设置占位文本。
    ///
    /// # 参数
    /// - `placeholder`：未输入内容时显示的静态或响应式文本。
    ///
    /// # 返回值
    /// 返回更新后的输入组件。
    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        if let WidgetKind::TextEditor {
            placeholder: target,
            ..
        } = &mut self.element.kind
        {
            *target = placeholder.into();
        }
        self
    }

    /// 设置内容变化时触发的无参命令。
    ///
    /// # 参数
    /// - `command`：文本提交到绑定值后执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的输入组件。
    pub fn on_change(mut self, command: Command<VM>) -> Self {
        if let WidgetKind::TextEditor { on_change, .. } = &mut self.element.kind {
            *on_change = Some(command);
        }
        self
    }

    /// 设置内容变化集合回调命令。
    ///
    /// # 参数
    /// - `command`：接收本次编辑变更集合的命令。
    ///
    /// # 返回值
    /// 返回更新后的输入组件。
    pub fn on_change_set(mut self, command: ValueCommand<VM, TextChangeSet>) -> Self {
        if let WidgetKind::TextEditor { on_change_set, .. } = &mut self.element.kind {
            *on_change_set = Some(command);
        }
        self
    }

    /// 设置是否禁用输入。
    ///
    /// # 参数
    /// - `disable`：静态或响应式的禁用状态。
    ///
    /// # 返回值
    /// 返回更新后的输入组件。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        if let WidgetKind::TextEditor { disabled, .. } = &mut self.element.kind {
            *disabled = disable.into();
        }
        self
    }

    /// 设置校验视觉状态。
    pub fn validation(mut self, validation: impl Into<Value<ValidationVisualState>>) -> Self {
        if let WidgetKind::TextEditor {
            validation: target, ..
        } = &mut self.element.kind
        {
            *target = validation.into();
        }
        self
    }

    /// 设置输入框主题样式解析器。
    ///
    /// # 参数
    /// - `resolver`：根据主题模式返回输入框样式的解析函数。
    ///
    /// # 返回值
    /// 返回更新后的输入组件。
    pub fn style(
        mut self,
        mutator: impl Fn(&mut InputStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::TextEditor { input_style, .. } = &mut self.element.kind {
            *input_style = Some(StyleResolver::mutate(
                |context| InputStyle::default_for_theme(context.theme),
                mutator,
            ));
        }
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> InputStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::TextEditor { input_style, .. } = &mut self.element.kind {
            *input_style = Some(StyleResolver::full(resolver));
        }
        self
    }

    pub(crate) fn style_full_with_style_sheet(
        mut self,
        resolver: impl Fn(
                &StyleContext<'_>,
                &crate::ui::widget::StyleSheet,
                &VisualStyle,
                WidgetState,
            ) -> InputStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        if let WidgetKind::TextEditor { input_style, .. } = &mut self.element.kind {
            *input_style = Some(StyleResolver::full_with_style_sheet(resolver));
        }
        self
    }

    /// 设置点击命令。
    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    /// 设置双击命令。
    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    /// 设置获得焦点命令。
    pub fn on_focus(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_focus = Some(command);
        self
    }

    /// 设置失去焦点命令。
    pub fn on_blur(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_blur = Some(command);
        self
    }

    /// 设置鼠标进入命令。
    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    /// 设置鼠标离开命令。
    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    /// 设置鼠标移动命令。
    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    /// 设置挂载命令。
    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    /// 设置卸载命令。
    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    /// 设置更新命令。
    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    /// 设置鼠标指针样式。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Input<VM>> for Element<VM> {
    fn from(value: Input<VM>) -> Self {
        value.element
    }
}
