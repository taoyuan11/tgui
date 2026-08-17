//! Backend-independent retained-render contracts.
//!
//! The P0 command subset exists so tree and snapshot behavior can be tested
//! without a GPU. Backend adapters are feature-bound and arrive in P3.

use crate::core::{Clip, Color, Error, Rect, Result, SceneRevision, Transform2D};
use std::sync::Arc;

/// Minimal typed command subset understood by the headless P0 renderer.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PaintCommand {
    Clear(Color),
    FillRect { rect: Rect, color: Color },
    PushClip(Clip),
    PopClip,
    PushTransform(Transform2D),
    PopTransform,
    Marker(Arc<str>),
}

impl PaintCommand {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Clear(_) | Self::PopClip | Self::PopTransform | Self::Marker(_) => Ok(()),
            Self::FillRect { rect, .. } => rect.validate().map_err(Error::from),
            Self::PushClip(clip) => clip.validate().map_err(Error::from),
            Self::PushTransform(transform) => transform.validate().map_err(Error::from),
        }
    }
}

/// Stable, immutable header for observable scene output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneSnapshot {
    revision: SceneRevision,
    command_count: usize,
    fingerprint: u64,
}

impl SceneSnapshot {
    pub const fn new(revision: SceneRevision, command_count: usize, fingerprint: u64) -> Self {
        Self {
            revision,
            command_count,
            fingerprint,
        }
    }

    pub const fn empty(revision: SceneRevision) -> Self {
        Self::new(revision, 0, 0)
    }

    pub const fn revision(&self) -> SceneRevision {
        self.revision
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub(crate) fn observable_eq(&self, other: &Self) -> bool {
        self.command_count == other.command_count && self.fingerprint == other.fingerprint
    }
}
