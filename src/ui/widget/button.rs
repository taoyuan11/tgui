use crate::foundation::color::Color;
use crate::foundation::view_model::Command;
use crate::ui::layout::{Insets, LayoutStyle};

use super::common::{Value, WidgetId, WidgetKind};
use super::core::Element;
use super::text::Text;

pub struct Button<VM> {
    element: Element<VM>,
}

impl<VM> Button<VM> {
    pub fn new(label: Text) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle {
                    padding: Insets::symmetric(12.0, 8.0),
                    ..LayoutStyle::default()
                },
                background: None,
                kind: WidgetKind::Button {
                    label,
                    on_click: None,
                },
            },
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.height = Some(height);
        self.element.layout.fill_width = false;
        self.element.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.element.layout.height = Some(height);
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

    pub fn margin(mut self, insets: Insets) -> Self {
        self.element.layout.margin = insets;
        self
    }

    pub fn padding(mut self, insets: Insets) -> Self {
        self.element.layout.padding = insets;
        self
    }

    pub fn grow(mut self, grow: f32) -> Self {
        self.element.layout.grow = grow;
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        if let WidgetKind::Button { on_click, .. } = &mut self.element.kind {
            *on_click = Some(command);
        }
        self
    }
}

impl<VM> From<Button<VM>> for Element<VM> {
    fn from(value: Button<VM>) -> Self {
        value.element
    }
}
