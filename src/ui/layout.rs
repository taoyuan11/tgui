use std::fmt;

use crate::animation::Transition;
use crate::foundation::binding::Binding;

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

#[derive(Clone)]
pub struct LayoutStyle {
    pub width: Option<Value<f32>>,
    pub height: Option<Value<f32>>,
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
