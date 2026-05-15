use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    SliderValueFormatter, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::{SliderStyle, StyleResolver};

pub struct Slider<VM> {
    element: Element<VM>,
}

macro_rules! impl_widget_layout_api {
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
            self.element.layout.column_start = Some(start.max(1));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.element.layout.row_start = Some(start.max(1));
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

impl<VM> Slider<VM> {
    pub fn new(value: impl Into<Value<f32>>, min: f32, max: f32) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let mut interactions = InteractionHandlers::default();
        interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                visual: VisualStyle::default(),
                interactions,
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                kind: WidgetKind::Slider {
                    value: value.into(),
                    min,
                    max,
                    step: 1.0,
                    show_ticks: false,
                    show_value_label: false,
                    tick_count: None,
                    value_formatter: None,
                    on_change: None,
                    disabled: Value::Static(false),
                    style: None,
                },
            },
        }
    }

    impl_widget_layout_api!();

    pub fn key(mut self, key: impl Into<super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        if let WidgetKind::Slider { step: target, .. } = &mut self.element.kind {
            *target = step;
        }
        self
    }

    pub fn show_ticks(mut self, show_ticks: bool) -> Self {
        if let WidgetKind::Slider {
            show_ticks: target, ..
        } = &mut self.element.kind
        {
            *target = show_ticks;
        }
        self
    }

    pub fn show_value_label(mut self, show_value_label: bool) -> Self {
        if let WidgetKind::Slider {
            show_value_label: target,
            ..
        } = &mut self.element.kind
        {
            *target = show_value_label;
        }
        self
    }

    pub fn tick_count(mut self, tick_count: usize) -> Self {
        if let WidgetKind::Slider {
            tick_count: target, ..
        } = &mut self.element.kind
        {
            *target = Some(tick_count.max(2));
        }
        self
    }

    pub fn format_value(
        mut self,
        formatter: impl Fn(f32) -> String + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Slider {
            value_formatter, ..
        } = &mut self.element.kind
        {
            *value_formatter = Some(SliderValueFormatter::new(formatter));
        }
        self
    }

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> SliderStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Slider { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::new(resolver));
        }
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, f32>) -> Self {
        if let WidgetKind::Slider { on_change, .. } = &mut self.element.kind {
            *on_change = Some(command);
        }
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

    pub fn on_focus(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_focus = Some(command);
        self
    }

    pub fn on_blur(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_blur = Some(command);
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

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        if let WidgetKind::Slider { disabled, .. } = &mut self.element.kind {
            *disabled = disable.into();
        }
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM> From<Slider<VM>> for Element<VM> {
    fn from(value: Slider<VM>) -> Self {
        value.element
    }
}
