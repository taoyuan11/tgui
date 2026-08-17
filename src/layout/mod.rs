//! Layout-facing immutable contracts.
//!
//! P0 exposes only a validated snapshot header. Layout engines depend on
//! `core`; concrete Taffy integration arrives in P2 and remains UI-thread owned.

use crate::core::{Error, LayoutRevision, Result, Size};

/// Observable layout output committed for one window.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSnapshot {
    revision: LayoutRevision,
    viewport: Size,
    node_count: usize,
    fingerprint: u64,
}

impl LayoutSnapshot {
    pub fn new(
        revision: LayoutRevision,
        viewport: Size,
        node_count: usize,
        fingerprint: u64,
    ) -> Result<Self> {
        viewport.validate().map_err(Error::from)?;
        Ok(Self {
            revision,
            viewport,
            node_count,
            fingerprint,
        })
    }

    pub const fn empty(revision: LayoutRevision) -> Self {
        Self {
            revision,
            viewport: Size::ZERO,
            node_count: 0,
            fingerprint: 0,
        }
    }

    pub const fn revision(&self) -> LayoutRevision {
        self.revision
    }

    pub const fn viewport(&self) -> Size {
        self.viewport
    }

    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && self.node_count == other.node_count
            && self.fingerprint == other.fingerprint
    }
}

impl Default for LayoutSnapshot {
    fn default() -> Self {
        Self::empty(LayoutRevision::ZERO)
    }
}
