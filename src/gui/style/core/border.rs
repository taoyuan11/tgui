use crate::gui::style::core::color::Color;

#[derive(Debug, Clone, Copy)]
pub struct BorderItem {
    pub value: i32,
    pub color: Color,
}

impl BorderItem {
    pub fn build() -> Self {
        Self {
            value: 0,
            color: Color::from_rgb(0, 0, 0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Border {
    pub left: BorderItem,
    pub right: BorderItem,
    pub top: BorderItem,
    pub bottom: BorderItem,
    pub color: Color,
}

impl Border {
    pub fn build() -> Self {
        Self {
            left: BorderItem::build(),
            right: BorderItem::build(),
            top: BorderItem::build(),
            bottom: BorderItem::build(),
            color: Color::from_rgb(0, 0, 0),
        }
    }
}