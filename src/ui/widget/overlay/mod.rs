//! Compatibility re-export layer for the runtime-owned overlay engine.

pub use crate::runtime::overlay::{
    solve_placement, Alignment, Anchor, AnchorKey, AnchorSource, FlipPolicy, OverlayId,
    OverlayLayer, Placement, PlacementOptions, Side, SolvedPlacement,
};

pub(crate) mod collect {
    pub(crate) use crate::runtime::overlay::collect::emit_overlay;
}

pub(crate) use crate::runtime::overlay::{Overlay, OverlayContent, OverlayPrimitive};
