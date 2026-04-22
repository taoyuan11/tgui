use std::fmt;

use crate::animation::Transition;
use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::ui::unit::{dp, Dp};

#[derive(Clone)]
pub enum Value<T> {
    Static(T),
    Bound(Binding<T>),
}

impl<T: Clone> Value<T> {
    pub fn resolve(&self) -> T {
        match self {
            Self::Static(value) => value.clone(),
            Self::Bound(binding) => binding.get(),
        }
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        match self {
            Self::Static(_) => None,
            Self::Bound(binding) => binding.transition(),
        }
    }
}

impl<T> From<T> for Value<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

impl<T> From<Binding<T>> for Value<T> {
    fn from(value: Binding<T>) -> Self {
        Self::Bound(value)
    }
}

impl From<&str> for Value<String> {
    fn from(value: &str) -> Self {
        Self::Static(value.to_string())
    }
}

impl<T: fmt::Debug> fmt::Debug for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(value) => f.debug_tuple("Static").field(value).finish(),
            Self::Bound(_) => f.write_str("Bound(..)"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insets {
    pub left: Dp,
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
}

impl Insets {
    pub const ZERO: Self = Self {
        left: Dp::ZERO,
        top: Dp::ZERO,
        right: Dp::ZERO,
        bottom: Dp::ZERO,
    };

    pub fn all(value: Dp) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub fn symmetric(horizontal: Dp, vertical: Dp) -> Self {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    NoWrap,
    Wrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    Visible,
    #[default]
    Hidden,
    Scroll,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub thumb_color: Color,
    pub hover_thumb_color: Color,
    pub active_thumb_color: Color,
    pub track_color: Color,
    pub thickness: Dp,
    pub radius: Dp,
    pub insets: Insets,
    pub min_thumb_length: Dp,
}

impl ScrollbarStyle {
    pub fn thumb_color(mut self, color: Color) -> Self {
        self.thumb_color = color;
        self
    }

    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = color;
        self
    }

    pub fn hover_thumb_color(mut self, color: Color) -> Self {
        self.hover_thumb_color = color;
        self
    }

    pub fn active_thumb_color(mut self, color: Color) -> Self {
        self.active_thumb_color = color;
        self
    }

    pub fn thickness(mut self, thickness: Dp) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn radius(mut self, radius: Dp) -> Self {
        self.radius = radius;
        self
    }

    pub fn insets(mut self, insets: Insets) -> Self {
        self.insets = insets;
        self
    }

    pub fn min_thumb_length(mut self, min_thumb_length: Dp) -> Self {
        self.min_thumb_length = min_thumb_length;
        self
    }
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            thumb_color: Color::hexa(0xFFFFFFFF).with_alpha_factor(0.72),
            hover_thumb_color: Color::hexa(0xFFFFFFFF).with_alpha_factor(0.86),
            active_thumb_color: Color::hexa(0xFFFFFFFF),
            track_color: Color::hexa(0xFFFFFF1F),
            thickness: dp(8.0),
            radius: dp(999.0),
            insets: Insets::all(dp(6.0)),
            min_thumb_length: dp(28.0),
        }
    }
}

#[derive(Clone)]
pub struct LayoutStyle {
    pub width: Option<Value<Dp>>,
    pub height: Option<Value<Dp>>,
    pub fill_width: bool,
    pub fill_height: bool,
    pub padding: Value<Insets>,
    pub margin: Value<Insets>,
    pub grow: Value<f32>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            fill_width: false,
            fill_height: false,
            padding: Value::Static(Insets::ZERO),
            margin: Value::Static(Insets::ZERO),
            grow: Value::Static(0.0),
        }
    }
}
