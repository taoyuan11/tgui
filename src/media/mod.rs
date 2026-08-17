//! Stable media/resource handles and immutable resource-reference snapshots.
//!
//! This module depends only on `core`. Decoders, SVG rasterizers, and GPU caches
//! are optional later-phase adapters and never enter the P0 minimal path.

use crate::core::ResourceRevision;
pub use crate::core::{FontHandle, GlyphPageId, ImageHandle, ResourceId};
use std::sync::Arc;

/// Resource handles retained by a committed CPU frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceSnapshot {
    revision: ResourceRevision,
    references: Arc<[ResourceId]>,
    fingerprint: u64,
}

impl ResourceSnapshot {
    pub fn new(
        revision: ResourceRevision,
        references: impl IntoIterator<Item = ResourceId>,
        fingerprint: u64,
    ) -> Self {
        let mut references = references.into_iter().collect::<Vec<_>>();
        references.sort_unstable();
        references.dedup();
        Self {
            revision,
            references: references.into(),
            fingerprint,
        }
    }

    pub fn empty(revision: ResourceRevision) -> Self {
        Self::new(revision, [], 0)
    }

    pub const fn revision(&self) -> ResourceRevision {
        self.revision
    }

    pub fn references(&self) -> &[ResourceId] {
        &self.references
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.references == other.references && self.fingerprint == other.fingerprint
    }
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self::empty(ResourceRevision::ZERO)
    }
}

pub const IMAGE_BACKEND_ENABLED: bool = cfg!(feature = "image");
pub const SVG_BACKEND_ENABLED: bool = cfg!(feature = "svg");
