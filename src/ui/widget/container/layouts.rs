use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Axis, Insets, Overflow, Track, Value, Wrap};

use super::super::common::{ContainerKind, ContainerLayout, CursorStyle, Point, WidgetKind};
use super::super::core::Element;
use super::super::style::ContainerStyle;
use super::base::{apply_layout_api, Container};
use super::length::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::IntoChildren;

/// 以堆叠方式布局子节点的容器。
///
/// 该容器适合用于叠放元素、覆盖层和滚动内容承载。
pub struct Stack<VM>(Container<VM>);

/// 以网格方式布局子节点的容器。
///
/// 该容器通过行列轨道描述网格结构，适合复杂二维排版。
pub struct Grid<VM>(Container<VM>);

/// 以弹性盒模型布局子节点的容器。
///
/// 该容器支持主轴方向和换行配置，适合线性排列场景。
pub struct Flex<VM>(Container<VM>);

macro_rules! impl_layout_api {
    ($name:ident) => {
        impl<VM> $name<VM> {
            /// 设置组件的宽度和高度。
            ///
            /// # 参数
            /// - `width`：目标宽度。
            /// - `height`：目标高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn size(self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_lengths(layout, width, height);
                    },
                )
            }

            /// 设置组件宽度。
            ///
            /// # 参数
            /// - `width`：目标宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.width, width);
                    },
                )
            }

            /// 设置组件高度。
            ///
            /// # 参数
            /// - `height`：目标高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.height, height);
                    },
                )
            }

            /// 设置最小宽度。
            ///
            /// # 参数
            /// - `width`：最小宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn min_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_width, width);
                    },
                )
            }

            /// 设置最小高度。
            ///
            /// # 参数
            /// - `height`：最小高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn min_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_height, height);
                    },
                )
            }

            /// 设置最大宽度。
            ///
            /// # 参数
            /// - `width`：最大宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn max_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_width, width);
                    },
                )
            }

            /// 设置最大高度。
            ///
            /// # 参数
            /// - `height`：最大高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn max_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_height, height);
                    },
                )
            }

            /// 设置宽高比。
            ///
            /// # 参数
            /// - `aspect_ratio`：静态或响应式宽高比。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn aspect_ratio(self, aspect_ratio: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.aspect_ratio = Some(aspect_ratio.into());
                    },
                )
            }

            /// 设置外边距。
            ///
            /// # 参数
            /// - `insets`：静态或响应式外边距。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn margin(self, insets: impl Into<Value<Insets>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.margin = insets.into();
                    },
                )
            }

            /// 设置增长因子。
            ///
            /// # 参数
            /// - `grow`：弹性增长值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn grow(self, grow: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.grow = grow.into();
                    },
                )
            }

            /// 设置收缩因子。
            ///
            /// # 参数
            /// - `shrink`：弹性收缩值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn shrink(self, shrink: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.shrink = shrink.into();
                    },
                )
            }

            /// 设置基础尺寸。
            ///
            /// # 参数
            /// - `basis`：布局基础尺寸。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn basis(self, basis: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.basis = Some(basis.into_length_value());
                    },
                )
            }

            /// 设置自身在交叉轴上的对齐方式。
            ///
            /// # 参数
            /// - `align`：自定义对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn align_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.align_self = Some(align);
                    },
                )
            }

            /// 设置自身在主轴上的对齐方式。
            ///
            /// # 参数
            /// - `align`：自定义对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn justify_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.justify_self = Some(align);
                    },
                )
            }

            /// 设置网格列起始位置。
            ///
            /// # 参数
            /// - `start`：从 1 开始的列索引。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn column(self, start: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_start = Some(start.max(1));
                    },
                )
            }

            /// 设置网格行起始位置。
            ///
            /// # 参数
            /// - `start`：从 1 开始的行索引。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn row(self, start: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_start = Some(start.max(1));
                    },
                )
            }

            /// 设置横跨列数。
            ///
            /// # 参数
            /// - `span`：至少为 1 的跨度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn column_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_span = span.max(1);
                    },
                )
            }

            /// 设置横跨行数。
            ///
            /// # 参数
            /// - `span`：至少为 1 的跨度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn row_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_span = span.max(1);
                    },
                )
            }

            /// 将组件定位方式设为绝对定位。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn position_absolute(self) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.position_type = crate::ui::layout::PositionType::Absolute;
                    },
                )
            }

            /// 设置左侧偏移。
            ///
            /// # 参数
            /// - `value`：左侧 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn left(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.left, value);
                    },
                )
            }

            /// 设置顶部偏移。
            ///
            /// # 参数
            /// - `value`：顶部 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn top(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.top, value);
                    },
                )
            }

            /// 设置右侧偏移。
            ///
            /// # 参数
            /// - `value`：右侧 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn right(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.right, value);
                    },
                )
            }

            /// 设置底部偏移。
            ///
            /// # 参数
            /// - `value`：底部 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn bottom(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.bottom, value);
                    },
                )
            }

            /// 同时设置四个方向的偏移。
            ///
            /// # 参数
            /// - `value`：应用到四边的 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn inset(self, value: impl IntoLengthValue + Copy) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.left, value);
                        set_layout_inset(&mut layout.top, value);
                        set_layout_inset(&mut layout.right, value);
                        set_layout_inset(&mut layout.bottom, value);
                    },
                )
            }

            /// 设置容器样式解析器。
            ///
            /// # 参数
            /// - `resolver`：根据主题模式生成容器样式的回调。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn style(
                self,
                resolver: impl Fn(ResolvedThemeMode) -> ContainerStyle + Send + Sync + 'static,
            ) -> Self {
                Self(self.0.style(resolver))
            }

            /// 注册点击命令。
            ///
            /// # 参数
            /// - `command`：点击时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_click(command))
            }

            /// 注册双击命令。
            ///
            /// # 参数
            /// - `command`：双击时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_double_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_double_click(command))
            }

            /// 注册鼠标进入命令。
            ///
            /// # 参数
            /// - `command`：鼠标进入时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_enter(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_enter(command))
            }

            /// 注册鼠标离开命令。
            ///
            /// # 参数
            /// - `command`：鼠标离开时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_leave(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_leave(command))
            }

            /// 注册鼠标移动命令。
            ///
            /// # 参数
            /// - `command`：接收当前指针坐标的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_move(self, command: ValueCommand<VM, Point>) -> Self {
                Self(self.0.on_mouse_move(command))
            }

            /// 注册挂载完成命令。
            ///
            /// # 参数
            /// - `command`：节点进入树后执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mount(self, command: Command<VM>) -> Self {
                Self(self.0.on_mount(command))
            }

            /// 注册卸载命令。
            ///
            /// # 参数
            /// - `command`：节点移出树时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_unmount(self, command: Command<VM>) -> Self {
                Self(self.0.on_unmount(command))
            }

            /// 注册更新命令。
            ///
            /// # 参数
            /// - `command`：节点更新后执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_update(self, command: Command<VM>) -> Self {
                Self(self.0.on_update(command))
            }

            /// 设置鼠标悬停时的光标样式。
            ///
            /// # 参数
            /// - `cursor`：静态或响应式光标样式。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn cursor(self, cursor: impl Into<Value<CursorStyle>>) -> Self {
                Self(self.0.cursor(cursor))
            }

            /// 追加一个子节点来源。
            ///
            /// # 参数
            /// - `child`：单个子节点、子节点集合或动态子节点来源。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn child(self, child: impl IntoChildren<VM>) -> Self {
                Self(self.0.child(child))
            }

            /// 设置容器内边距。
            ///
            /// # 参数
            /// - `padding`：静态或响应式内边距值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn padding(self, padding: impl Into<Value<Insets>>) -> Self {
                Self(self.0.padding(padding))
            }

            /// 设置容器子项间距。
            ///
            /// # 参数
            /// - `gap`：容器主布局使用的间距值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn gap(self, gap: impl IntoLengthValue) -> Self {
                Self(self.0.gap(gap))
            }

            /// 设置主轴分布方式。
            ///
            /// # 参数
            /// - `justify`：容器主轴上的排列策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn justify(self, justify: crate::ui::layout::Justify) -> Self {
                Self(self.0.justify(justify))
            }

            /// 设置交叉轴对齐方式。
            ///
            /// # 参数
            /// - `align`：容器交叉轴上的对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn align(self, align: Align) -> Self {
                Self(self.0.align(align))
            }

            /// 同时将主轴和交叉轴对齐方式设为居中。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn center(self) -> Self {
                Self(self.0.center())
            }

            /// 同时设置水平和垂直方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：统一使用的溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow(self, overflow: Overflow) -> Self {
                Self(self.0.overflow(overflow))
            }

            /// 设置水平方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：水平溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow_x(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_x(overflow))
            }

            /// 设置垂直方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：垂直溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow_y(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_y(overflow))
            }
        }

        impl<VM> From<$name<VM>> for Element<VM> {
            fn from(value: $name<VM>) -> Self {
                value.0.into()
            }
        }
    };
}

