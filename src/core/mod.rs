//! Dependency-free foundation shared by every subsystem.
//!
//! This module is public because identifiers, geometry, errors, and revisions
//! cross application-facing contracts. Arena mutation and tree topology remain
//! UI-thread-owned by convention and by the non-`Send` application façade.

mod arena;
mod error;
mod geometry;
mod id;
mod invariants;
mod key;
mod revision;
mod tree;

pub use arena::{ArenaIter, ArenaIterMut, ArenaStats, DenseArena, ElementArena};
pub use error::{
    CompileError, Degradation, Error, ErrorKind, InputError, PlatformError, ResourceError, Result,
};
pub use geometry::{
    Clip, Color, CornerRadii, DpiScale, GeometryError, Point, Rect, Size, Transform2D,
};
pub use id::{
    AnimationId, ElementId, FontHandle, GenerationStamp, GenerationalId, GlyphPageId, ImageHandle,
    RenderNodeId, ResourceId, WindowHandle, WindowId,
};
pub use invariants::{ARCHITECTURE_INVARIANTS, ArchitectureInvariant};
pub use key::{ItemKey, NodeKey, PropertyId, WidgetKey};
pub use revision::{
    LayoutRevision, ResourceRevision, RevisionChanges, RevisionError, RevisionSet, RevisionValue,
    SceneRevision, SemanticRevision,
};
pub use tree::{TreeLinks, TreeNode};

/// Backwards-neutral short name for the P0 dense generational arena.
pub type Arena<T, I = ElementId> = DenseArena<T, I>;
