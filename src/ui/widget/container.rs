use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Overflow, ScrollbarStyle, Value, Wrap,
};

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

    pub fn border(mut self, width: impl Into<Value<f32>>, color: impl Into<Value<Color>>) -> Self {
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

    pub fn gap(mut self, gap: impl Into<Value<f32>>) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.gap = gap.into();
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

    pub fn scrollbar_thickness(mut self, thickness: f32) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.thickness = thickness;
        }
        self
    }

    pub fn scrollbar_radius(mut self, radius: f32) -> Self {
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

    pub fn scrollbar_min_thumb_length(mut self, min_thumb_length: f32) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.scrollbar_style.min_thumb_length = min_thumb_length;
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
            pub fn size(
                mut self,
                width: impl Into<Value<f32>>,
                height: impl Into<Value<f32>>,
            ) -> Self {
                self.0.element.layout.width = Some(width.into());
                self.0.element.layout.height = Some(height.into());
                self.0.element.layout.fill_width = false;
                self.0.element.layout.fill_height = false;
                self
            }

            pub fn width(mut self, width: impl Into<Value<f32>>) -> Self {
                self.0.element.layout.width = Some(width.into());
                self.0.element.layout.fill_width = false;
                self
            }

            pub fn height(mut self, height: impl Into<Value<f32>>) -> Self {
                self.0.element.layout.height = Some(height.into());
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

            pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
                self.0.element.layout.margin = insets.into();
                self
            }

            pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
                self.0.element.layout.grow = grow.into();
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

            pub fn cursor(self, cursor: impl Into<Value<CursorStyle>>) -> Self {
                Self(self.0.cursor(cursor))
            }

            pub fn child(self, child: impl IntoChildren<VM>) -> Self {
                Self(self.0.child(child))
            }

            pub fn padding(self, padding: impl Into<Value<Insets>>) -> Self {
                Self(self.0.padding(padding))
            }

            pub fn gap(self, gap: impl Into<Value<f32>>) -> Self {
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

            pub fn scrollbar_thickness(self, thickness: f32) -> Self {
                Self(self.0.scrollbar_thickness(thickness))
            }

            pub fn scrollbar_radius(self, radius: f32) -> Self {
                Self(self.0.scrollbar_radius(radius))
            }

            pub fn scrollbar_insets(self, insets: Insets) -> Self {
                Self(self.0.scrollbar_insets(insets))
            }

            pub fn scrollbar_min_thumb_length(self, min_thumb_length: f32) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::InvalidationSignal;
    use crate::foundation::binding::ViewModelContext;
    use crate::Text;

    fn child_sources(container: &Container<()>) -> &Vec<ChildSource<()>> {
        let WidgetKind::Container { children, .. } = &container.element.kind else {
            panic!("expected container widget");
        };
        children
    }

    fn resolved_child_count(container: &Container<()>) -> usize {
        child_sources(container)
            .iter()
            .map(|child| child.resolve().len())
            .sum()
    }

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    #[test]
    fn container_overflow_defaults_to_hidden() {
        let container = Container::<()>::new();
        let WidgetKind::Container { layout, .. } = &container.element.kind else {
            panic!("expected container widget");
        };

        assert_eq!(layout.overflow_x, Overflow::Hidden);
        assert_eq!(layout.overflow_y, Overflow::Hidden);
    }

    #[test]
    fn overflow_helpers_update_expected_axes() {
        let container = Container::<()>::new()
            .overflow_x(Overflow::Scroll)
            .overflow_y(Overflow::Visible)
            .overflow(Overflow::Hidden);
        let WidgetKind::Container { layout, .. } = &container.element.kind else {
            panic!("expected container widget");
        };

        assert_eq!(layout.overflow_x, Overflow::Hidden);
        assert_eq!(layout.overflow_y, Overflow::Hidden);

        let container = Container::<()>::new()
            .overflow_x(Overflow::Scroll)
            .overflow_y(Overflow::Visible);
        let WidgetKind::Container { layout, .. } = &container.element.kind else {
            panic!("expected container widget");
        };

        assert_eq!(layout.overflow_x, Overflow::Scroll);
        assert_eq!(layout.overflow_y, Overflow::Visible);
    }

    #[test]
    fn scrollbar_style_helpers_update_layout_style() {
        let container = Container::<()>::new()
            .scrollbar_thickness(14.0)
            .scrollbar_radius(6.0)
            .scrollbar_insets(Insets::symmetric(3.0, 5.0))
            .scrollbar_min_thumb_length(40.0)
            .scrollbar_thumb_color(Color::BLACK)
            .scrollbar_track_color(Color::WHITE);
        let WidgetKind::Container { layout, .. } = &container.element.kind else {
            panic!("expected container widget");
        };

        assert_eq!(layout.scrollbar_style.thickness, 14.0);
        assert_eq!(layout.scrollbar_style.radius, 6.0);
        assert_eq!(layout.scrollbar_style.insets, Insets::symmetric(3.0, 5.0));
        assert_eq!(layout.scrollbar_style.min_thumb_length, 40.0);
        assert_eq!(layout.scrollbar_style.thumb_color, Color::BLACK);
        assert_eq!(layout.scrollbar_style.track_color, Color::WHITE);

        let container = Container::<()>::new()
            .scrollbar_hover_thumb_color(Color::hexa(0x11223344))
            .scrollbar_active_thumb_color(Color::hexa(0x55667788));
        let WidgetKind::Container { layout, .. } = &container.element.kind else {
            panic!("expected container widget");
        };

        assert_eq!(
            layout.scrollbar_style.hover_thumb_color,
            Color::hexa(0x11223344)
        );
        assert_eq!(
            layout.scrollbar_style.active_thumb_color,
            Color::hexa(0x55667788)
        );
    }

    #[test]
    fn cursor_helper_sets_cursor_style() {
        let container = Container::<()>::new().cursor(CursorStyle::Pointer);
        assert_eq!(
            container
                .element
                .interactions
                .cursor_style
                .map(|style| style.resolve()),
            Some(CursorStyle::Pointer)
        );
    }

    #[test]
    fn child_accepts_empty_array() {
        let empty: [Element<()>; 0] = [];
        let container = Container::<()>::new().child(empty);

        assert_eq!(child_sources(&container).len(), 1);
        assert_eq!(resolved_child_count(&container), 0);
    }

    #[test]
    fn child_accepts_single_and_multiple_arrays() {
        let single = Container::<()>::new().child([Element::from(Text::new("one"))]);
        assert_eq!(child_sources(&single).len(), 1);
        assert_eq!(resolved_child_count(&single), 1);

        let multiple = Container::<()>::new().child([
            Element::from(Text::new("one")),
            Element::from(Text::new("two")),
        ]);
        assert_eq!(child_sources(&multiple).len(), 1);
        assert_eq!(resolved_child_count(&multiple), 2);
    }

    #[test]
    fn child_accepts_vec_groups() {
        let container = Container::<()>::new().child(vec![
            Element::from(Text::new("one")),
            Element::from(Text::new("two")),
        ]);

        assert_eq!(child_sources(&container).len(), 1);
        assert_eq!(resolved_child_count(&container), 2);
    }

    #[test]
    fn child_accepts_binding_for_single_and_multiple_children() {
        let context = test_context();
        let enabled = context.observable(false);

        let single = Container::<()>::new().child(enabled.binding().map(|value| {
            if value {
                Text::new("enabled")
            } else {
                Text::new("disabled")
            }
        }));
        assert_eq!(resolved_child_count(&single), 1);
        enabled.set(true);
        assert_eq!(resolved_child_count(&single), 1);

        let multiple = Container::<()>::new().child(enabled.binding().map(|value| {
            if value {
                vec![Text::new("a").into(), Text::new("b").into()]
            } else {
                Vec::<Element<()>>::new()
            }
        }));
        assert_eq!(resolved_child_count(&multiple), 2);
        enabled.set(false);
        assert_eq!(resolved_child_count(&multiple), 0);
    }
}
