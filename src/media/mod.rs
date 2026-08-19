//! Stable media handles, image decoding/cache contracts, and immutable
//! resource-reference snapshots. Decoder crates remain feature-gated while
//! source identity, generations, budgets, and headless caches stay available.

use crate::core::ResourceRevision;
pub use crate::core::{FontHandle, GlyphPageId, ImageHandle, ResourceId};
use std::sync::Arc;

mod image;

pub use image::{
    CpuImageCache, DecodedImage, GpuImageCacheError, GpuTexture, GpuTextureCache, ImageCacheStats,
    ImageCompletion, ImageCompletionBatch, ImageDecodeRequest, ImageDecodeResult, ImageLoadError,
    ImagePayload, ImagePresentation, ImageRegistry, ImageRequest, ImageRequestKey, ImageSize,
    ImageSource, ImageSourceResolver, ImageState, ImageTextureUploader, LocalImageSourceResolver,
    decode_image, spawn_image_decode,
};

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
