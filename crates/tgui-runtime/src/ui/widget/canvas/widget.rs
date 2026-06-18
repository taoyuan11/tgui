use super::*;
use crate::theme::StyleContext;
use crate::ui::widget::style::StyleResolver;
use crate::ui::widget::WidgetKey;

pub struct Canvas<VM> {
    element: Element<VM>,
}

macro_rules! impl_canvas_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.element.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.element.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.element.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.element.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.element.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.element.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.element.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.element.layout.column_start = Some(Value::Static(start.max(1)));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.element.layout.row_start = Some(Value::Static(start.max(1)));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.element.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.element.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            set_layout_inset(&mut self.element.layout.top, value);
            set_layout_inset(&mut self.element.layout.right, value);
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }
    };
}

impl<VM> Canvas<VM> {
    pub fn new(scene: impl IntoCanvasContent) -> Self {
        Self::from_scene(scene.into_canvas_scene())
    }

    pub(crate) fn from_scene(scene: impl Into<Value<CanvasScene>>) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
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
                kind: WidgetKind::Canvas {
                    scene: scene.into(),
                    item_interactions: CanvasItemInteractionHandlers::default(),
                    style: None,
                },
            },
        }
    }

    impl_canvas_layout_api!();

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CanvasStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Canvas { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::mutate(
                |context| CanvasStyle::default_for_theme(context.theme),
                mutator,
            ));
        }
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CanvasStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Canvas { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::full(resolver));
        }
        self
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    pub fn on_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: crate::foundation::view_model::Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    pub fn on_item_click(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_click = Some(command);
        }
        self
    }

    pub fn on_item_double_click(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_double_click = Some(command);
        }
        self
    }

    pub fn on_item_mouse_down(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_down = Some(command);
        }
        self
    }

    pub fn on_item_mouse_up(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_up = Some(command);
        }
        self
    }

    pub fn on_item_mouse_enter(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_enter = Some(command);
        }
        self
    }

    pub fn on_item_mouse_leave(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_leave = Some(command);
        }
        self
    }

    pub fn on_item_mouse_move(mut self, command: ValueCommand<VM, CanvasMouseEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_mouse_move = Some(command);
        }
        self
    }

    pub fn on_item_wheel(mut self, command: ValueCommand<VM, CanvasWheelEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_wheel = Some(command);
        }
        self
    }

    pub fn on_item_drag_start(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag_start = Some(command);
        }
        self
    }

    pub fn on_item_drag(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag = Some(command);
        }
        self
    }

    pub fn on_item_drag_end(mut self, command: ValueCommand<VM, CanvasDragEvent>) -> Self {
        if let WidgetKind::Canvas {
            item_interactions, ..
        } = &mut self.element.kind
        {
            item_interactions.on_drag_end = Some(command);
        }
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Canvas<VM>> for Element<VM> {
    fn from(value: Canvas<VM>) -> Self {
        value.element
    }
}
