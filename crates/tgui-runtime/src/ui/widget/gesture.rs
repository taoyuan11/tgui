use crate::foundation::view_model::ValueCommand;
use crate::platform::event::FingerId;
use crate::ui::unit::Dp;
use crate::ui::widget::{Point, WidgetId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GestureSource {
    Mouse,
    Touch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GesturePhase {
    Start,
    Update,
    End,
    Cancel,
    Recognized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwipeAxis {
    Horizontal,
    Vertical,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GestureEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GestureEdgeSet(u8);

impl GestureEdgeSet {
    const LEFT: u8 = 1 << 0;
    const RIGHT: u8 = 1 << 1;
    const TOP: u8 = 1 << 2;
    const BOTTOM: u8 = 1 << 3;

    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::LEFT | Self::RIGHT | Self::TOP | Self::BOTTOM)
    }

    pub const fn horizontal() -> Self {
        Self(Self::LEFT | Self::RIGHT)
    }

    pub const fn vertical() -> Self {
        Self(Self::TOP | Self::BOTTOM)
    }

    pub const fn with(mut self, edge: GestureEdge) -> Self {
        self.0 |= Self::bit(edge);
        self
    }

    pub const fn contains(self, edge: GestureEdge) -> bool {
        self.0 & Self::bit(edge) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    const fn bit(edge: GestureEdge) -> u8 {
        match edge {
            GestureEdge::Left => Self::LEFT,
            GestureEdge::Right => Self::RIGHT,
            GestureEdge::Top => Self::TOP,
            GestureEdge::Bottom => Self::BOTTOM,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressEvent {
    pub widget_id: WidgetId,
    pub source: GestureSource,
    pub phase: GesturePhase,
    pub start_position: Point,
    pub position: Point,
    pub finger_id: Option<FingerId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoubleTapEvent {
    pub widget_id: WidgetId,
    pub source: GestureSource,
    pub phase: GesturePhase,
    pub position: Point,
    pub finger_id: Option<FingerId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwipeGestureEvent {
    pub widget_id: WidgetId,
    pub source: GestureSource,
    pub phase: GesturePhase,
    pub axis: SwipeAxis,
    pub direction: SwipeDirection,
    pub start_position: Point,
    pub position: Point,
    pub delta: Point,
    pub finger_id: Option<FingerId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeSwipeEvent {
    pub widget_id: WidgetId,
    pub source: GestureSource,
    pub phase: GesturePhase,
    pub edge: GestureEdge,
    pub direction: SwipeDirection,
    pub start_position: Point,
    pub position: Point,
    pub delta: Point,
    pub finger_id: Option<FingerId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinchGestureEvent {
    pub widget_id: WidgetId,
    pub source: GestureSource,
    pub phase: GesturePhase,
    pub start_center: Point,
    pub center: Point,
    pub start_distance: Dp,
    pub distance: Dp,
    pub scale: f32,
    pub delta_scale: f32,
    pub finger_ids: [FingerId; 2],
}

pub struct GestureRecognizer<VM> {
    pub(crate) on_long_press: Option<ValueCommand<VM, LongPressEvent>>,
    pub(crate) on_double_tap: Option<ValueCommand<VM, DoubleTapEvent>>,
    pub(crate) on_swipe: Option<(SwipeAxis, ValueCommand<VM, SwipeGestureEvent>)>,
    pub(crate) on_edge_swipe: Option<(GestureEdgeSet, ValueCommand<VM, EdgeSwipeEvent>)>,
    pub(crate) on_pinch: Option<ValueCommand<VM, PinchGestureEvent>>,
}

impl<VM> GestureRecognizer<VM> {
    pub fn new() -> Self {
        Self {
            on_long_press: None,
            on_double_tap: None,
            on_swipe: None,
            on_edge_swipe: None,
            on_pinch: None,
        }
    }

    pub fn on_long_press(mut self, command: ValueCommand<VM, LongPressEvent>) -> Self {
        self.on_long_press = Some(command);
        self
    }

    pub fn on_double_tap(mut self, command: ValueCommand<VM, DoubleTapEvent>) -> Self {
        self.on_double_tap = Some(command);
        self
    }

    pub fn on_swipe(
        mut self,
        axis: SwipeAxis,
        command: ValueCommand<VM, SwipeGestureEvent>,
    ) -> Self {
        self.on_swipe = Some((axis, command));
        self
    }

    pub fn on_edge_swipe(
        mut self,
        edges: GestureEdgeSet,
        command: ValueCommand<VM, EdgeSwipeEvent>,
    ) -> Self {
        self.on_edge_swipe = Some((edges, command));
        self
    }

    pub fn on_pinch(mut self, command: ValueCommand<VM, PinchGestureEvent>) -> Self {
        self.on_pinch = Some(command);
        self
    }
}

impl<VM> Clone for GestureRecognizer<VM> {
    fn clone(&self) -> Self {
        Self {
            on_long_press: self.on_long_press.clone(),
            on_double_tap: self.on_double_tap.clone(),
            on_swipe: self.on_swipe.clone(),
            on_edge_swipe: self.on_edge_swipe.clone(),
            on_pinch: self.on_pinch.clone(),
        }
    }
}

impl<VM> Default for GestureRecognizer<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> GestureRecognizer<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_long_press.is_some()
            || self.on_double_tap.is_some()
            || self.on_swipe.is_some()
            || self.on_edge_swipe.is_some()
            || self.on_pinch.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: std::sync::Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> GestureRecognizer<RootVm>
    where
        VM: 'static,
    {
        GestureRecognizer {
            on_long_press: self
                .on_long_press
                .map(|command| command.scope(selector.clone())),
            on_double_tap: self
                .on_double_tap
                .map(|command| command.scope(selector.clone())),
            on_swipe: self
                .on_swipe
                .map(|(axis, command)| (axis, command.scope(selector.clone()))),
            on_edge_swipe: self
                .on_edge_swipe
                .map(|(edges, command)| (edges, command.scope(selector.clone()))),
            on_pinch: self.on_pinch.map(|command| command.scope(selector)),
        }
    }
}
