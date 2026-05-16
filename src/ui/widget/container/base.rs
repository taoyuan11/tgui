use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, Justify, LayoutStyle, Overflow, Value};

use super::super::common::{
    ContainerLayout, CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers,
    Point, VisualStyle, WidgetId, WidgetKind,
};
use super::super::core::Element;
use super::super::style::ContainerStyle;
use super::length::IntoLengthValue;
use super::IntoChildren;

pub(super) fn apply_layout_api<VM, T>(
    mut owner: T,
    element: impl Fn(&mut T) -> &mut Element<VM>,
    op: impl FnOnce(&mut LayoutStyle),
) -> T {
    op(&mut element(&mut owner).layout);
    owner
}

/// 容器类 widget 的基础实现。
///
/// 该类型负责承载通用交互、生命周期和子节点管理逻辑，具体布局类型在其上继续封装。
pub struct Container<VM> {
    pub(super) element: Element<VM>,
}

impl<VM> Container<VM> {
    pub(crate) fn with_layout(layout: ContainerLayout) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                kind: WidgetKind::Container {
                    layout,
                    children: Vec::new(),
                    style: None,
                },
            },
        }
    }

    /// 设置容器样式解析器。
    ///
    /// # 参数
    /// - `resolver`：根据主题模式生成容器样式的回调。
    ///
    /// # 返回值
    /// 返回更新后的容器实例，便于继续链式调用。
    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> ContainerStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Container { style, .. } = &mut self.element.kind {
            *style = Some(super::super::style::StyleResolver::new(resolver));
        }
        self
    }

    /// 设置容器稳定 key。
    ///
    /// # 参数
    /// - `key`：用于保留节点身份的逻辑 key。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn key(mut self, key: impl Into<super::super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    /// 注册点击命令。
    ///
    /// # 参数
    /// - `command`：点击时执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    /// 注册双击命令。
    ///
    /// # 参数
    /// - `command`：双击时执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    /// 注册鼠标进入命令。
    ///
    /// # 参数
    /// - `command`：鼠标进入时执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    /// 注册鼠标离开命令。
    ///
    /// # 参数
    /// - `command`：鼠标离开时执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    /// 注册鼠标移动命令。
    ///
    /// # 参数
    /// - `command`：接收当前指针坐标的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    /// 注册挂载完成命令。
    ///
    /// # 参数
    /// - `command`：节点进入树后执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    /// 注册卸载命令。
    ///
    /// # 参数
    /// - `command`：节点移出树时执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    /// 注册更新命令。
    ///
    /// # 参数
    /// - `command`：节点更新后执行的命令。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    /// 设置鼠标悬停时的光标样式。
    ///
    /// # 参数
    /// - `cursor`：静态或响应式光标样式。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }

    /// 追加一个子节点来源。
    ///
    /// # 参数
    /// - `child`：单个子节点、子节点集合或动态子节点来源。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn child(mut self, child: impl IntoChildren<VM>) -> Self {
        if let WidgetKind::Container { children, .. } = &mut self.element.kind {
            children.push(child.into_child_source());
        }
        self
    }

    /// 设置容器内边距。
    ///
    /// # 参数
    /// - `padding`：静态或响应式内边距值。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn padding(mut self, padding: impl Into<Value<Insets>>) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.padding = Some(padding.into());
        }
        self
    }

    /// 设置容器子项间距。
    ///
    /// # 参数
    /// - `gap`：容器主布局使用的间距值。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn gap(mut self, gap: impl IntoLengthValue) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.gap = gap.into_length_value();
        }
        self
    }

    /// 设置主轴分布方式。
    ///
    /// # 参数
    /// - `justify`：容器主轴上的排列策略。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn justify(mut self, justify: Justify) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.justify = justify;
        }
        self
    }

    /// 设置交叉轴对齐方式。
    ///
    /// # 参数
    /// - `align`：容器交叉轴上的对齐策略。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn align(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align = align;
        }
        self
    }

    /// 同时将主轴和交叉轴对齐方式设为居中。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn center(self) -> Self {
        self.justify(Justify::Center).align(Align::Center)
    }

    /// 同时设置水平和垂直滚动溢出策略。
    ///
    /// # 参数
    /// - `overflow`：统一使用的溢出策略。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_x = overflow;
            layout.overflow_y = overflow;
        }
        self
    }

    /// 设置水平方向的溢出策略。
    ///
    /// # 参数
    /// - `overflow`：水平溢出策略。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_x = overflow;
        }
        self
    }

    /// 设置垂直方向的溢出策略。
    ///
    /// # 参数
    /// - `overflow`：垂直溢出策略。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_y = overflow;
        }
        self
    }
}

impl<VM> From<Container<VM>> for Element<VM> {
    fn from(value: Container<VM>) -> Self {
        value.element
    }
}
