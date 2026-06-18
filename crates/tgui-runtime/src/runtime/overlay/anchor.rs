use crate::ui::widget::{Point, Rect, WidgetId};

/// Public-facing anchor description for overlays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnchorSource {
    Widget(WidgetId),
    Caret(WidgetId),
    Point,
}

/// Stable key used by runtime to resolve overlay anchors from collect output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnchorKey {
    source: AnchorSource,
}

impl AnchorKey {
    pub const fn widget(widget_id: WidgetId) -> Self {
        Self {
            source: AnchorSource::Widget(widget_id),
        }
    }

    pub const fn caret(widget_id: WidgetId) -> Self {
        Self {
            source: AnchorSource::Caret(widget_id),
        }
    }

    pub const fn point() -> Self {
        Self {
            source: AnchorSource::Point,
        }
    }

    pub const fn source(self) -> AnchorSource {
        self.source
    }
}

/// Runtime anchor reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Key(AnchorKey),
    Rect(Rect),
    Point(Point),
}

impl Anchor {
    pub fn to_rect_with(self, resolve: impl FnOnce(AnchorKey) -> Option<Rect>) -> Option<Rect> {
        match self {
            Anchor::Key(key) => resolve(key),
            Anchor::Rect(rect) => Some(rect),
            Anchor::Point(point) => Some(Rect::new(point.x, point.y, 0.0, 0.0)),
        }
    }

    pub fn to_rect(self) -> Rect {
        match self {
            Anchor::Key(_) => Rect::new(0.0, 0.0, 0.0, 0.0),
            Anchor::Rect(rect) => rect,
            Anchor::Point(point) => Rect::new(point.x, point.y, 0.0, 0.0),
        }
    }
}

impl From<Rect> for Anchor {
    fn from(rect: Rect) -> Self {
        Anchor::Rect(rect)
    }
}

impl From<Point> for Anchor {
    fn from(point: Point) -> Self {
        Anchor::Point(point)
    }
}
