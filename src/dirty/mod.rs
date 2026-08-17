//! Crate-private invalidation-index boundary.
//!
//! Dirty state is an index over the element/render trees, not an independent UI
//! truth. Full propagation is intentionally deferred to P2.
