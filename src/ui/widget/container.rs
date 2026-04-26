use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, ScrollbarStyle, Track, Value, Wrap,
};
use crate::ui::unit::Dp;

use super::common::{
    ChildSource, ContainerKind, ContainerLayout, CursorStyle, InteractionHandlers,
    MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::core::Element;

trait IntoChildGroup<VM> {
    fn into_elements(self) -> Vec<Element<VM>>;
}

impl<VM, T> IntoChildGroup<VM> for T
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        vec![self.into()]
    }
}

impl<VM, T, const N: usize> IntoChildGroup<VM> for [T; N]
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<VM, T> IntoChildGroup<VM> for Vec<T>
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        self.into_iter().map(Into::into).collect()
    }
}

pub trait IntoChildren<VM> {
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM>;
}

impl<VM, T> IntoChildren<VM> for T
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(vec![self.into()])
    }
}

impl<VM, T, const N: usize> IntoChildren<VM> for [T; N]
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(self.into_elements())
    }
}

impl<VM, T> IntoChildren<VM> for Vec<T>
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(self.into_elements())
    }
}

impl<VM, T> IntoChildren<VM> for Binding<T>
where
    T: IntoChildGroup<VM> + Send + Sync + 'static,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Dynamic(std::sync::Arc::new(move || self.get().into_elements()))
    }
}

pub trait IntoLengthValue {
    fn into_length_value(self) -> Value<Length>;
}

impl IntoLengthValue for Length {
    fn into_length_value(self) -> Value<Length> {
        self.into()
    }
}

impl IntoLengthValue for Dp {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for f32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for f64 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for i32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for u32 {
    fn into_length_value(self) -> Value<Length> {
        Length::from(self).into()
    }
}

impl IntoLengthValue for Value<Length> {
    fn into_length_value(self) -> Value<Length> {
        self
    }
}

impl IntoLengthValue for Binding<Length> {
    fn into_length_value(self) -> Value<Length> {
        self.into()
    }
}

impl IntoLengthValue for Binding<Dp> {
    fn into_length_value(self) -> Value<Length> {
        self.map(Length::from).into()
    }
}

impl IntoLengthValue for Value<Dp> {
    fn into_length_value(self) -> Value<Length> {
        match self {
            Value::Static(value) => Length::from(value).into(),
            Value::Bound(binding) => binding.map(Length::from).into(),
        }
    }
}

impl IntoLengthValue for Value<f32> {
    fn into_length_value(self) -> Value<Length> {
        match self {
            Value::Static(value) => Length::from(value).into(),
            Value::Bound(binding) => binding.map(Length::from).into(),
        }
    }
}

pub(crate) fn set_layout_length(target: &mut Option<Value<Length>>, value: impl IntoLengthValue) {
    *target = Some(value.into_length_value());
}

pub(crate) fn set_layout_lengths(
    layout: &mut LayoutStyle,
    width: impl IntoLengthValue,
    height: impl IntoLengthValue,
) {
    set_layout_length(&mut layout.width, width);
    set_layout_length(&mut layout.height, height);
}

pub(crate) fn set_layout_inset(target: &mut Option<Value<Length>>, value: impl IntoLengthValue) {
    *target = Some(value.into_length_value());
}

fn apply_layout_api<VM, T>(
    mut owner: T,
    element: impl Fn(&mut T) -> &mut Element<VM>,
    op: impl FnOnce(&mut LayoutStyle),
) -> T {
    op(&mut element(&mut owner).layout);
    owner
}

pub struct Container<VM> {
    element: Element<VM>,
}

impl<VM> Container<VM> {
    pub(crate) fn with_layout(layout: ContainerLayout) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                media_events: MediaEventHandlers::default(),
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

    pub fn border(mut self, width: impl Into<Value<Dp>>, color: impl Into<Value<Color>>) -> Self {
        self.element.visual.border_width = width.into();
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.element.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<Dp>>) -> Self {
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

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }

