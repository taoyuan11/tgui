use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{Insets, LayoutStyle};

use super::common::{InteractionHandlers, Point, Value, VisualStyle, WidgetId, WidgetKind};
use super::core::Element;

#[derive(Clone)]
pub struct Text {
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) content: Value<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) color: Option<Value<Color>>,
    pub(crate) font_size: Option<f32>,
    pub(crate) font_weight: FontWeight,
    pub(crate) letter_spacing: f32,
}

impl Text {
    pub fn new(content: impl Into<Value<String>>) -> Self {
        Self {
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            content: content.into(),
            font_family: None,
            background: None,
            color: None,
            font_size: None,
            font_weight: FontWeight::NORMAL,
            letter_spacing: 0.0,
        }
    }

    pub fn margin(mut self, insets: Insets) -> Self {
        self.layout.margin = insets;
        self
    }

    pub fn padding(mut self, insets: Insets) -> Self {
        self.layout.padding = insets;
        self
    }

    pub fn font(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = Some(font_family.into());
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn border(
        mut self,
        width: impl Into<Value<f32>>,
        color: impl Into<Value<Color>>,
    ) -> Self {
        self.visual.border_width = width.into();
        self.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<f32>>) -> Self {
        self.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<f32>>) -> Self {
        self.visual.border_width = width.into();
        self
    }

    pub fn color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size.max(1.0));
        self
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.visual.offset = offset.into();
        self
    }

    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    fn into_element_with_interactions<VM>(
        self,
        interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        let background = self.background.clone();
        let layout = self.layout;
        let visual = self.visual.clone();
        Element {
            id: WidgetId::next(),
            layout,
            visual,
            interactions,
            background,
            kind: WidgetKind::Text { text: self },
        }
    }
}

impl<VM> From<Text> for Element<VM> {
    fn from(value: Text) -> Self {
        let background = value.background.clone();
        let layout = value.layout;
        let visual = value.visual.clone();
        Element {
            id: WidgetId::next(),
            layout,
            visual,
            interactions: InteractionHandlers::default(),
            background,
            kind: WidgetKind::Text { text: value },
        }
    }
}
