use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::media::{IntrinsicSize, TextureFrame};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoSource {
    File(PathBuf),
    Url {
        url: String,
        headers: Vec<(String, String)>,
    },
}

impl VideoSource {
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url {
            url: url.into(),
            headers: Vec::new(),
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let Self::Url { headers, .. } = &mut self {
            headers.push((name.into(), value.into()));
        }
        self
    }

    pub fn with_headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        if let Self::Url {
            headers: source_headers,
            ..
        } = &mut self
        {
            source_headers.extend(
                headers
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.into())),
            );
        }
        self
    }
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
        Self::url(value)
    }
}

impl From<&str> for VideoSource {
    fn from(value: &str) -> Self {
        Self::url(value)
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

#[cfg(test)]
mod tests {
    use super::VideoSource;

    #[test]
    fn string_conversions_create_url_sources_without_headers() {
        assert_eq!(
            VideoSource::from("https://example.com/demo.mp4"),
            VideoSource::Url {
                url: "https://example.com/demo.mp4".to_string(),
                headers: Vec::new(),
            }
        );
        assert_eq!(
            VideoSource::from("https://example.com/demo-2.mp4".to_string()),
            VideoSource::Url {
                url: "https://example.com/demo-2.mp4".to_string(),
                headers: Vec::new(),
            }
        );
    }

    #[test]
    fn header_builders_append_in_order() {
        let source = VideoSource::url("https://example.com/demo.mp4")
            .with_header("Authorization", "Bearer token")
            .with_headers([
                ("Referer", "https://example.com/app"),
                ("Cookie", "a=1; b=2"),
            ]);

        assert_eq!(
            source,
            VideoSource::Url {
                url: "https://example.com/demo.mp4".to_string(),
                headers: vec![
                    ("Authorization".to_string(), "Bearer token".to_string()),
                    ("Referer".to_string(), "https://example.com/app".to_string()),
                    ("Cookie".to_string(), "a=1; b=2".to_string()),
                ],
            }
        );
    }
}