    pub fn child(mut self, child: impl IntoChildren<VM>) -> Self {
        if let WidgetKind::Container { children, .. } = &mut self.element.kind {
            children.push(child.into_child_source());
        }
        self
    }

    pub fn padding(mut self, padding: impl Into<Value<Insets>>) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.padding = padding.into();
        }
        self
    }

    pub fn gap(mut self, gap: impl IntoLengthValue) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.gap = gap.into_length_value();
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
        }
        self
    }

    pub fn center(self) -> Self {
        self.justify(Justify::Center).align(Align::Center)
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_x = overflow;
            layout.overflow_y = overflow;
        }
        self
    }

    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_x = overflow;
        }
        self
    }

    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.overflow_y = overflow;
        }
        self
    }

    pub fn scrollbar_style(mut self, style: ScrollbarStyle) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style = style;
        }
        self
    }

    pub fn scrollbar_thumb_color(mut self, color: Color) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.thumb_color = color;
        }
        self
    }

    pub fn scrollbar_track_color(mut self, color: Color) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.track_color = color;
        }
        self
    }

    pub fn scrollbar_hover_thumb_color(mut self, color: Color) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.hover_thumb_color = color;
        }
        self
    }

    pub fn scrollbar_active_thumb_color(mut self, color: Color) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.active_thumb_color = color;
        }
        self
    }

    pub fn scrollbar_thickness(mut self, thickness: Dp) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.thickness = thickness;
        }
        self
    }

    pub fn scrollbar_radius(mut self, radius: Dp) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.radius = radius;
        }
        self
    }

    pub fn scrollbar_insets(mut self, insets: Insets) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.insets = insets;
        }
        self
    }

    pub fn scrollbar_min_thumb_length(mut self, min_thumb_length: Dp) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.min_thumb_length = min_thumb_length;
        }
        self
    }
}

impl<VM> From<Container<VM>> for Element<VM> {
    fn from(value: Container<VM>) -> Self {
        value.element
    }
}

pub struct Stack<VM>(Container<VM>);
pub struct Grid<VM>(Container<VM>);
pub struct Flex<VM>(Container<VM>);

