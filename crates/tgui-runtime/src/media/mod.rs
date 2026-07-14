mod loader;
mod manager;
mod raster;
mod svg;
#[cfg(test)]
mod tests;
mod types;

pub use types::{ContentFit, MediaBytes, MediaPlaybackSource, MediaSource};

pub(crate) use loader::{media_placeholder_color, media_placeholder_label};
pub(crate) use manager::MediaManager;
#[cfg(any(feature = "audio", feature = "video"))]
pub(crate) use types::normalize_media_extension_hint;
pub(crate) use types::{
    resolve_media_rect, IntrinsicSize, MediaCompletion, MediaTextureKey, MediaTextureLayout,
    RasterRequest, TextureFrame,
};
