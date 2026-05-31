use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::SwitchStyle;

/// 开关组件。
pub struct Switch<VM> {
    element: Element<VM>,
}

macro_rules! impl_widget_layout_api {
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

impl<VM> Switch<VM> {
    /// 创建开关组件。
    ///
    /// # 参数
    /// - `checked`：当前是否开启。
    ///
    /// # 返回值
    /// 返回一个新的开关组件。
    pub fn new(checked: impl Into<Value<bool>>) -> Self {
        let mut interactions = InteractionHandlers::default();
        interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

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
                kind: WidgetKind::Switch {
                    checked: checked.into(),
                    on_change: None,
                    active_background: None,
                    inactive_background: None,
                    active_thumb_color: None,
                    inactive_thumb_color: None,
                    disabled: Value::Static(false),
                    style: None,
                },
            },
        }
    }

    impl_widget_layout_api!();

    /// 设置组件 key。
    pub fn key(mut self, key: impl Into<super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    /// 设置开关样式解析器。
    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> SwitchStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Switch { style, .. } = &mut self.element.kind {
            *style = Some(super::style::StyleResolver::new(resolver));
        }
        self
    }

    /// 设置值变更回调。
    pub fn on_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        if let WidgetKind::Switch { on_change, .. } = &mut self.element.kind {
            *on_change = Some(command);
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

    /// 设置聚焦命令。
    pub fn on_focus(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_focus = Some(command);
        self
    }

    /// 设置失焦命令。
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

    /// 设置禁用状态。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        if let WidgetKind::Switch { disabled, .. } = &mut self.element.kind {
            *disabled = disable.into();
        }
        self
    }

    /// 设置鼠标指针样式。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Switch<VM>> for Element<VM> {
    fn from(value: Switch<VM>) -> Self {
        value.element
    }
}