macro_rules! impl_layout_api {
    ($name:ident) => {
        impl<VM> $name<VM> {
            pub fn size(self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_lengths(layout, width, height);
                    },
                )
            }

            pub fn width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.width, width);
                    },
                )
            }

            pub fn height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.height, height);
                    },
                )
            }

            pub fn min_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_width, width);
                    },
                )
            }

            pub fn min_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_height, height);
                    },
                )
            }

            pub fn max_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_width, width);
                    },
                )
            }

            pub fn max_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_height, height);
                    },
                )
            }

            pub fn aspect_ratio(self, aspect_ratio: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.aspect_ratio = Some(aspect_ratio.into());
                    },
                )
            }

            pub fn margin(self, insets: impl Into<Value<Insets>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.margin = insets.into();
                    },
                )
            }

            pub fn grow(self, grow: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.grow = grow.into();
                    },
                )
            }

            pub fn shrink(self, shrink: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.shrink = shrink.into();
                    },
                )
            }

            pub fn basis(self, basis: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.basis = Some(basis.into_length_value());
                    },
                )
            }

            pub fn align_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.align_self = Some(align);
                    },
                )
            }

            pub fn justify_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.justify_self = Some(align);
                    },
                )
            }

            pub fn column(self, start: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_start = Some(start.max(1));
                    },
                )
            }

            pub fn row(self, start: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_start = Some(start.max(1));
                    },
                )
            }

            pub fn column_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_span = span.max(1);
                    },
                )
            }

            pub fn row_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_span = span.max(1);
                    },
                )
            }

            pub fn position_absolute(self) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.position_type = crate::ui::layout::PositionType::Absolute;
                    },
                )
            }

            pub fn left(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.left, value);
                    },
                )
            }

            pub fn top(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.top, value);
                    },
                )
            }

            pub fn right(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.right, value);
                    },
                )
            }

            pub fn bottom(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.bottom, value);
                    },
                )
            }

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

            pub fn background(self, color: impl Into<Value<Color>>) -> Self {
                Self(self.0.background(color))
            }

            pub fn border(
                self,
                width: impl Into<Value<Dp>>,
                color: impl Into<Value<Color>>,
            ) -> Self {
                Self(self.0.border(width, color))
            }

            pub fn border_color(self, color: impl Into<Value<Color>>) -> Self {
                Self(self.0.border_color(color))
            }

            pub fn border_radius(self, radius: impl Into<Value<Dp>>) -> Self {
                Self(self.0.border_radius(radius))
            }

            pub fn border_width(self, width: impl Into<Value<Dp>>) -> Self {
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

            pub fn cursor(self, cursor: impl Into<Value<CursorStyle>>) -> Self {
                Self(self.0.cursor(cursor))
            }

            pub fn child(self, child: impl IntoChildren<VM>) -> Self {
                Self(self.0.child(child))
            }

            pub fn padding(self, padding: impl Into<Value<Insets>>) -> Self {
                Self(self.0.padding(padding))
            }

            pub fn gap(self, gap: impl IntoLengthValue) -> Self {
                Self(self.0.gap(gap))
            }

            pub fn justify(self, justify: Justify) -> Self {
                Self(self.0.justify(justify))
            }

            pub fn align(self, align: Align) -> Self {
                Self(self.0.align(align))
            }

            pub fn center(self) -> Self {
                Self(self.0.center())
            }

            pub fn overflow(self, overflow: Overflow) -> Self {
                Self(self.0.overflow(overflow))
            }

            pub fn overflow_x(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_x(overflow))
            }

            pub fn overflow_y(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_y(overflow))
            }

            pub fn scrollbar_style(self, style: ScrollbarStyle) -> Self {
                Self(self.0.scrollbar_style(style))
            }

            pub fn scrollbar_thumb_color(self, color: Color) -> Self {
                Self(self.0.scrollbar_thumb_color(color))
            }

            pub fn scrollbar_track_color(self, color: Color) -> Self {
                Self(self.0.scrollbar_track_color(color))
            }

            pub fn scrollbar_hover_thumb_color(self, color: Color) -> Self {
                Self(self.0.scrollbar_hover_thumb_color(color))
            }

            pub fn scrollbar_active_thumb_color(self, color: Color) -> Self {
                Self(self.0.scrollbar_active_thumb_color(color))
            }

            pub fn scrollbar_thickness(self, thickness: Dp) -> Self {
                Self(self.0.scrollbar_thickness(thickness))
            }

            pub fn scrollbar_radius(self, radius: Dp) -> Self {
                Self(self.0.scrollbar_radius(radius))
            }

            pub fn scrollbar_insets(self, insets: Insets) -> Self {
                Self(self.0.scrollbar_insets(insets))
            }

            pub fn scrollbar_min_thumb_length(self, min_thumb_length: Dp) -> Self {
                Self(self.0.scrollbar_min_thumb_length(min_thumb_length))
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

impl<VM> Grid<VM> {
    pub fn columns<const N: usize>(columns: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: columns.into_iter().collect(),
                rows: Vec::new(),
            },
            ..ContainerLayout::flow()
        }))
    }

    pub fn rows<const N: usize>(rows: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: Vec::new(),
                rows: rows.into_iter().collect(),
            },
            ..ContainerLayout::flow()
        }))
    }

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
    pub fn new(direction: Axis) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Flex {
                direction,
                wrap: Wrap::NoWrap,
            },
            ..ContainerLayout::flow()
        }))
    }

    pub fn direction(mut self, direction: Axis) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Flex { wrap, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }

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
