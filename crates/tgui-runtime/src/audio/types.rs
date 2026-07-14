use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::media::{normalize_media_extension_hint, MediaPlaybackSource, MediaSource};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// 音频资源来源。
pub enum AudioSource {
    File(PathBuf),
    Url {
        url: String,
        headers: Vec<(String, String)>,
    },
    Bytes {
        bytes: Arc<[u8]>,
        extension: Option<Arc<str>>,
    },
}

impl AudioSource {
    /// 创建一个网络音频源。
    ///
    /// # 参数
    /// - `url`：音频资源地址。
    ///
    /// # 返回值
    /// 返回不带额外请求头的 URL 音频源。
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url {
            url: url.into(),
            headers: Vec::new(),
        }
    }

    /// 创建一个内存音频源。
    ///
    /// 该源会在 FFmpeg 后端加载时写入一个临时只读媒体文件，并在会话关闭后清理。
    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::Bytes {
            bytes: bytes.into(),
            extension: None,
        }
    }

    /// 创建带格式扩展名提示的内存音频源。
    ///
    /// `extension` 可以传入 `"mp3"` 或 `".mp3"`；提示只用于临时文件名，
    /// 便于 FFmpeg 探测容器格式。
    pub fn bytes_with_extension(bytes: impl Into<Arc<[u8]>>, extension: impl Into<String>) -> Self {
        Self::bytes(bytes).with_extension(extension)
    }

    /// 为网络音频源追加一个请求头。
    ///
    /// # 参数
    /// - `name`：请求头名称。
    /// - `value`：请求头值。
    ///
    /// # 返回值
    /// 返回更新后的音频源；文件音频源会保持原样。
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let Self::Url { headers, .. } = &mut self {
            headers.push((name.into(), value.into()));
        }
        self
    }

    /// 为网络音频源批量追加请求头。
    ///
    /// # 参数
    /// - `headers`：可迭代的请求头集合。
    ///
    /// # 返回值
    /// 返回更新后的音频源；文件音频源会保持原样。
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

    /// 为内存音频源设置格式扩展名提示。
    ///
    /// 对文件和 URL 音频源调用时保持原样。
    pub fn with_extension(mut self, extension: impl Into<String>) -> Self {
        if let Self::Bytes {
            extension: source_extension,
            ..
        } = &mut self
        {
            *source_extension = normalize_media_extension_hint(extension);
        }
        self
    }
}

impl From<PathBuf> for AudioSource {
    fn from(value: PathBuf) -> Self {
        Self::File(value)
    }
}

impl From<&std::path::Path> for AudioSource {
    fn from(value: &std::path::Path) -> Self {
        Self::File(value.to_path_buf())
    }
}

impl From<String> for AudioSource {
    fn from(value: String) -> Self {
        Self::url(value)
    }
}

impl From<&str> for AudioSource {
    fn from(value: &str) -> Self {
        Self::url(value)
    }
}

impl From<MediaSource> for AudioSource {
    fn from(value: MediaSource) -> Self {
        Self::from(MediaPlaybackSource::from(value))
    }
}

impl From<MediaPlaybackSource> for AudioSource {
    fn from(value: MediaPlaybackSource) -> Self {
        match value {
            MediaPlaybackSource::File(path) => Self::File(path),
            MediaPlaybackSource::Url { url, headers } => Self::Url { url, headers },
            MediaPlaybackSource::Bytes { bytes, extension } => Self::Bytes {
                bytes: bytes.into_shared_bytes(),
                extension,
            },
        }
    }
}

#[cfg(test)]
mod audio_source_tests {
    use crate::media::{MediaBytes, MediaPlaybackSource, MediaSource};

    use super::AudioSource;

    #[test]
    fn bytes_source_stores_payload_and_extension_hint() {
        let source = AudioSource::bytes_with_extension(vec![1, 2, 3], ".mp3");

        match source {
            AudioSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension.as_deref(), Some("mp3"));
            }
            _ => panic!("expected bytes source"),
        }
    }

    #[test]
    fn extension_hint_is_ignored_for_non_bytes_sources() {
        let source = AudioSource::File("demo.mp3".into()).with_extension("wav");

        assert_eq!(source, AudioSource::File("demo.mp3".into()));
    }

    #[test]
    fn media_source_converts_to_audio_source() {
        assert_eq!(
            AudioSource::from(MediaSource::path("demo.mp3")),
            AudioSource::File("demo.mp3".into())
        );
        assert_eq!(
            AudioSource::from(MediaSource::url("https://example.com/demo.mp3")),
            AudioSource::url("https://example.com/demo.mp3")
        );

        match AudioSource::from(MediaSource::bytes(MediaBytes::from_static(&[1, 2, 3]))) {
            AudioSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension, None);
            }
            _ => panic!("expected bytes source"),
        }
    }

    #[test]
    fn media_playback_source_preserves_audio_headers_and_extension() {
        assert_eq!(
            AudioSource::from(
                MediaPlaybackSource::url("https://example.com/demo.mp3")
                    .with_header("Authorization", "Bearer token")
            ),
            AudioSource::url("https://example.com/demo.mp3")
                .with_header("Authorization", "Bearer token")
        );

        match AudioSource::from(MediaPlaybackSource::bytes_with_extension(
            MediaBytes::from_static(&[1, 2, 3]),
            ".mp3",
        )) {
            AudioSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension.as_deref(), Some("mp3"));
            }
            _ => panic!("expected bytes source"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
/// 音频播放状态。
pub enum AudioPlaybackState {
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

#[derive(Clone, Debug, PartialEq, Eq)]
/// 音频播放进度与缓冲指标。
pub struct AudioMetrics {
    pub duration: Option<Duration>,
    pub position: Duration,
    pub buffered: Option<Duration>,
}

impl Default for AudioMetrics {
    fn default() -> Self {
        Self {
            duration: None,
            position: Duration::ZERO,
            buffered: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct AudioSnapshot {
    pub loading: bool,
    pub error: Option<String>,
}
