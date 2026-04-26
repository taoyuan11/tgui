use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{dp, Dp};

use super::background::{BackgroundBrush, BackgroundImage};
use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::text::Text;

pub struct Input<VM> {
    element: Element<VM>,
}

macro_rules! impl_input_layout_api {
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
            self.element.layout.padding = insets.into();
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

impl<VM> Input<VM> {
    pub fn new(text: Text) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle {
                    padding: Value::Static(Insets::symmetric(dp(12.0), dp(8.0))),
                    ..LayoutStyle::default()
                },
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                kind: WidgetKind::Input {
                    text,
                    placeholder: Text::new(String::new()),
                    on_change: None,
                    disabled: Value::Static(false),
                },
            },
        }
    }

    impl_input_layout_api!();

    pub fn placeholder_with_text(mut self, placeholder: Text) -> Self {
        if let WidgetKind::Input {
            placeholder: value, ..
        } = &mut self.element.kind
        {
            *value = placeholder;
        }
        self
    }

    pub fn placeholder_with_str(mut self, placeholder: &str) -> Self {
        let text = Text::new(placeholder.to_string()).into();

        if let WidgetKind::Input {
            placeholder: value, ..
        } = &mut self.element.kind
        {
            *value = text;
        }
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn background_brush(mut self, brush: impl Into<Value<BackgroundBrush>>) -> Self {
        self.element.visual.background_brush = Some(brush.into());
        self
    }

    pub fn background_image(mut self, image: impl Into<Value<BackgroundImage>>) -> Self {
        self.element.visual.background_image = Some(image.into());
        self
    }

    pub fn background_blur(mut self, blur: impl Into<Value<Dp>>) -> Self {
        self.element.visual.background_blur = blur.into();
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

    pub fn on_change(mut self, command: ValueCommand<VM, String>) -> Self {
        if let WidgetKind::Input { on_change, .. } = &mut self.element.kind {
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

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        if let WidgetKind::Input { disabled, .. } = &mut self.element.kind {
            *disabled = disable.into();
        }
        self
    }

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
