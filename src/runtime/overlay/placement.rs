use crate::ui::unit::{dp, Dp};
use crate::ui::widget::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    pub fn is_vertical(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub side: Side,
    pub alignment: Alignment,
}

impl Placement {
    pub const fn new(side: Side, alignment: Alignment) -> Self {
        Self { side, alignment }
    }

    pub const fn top() -> Self {
        Self::new(Side::Top, Alignment::Center)
    }

    pub const fn bottom() -> Self {
        Self::new(Side::Bottom, Alignment::Center)
    }

    pub const fn left() -> Self {
        Self::new(Side::Left, Alignment::Center)
    }

    pub const fn right() -> Self {
        Self::new(Side::Right, Alignment::Center)
    }

    pub const fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self::bottom()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlipPolicy {
    None,
    ShiftOnly,
    #[default]
    FlipSide,
    FlipAndShift,
    Hide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacementOptions {
    pub placement: Placement,
    pub offset: Dp,
    pub cross_offset: Dp,
    pub flip: FlipPolicy,
    pub viewport_padding: Dp,
    pub clamp_to_viewport: bool,
    pub match_anchor_width: bool,
}

impl Default for PlacementOptions {
    fn default() -> Self {
        Self {
            placement: Placement::bottom(),
            offset: dp(8.0),
            cross_offset: Dp::ZERO,
            flip: FlipPolicy::FlipSide,
            viewport_padding: dp(8.0),
            clamp_to_viewport: true,
            match_anchor_width: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedPlacement {
    pub rect: Rect,
    pub resolved_placement: Placement,
    pub clip_rect: Rect,
    pub did_flip: bool,
    pub was_hidden: bool,
    pub anchor_rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum OverlayLayer {
    #[default]
    Tooltip,
    Popover,
    Menu,
    Modal,
    Toast,
}

impl OverlayLayer {
    pub const ALL: [OverlayLayer; 5] = [
        OverlayLayer::Tooltip,
        OverlayLayer::Popover,
        OverlayLayer::Menu,
        OverlayLayer::Modal,
        OverlayLayer::Toast,
    ];

    pub(crate) const fn index(self) -> usize {
        match self {
            OverlayLayer::Tooltip => 0,
            OverlayLayer::Popover => 1,
            OverlayLayer::Menu => 2,
            OverlayLayer::Modal => 3,
            OverlayLayer::Toast => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(pub u64);

impl OverlayId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}
