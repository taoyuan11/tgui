//! Logical-pixel Taffy layout, intrinsic measurement, and immutable snapshots.
//!
//! Application-facing code configures [`LayoutStyle`] and [`MeasureSpec`] on a
//! Widget declaration. The Taffy node map, cache, and incremental synchronizer
//! remain UI-thread-owned and crate-private. DPI participates in intrinsic
//! cache keys but never scales the logical geometry stored in a snapshot.

mod engine;
mod measure;
mod snapshot;
mod style;

pub use engine::LayoutPassReport;
pub use measure::{
    AvailableDimension, AvailableSize, KnownDimensions, Measure, MeasureCacheStats, MeasureHandle,
    MeasureInput, MeasureKind, MeasureOutput, MeasureSpec,
};
pub use snapshot::{LayoutNode, LayoutSnapshot, compare_layout_snapshots};
pub use style::{
    AlignContent, AlignItems, Dimension, Display, FlexDirection, FlexWrap, GridAxisPlacement,
    GridPlacement, GridTrack, LayoutBoundaries, LayoutSize, LayoutStyle, LengthPercentage,
    LengthPercentageAuto, Overflow, OverflowAxes, Position, Sides,
};

pub(crate) use engine::{LayoutEngine, LayoutNodeInput};
pub(crate) use measure::MeasureCache;