impl<VM> Stack<VM> {
    /// 创建堆叠容器。
    ///
    /// # 返回值
    /// 返回一个采用 `Stack` 布局策略的容器实例。
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Stack,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Default for Stack<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> Grid<VM> {
    /// 创建仅指定列轨道的网格容器。
    ///
    /// # 参数
    /// - `columns`：网格列轨道定义。
    ///
    /// # 返回值
    /// 返回一个采用 `Grid` 布局策略的容器实例。
    pub fn columns<const N: usize>(columns: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: columns.into_iter().collect(),
                rows: Vec::new(),
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 创建仅指定行轨道的网格容器。
    ///
    /// # 参数
    /// - `rows`：网格行轨道定义。
    ///
    /// # 返回值
    /// 返回一个采用 `Grid` 布局策略的容器实例。
    pub fn rows<const N: usize>(rows: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: Vec::new(),
                rows: rows.into_iter().collect(),
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 更新网格列轨道定义。
    ///
    /// # 参数
    /// - `columns`：新的列轨道定义。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn set_columns<const N: usize>(mut self, columns: [Track; N]) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Grid { rows, .. } => ContainerKind::Grid {
                    columns: columns.into_iter().collect(),
                    rows,
                },
                other => other,
            };
        }
        self
    }

    /// 更新网格行轨道定义。
    ///
    /// # 参数
    /// - `rows`：新的行轨道定义。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn set_rows<const N: usize>(mut self, rows: [Track; N]) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Grid { columns, .. } => ContainerKind::Grid {
                    columns,
                    rows: rows.into_iter().collect(),
                },
                other => other,
            };
        }
        self
    }
}

impl<VM> Flex<VM> {
    /// 创建弹性容器。
    ///
    /// # 参数
    /// - `direction`：主轴方向。
    ///
    /// # 返回值
    /// 返回一个采用 `Flex` 布局策略的容器实例。
    pub fn new(direction: Axis) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Flex {
                direction,
                wrap: Wrap::NoWrap,
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 创建水平方向的弹性容器。
    ///
    /// # 返回值
    /// 返回主轴为水平方向的容器实例。
    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    /// 创建垂直方向的弹性容器。
    ///
    /// # 返回值
    /// 返回主轴为垂直方向的容器实例。
    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    /// 更新弹性容器主轴方向。
    ///
    /// # 参数
    /// - `direction`：新的主轴方向。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn direction(mut self, direction: Axis) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Flex { wrap, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }

    /// 更新弹性容器换行策略。
    ///
    /// # 参数
    /// - `wrap`：新的换行配置。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn wrap(mut self, wrap: Wrap) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Flex { direction, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }
}

impl_layout_api!(Stack);
impl_layout_api!(Grid);
impl_layout_api!(Flex);
