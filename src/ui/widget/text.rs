use crate::foundation::color::Color;
use crate::text::font::FontWeight;
use crate::ui::layout::{Insets, LayoutStyle};

use super::common::{Value, WidgetId, WidgetKind};
use super::core::Element;

#[derive(Clone)]
pub struct Text {
    pub(crate) layout: LayoutStyle,
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
}

impl<VM> From<Text> for Element<VM> {
    fn from(value: Text) -> Self {
        let background = value.background.clone();
        let layout = value.layout;
        Element {
            id: WidgetId::next(),
            layout,
            background,
            kind: WidgetKind::Text { text: value },
        }
    }
}
