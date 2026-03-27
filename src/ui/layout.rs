#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    pub fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

impl Default for Insets {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

impl Default for Align {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
}

impl Default for Justify {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    NoWrap,
    Wrap,
}

impl Default for Wrap {
    fn default() -> Self {
        Self::NoWrap
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutStyle {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fill_width: bool,
    pub fill_height: bool,
    pub padding: Insets,
    pub margin: Insets,
    pub grow: f32,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            fill_width: false,
            fill_height: false,
            padding: Insets::ZERO,
            margin: Insets::ZERO,
            grow: 0.0,
        }
    }
}
