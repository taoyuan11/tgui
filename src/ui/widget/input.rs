use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{Insets, LayoutStyle, Value};

use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::core::Element;
use super::text::Text;

pub struct Input<VM> {
    element: Element<VM>,
}

impl<VM> Input<VM> {
    pub fn new(text: Text) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle {
                    padding: Value::Static(Insets::symmetric(12.0, 8.0)),
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

    pub fn size(mut self, width: impl Into<Value<f32>>, height: impl Into<Value<f32>>) -> Self {
        self.element.layout.width = Some(width.into());
        self.element.layout.height = Some(height.into());
        self.element.layout.fill_width = false;
        self.element.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: impl Into<Value<f32>>) -> Self {
        self.element.layout.width = Some(width.into());
        self.element.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: impl Into<Value<f32>>) -> Self {
        self.element.layout.height = Some(height.into());
        self.element.layout.fill_height = false;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.width = None;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.element.layout.fill_height = true;
        self.element.layout.height = None;
        self
    }

    pub fn fill_size(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.fill_height = true;
        self.element.layout.width = None;
        self.element.layout.height = None;
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
