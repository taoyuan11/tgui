use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::media::{IntrinsicSize, TextureFrame};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoSource {
    File(PathBuf),
    Url(String),
}

impl From<PathBuf> for VideoSource {
    fn from(value: PathBuf) -> Self {
        Self::File(value)
    }
}

impl From<&std::path::Path> for VideoSource {
    fn from(value: &std::path::Path) -> Self {
        Self::File(value.to_path_buf())
    }
}

impl From<String> for VideoSource {
    fn from(value: String) -> Self {
        Self::Url(value)
    }
}

impl From<&str> for VideoSource {
    fn from(value: &str) -> Self {
        Self::Url(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Buffering,
    Ended,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VideoSize {
    pub width: u32,
    pub height: u32,
}

impl VideoSize {
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub(crate) fn intrinsic_size(self) -> IntrinsicSize {
        IntrinsicSize::from_pixels(self.width, self.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoMetrics {
    pub duration: Option<Duration>,
    pub position: Duration,
    pub buffered: Option<Duration>,
    pub video_width: u32,
    pub video_height: u32,
}

impl Default for VideoMetrics {
    fn default() -> Self {
        Self {
            duration: None,
            position: Duration::ZERO,
            buffered: None,
            video_width: 0,
            video_height: 0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct VideoSurfaceSnapshot {
    pub intrinsic_size: IntrinsicSize,
    pub texture: Option<Arc<TextureFrame>>,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for VideoSurfaceSnapshot {
    fn default() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: false,
            error: None,
        }
    }
}
