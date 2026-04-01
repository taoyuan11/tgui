use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{Align, Axis, Insets, Justify, LayoutStyle, Wrap};

use super::common::{
    ContainerKind, ContainerLayout, InteractionHandlers, Point, Value, VisualStyle, WidgetId,
    WidgetKind,
};
use super::core::Element;

pub trait IntoChildren<VM> {
    fn into_children(self) -> Vec<Element<VM>>;
}

impl<VM, T> IntoChildren<VM> for T
where
    T: Into<Element<VM>>,
{
    fn into_children(self) -> Vec<Element<VM>> {
        vec![self.into()]
    }
}

impl<VM, const N: usize> IntoChildren<VM> for [Element<VM>; N] {
    fn into_children(self) -> Vec<Element<VM>> {
        self.into_iter().collect()
    }
}

impl<VM> IntoChildren<VM> for Vec<Element<VM>> {
    fn into_children(self) -> Vec<Element<VM>> {
        self
    }
}

pub struct Container<VM> {
    element: Element<VM>,
}

impl<VM> Container<VM> {
    pub fn new() -> Self {
        Self::with_layout(ContainerLayout::flow())
    }

    pub(crate) fn with_layout(layout: ContainerLayout) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                background: None,
                kind: WidgetKind::Container {
                    layout,
                    children: Vec::new(),
                },
            },
        }
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn border(
        mut self,
        width: impl Into<Value<f32>>,
        color: impl Into<Value<Color>>,
    ) -> Self {
        self.element.visual.border_width = width.into();
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<f32>>) -> Self {
        self.element.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<f32>>) -> Self {
        self.element.visual.border_width = width.into();
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.element.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.element.visual.offset = offset.into();
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn child(mut self, child: impl IntoChildren<VM>) -> Self {
        if let WidgetKind::Container { children, .. } = &mut self.element.kind {
            children.extend(child.into_children());
        }
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.padding = padding;
        }
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.gap = gap;
        }
        self
    }

    pub fn justify(mut self, justify: Justify) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.justify = justify;
        }
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align = align;
            layout.align_x = Some(align);
            layout.align_y = Some(align);
        }
        self
    }

    pub fn align_x(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align_x = Some(align);
        }
        self
    }

    pub fn align_y(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align_y = Some(align);
        }
        self
    }
}

impl<VM> Default for Container<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> From<Container<VM>> for Element<VM> {
    fn from(value: Container<VM>) -> Self {
        value.element
    }
}

pub struct Stack<VM>(Container<VM>);
pub struct Row<VM>(Container<VM>);
pub struct Column<VM>(Container<VM>);
pub struct Grid<VM>(Container<VM>);
pub struct Flex<VM>(Container<VM>);

macro_rules! impl_layout_container {
    ($name:ident) => {
        impl<VM> $name<VM> {
            pub fn size(mut self, width: f32, height: f32) -> Self {
                self.0.element.layout.width = Some(width);
                self.0.element.layout.height = Some(height);
                self.0.element.layout.fill_width = false;
                self.0.element.layout.fill_height = false;
                self
            }

            pub fn width(mut self, width: f32) -> Self {
                self.0.element.layout.width = Some(width);
                self.0.element.layout.fill_width = false;
                self
            }

            pub fn height(mut self, height: f32) -> Self {
                self.0.element.layout.height = Some(height);
                self.0.element.layout.fill_height = false;
                self
            }

            pub fn fill_width(mut self) -> Self {
                self.0.element.layout.fill_width = true;
                self.0.element.layout.width = None;
                self
            }

            pub fn fill_height(mut self) -> Self {
                self.0.element.layout.fill_height = true;
                self.0.element.layout.height = None;
                self
            }

            pub fn fill_size(mut self) -> Self {
                self.0.element.layout.fill_width = true;
                self.0.element.layout.fill_height = true;
                self.0.element.layout.width = None;
                self.0.element.layout.height = None;
                self
            }

            pub fn margin(mut self, insets: Insets) -> Self {
                self.0.element.layout.margin = insets;
                self
            }

            pub fn grow(mut self, grow: f32) -> Self {
                self.0.element.layout.grow = grow;
                self
            }

            pub fn background(self, color: impl Into<Value<Color>>) -> Self {
                Self(self.0.background(color))
            }

            pub fn border(
                self,
                width: impl Into<Value<f32>>,
                color: impl Into<Value<Color>>,
            ) -> Self {
                Self(self.0.border(width, color))
            }

            pub fn border_color(self, color: impl Into<Value<Color>>) -> Self {
                Self(self.0.border_color(color))
            }

            pub fn border_radius(self, radius: impl Into<Value<f32>>) -> Self {
                Self(self.0.border_radius(radius))
            }

            pub fn border_width(self, width: impl Into<Value<f32>>) -> Self {
                Self(self.0.border_width(width))
            }

            pub fn opacity(self, opacity: impl Into<Value<f32>>) -> Self {
                Self(self.0.opacity(opacity))
            }

            pub fn offset(self, offset: impl Into<Value<Point>>) -> Self {
                Self(self.0.offset(offset))
            }

            pub fn on_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_click(command))
            }

            pub fn on_double_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_double_click(command))
            }

            pub fn on_mouse_enter(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_enter(command))
            }

            pub fn on_mouse_leave(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_leave(command))
            }

            pub fn on_mouse_move(self, command: ValueCommand<VM, Point>) -> Self {
                Self(self.0.on_mouse_move(command))
            }

            pub fn child(self, child: impl IntoChildren<VM>) -> Self {
                Self(self.0.child(child))
            }

            pub fn padding(self, padding: Insets) -> Self {
                Self(self.0.padding(padding))
            }

            pub fn gap(self, gap: f32) -> Self {
                Self(self.0.gap(gap))
            }

            pub fn justify(self, justify: Justify) -> Self {
                Self(self.0.justify(justify))
            }

            pub fn align(self, align: Align) -> Self {
                Self(self.0.align(align))
            }

            pub fn align_x(self, align: Align) -> Self {
                Self(self.0.align_x(align))
            }

            pub fn align_y(self, align: Align) -> Self {
                Self(self.0.align_y(align))
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

impl<VM> Row<VM> {
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Row,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Default for Row<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> Column<VM> {
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Column,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Default for Column<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> Grid<VM> {
    pub fn new(columns: usize) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: columns.max(1),
            },
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Flex<VM> {
    pub fn new(direction: Axis) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Flex {
                direction,
                wrap: Wrap::NoWrap,
            },
            ..ContainerLayout::flow()
        }))
    }

    pub fn wrap(mut self, wrap: Wrap) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind {
                ContainerKind::Flex { direction, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }
}

impl_layout_container!(Stack);
impl_layout_container!(Row);
impl_layout_container!(Column);
impl_layout_container!(Grid);
impl_layout_container!(Flex);
