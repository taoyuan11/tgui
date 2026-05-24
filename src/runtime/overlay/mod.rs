//! Runtime-owned overlay / popup anchoring engine.
//!
//! Widgets only declare overlay requests and content. Runtime owns anchor
//! resolution, placement solving, layer ordering, close handlers, and
//! flattening into `ScenePrimitives`.

mod anchor;
mod close;
pub(crate) mod collect;
mod overlay;
mod placement;
mod solver;

#[cfg(test)]
mod tests;

pub use anchor::{Anchor, AnchorKey, AnchorSource};
pub use placement::{
    Alignment, FlipPolicy, OverlayId, OverlayLayer, Placement, PlacementOptions, Side,
    SolvedPlacement,
};
pub use solver::solve_placement;

pub(crate) use close::OverlayCloseHandle;
pub(crate) use overlay::{Overlay, OverlayBackdrop, OverlayContent, OverlayPrimitive};
