//! Accessibility semantics contract.
//!
//! The immutable semantic header is always present for atomic commits. Platform
//! adapters are compiled only through the `accessibility` feature in P6.

use crate::core::SemanticRevision;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticSnapshot {
    revision: SemanticRevision,
    node_count: usize,
    fingerprint: u64,
}

impl SemanticSnapshot {
    pub const fn new(revision: SemanticRevision, node_count: usize, fingerprint: u64) -> Self {
        Self {
            revision,
            node_count,
            fingerprint,
        }
    }

    pub const fn empty(revision: SemanticRevision) -> Self {
        Self::new(revision, 0, 0)
    }

    pub const fn revision(&self) -> SemanticRevision {
        self.revision
    }

    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.node_count == other.node_count && self.fingerprint == other.fingerprint
    }
}

pub const ADAPTER_ENABLED: bool = cfg!(feature = "accessibility");
