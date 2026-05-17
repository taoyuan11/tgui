use lyon::tessellation::StrokeOptions;

use super::*;

mod builder;
mod item;

pub(crate) use self::builder::{PathCommand, PathShapeHint};
pub use self::builder::PathBuilder;
pub use self::item::CanvasPath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasPathOpError {
    OpenSubpath,
}

impl std::fmt::Display for CanvasPathOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenSubpath => write!(f, "canvas path operations require closed subpaths"),
        }
    }
}

impl std::error::Error for CanvasPathOpError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSvgPathError(pub String);

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasStroke {
    pub width: Dp,
    pub brush: Value<CanvasBrush>,
    pub dash_pattern: Option<Vec<Dp>>,
    pub dash_offset: Dp,
    pub line_cap: CanvasStrokeCap,
    pub line_join: CanvasStrokeJoin,
    pub miter_limit: f32,
    pub alignment: CanvasStrokeAlignment,
}

impl CanvasStroke {
    pub fn new(width: impl Into<Dp>, color: Color) -> Self {
        Self::with_brush(width, CanvasBrush::Solid(color))
    }

    pub fn with_brush(width: impl Into<Dp>, brush: impl Into<Value<CanvasBrush>>) -> Self {
        Self {
            width: width.into(),
            brush: brush.into(),
            dash_pattern: None,
            dash_offset: Dp::ZERO,
            line_cap: CanvasStrokeCap::Butt,
            line_join: CanvasStrokeJoin::Miter,
            miter_limit: StrokeOptions::DEFAULT_MITER_LIMIT,
            alignment: CanvasStrokeAlignment::Center,
        }
    }

    pub fn dash<I, T>(mut self, pattern: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Dp>,
    {
        self.dash_pattern = Some(pattern.into_iter().map(Into::into).collect());
        self
    }

    pub fn dash_offset(mut self, offset: impl Into<Dp>) -> Self {
        self.dash_offset = offset.into();
        self
    }

    pub fn line_cap(mut self, line_cap: CanvasStrokeCap) -> Self {
        self.line_cap = line_cap;
        self
    }

    pub fn line_join(mut self, line_join: CanvasStrokeJoin) -> Self {
        self.line_join = line_join;
        self
    }

    pub fn miter_limit(mut self, miter_limit: f32) -> Self {
        self.miter_limit = miter_limit.max(0.0);
        self
    }

    pub fn alignment(mut self, alignment: CanvasStrokeAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}
