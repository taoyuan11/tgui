use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AudioSource {
    File(PathBuf),
    Url {
        url: String,
        headers: Vec<(String, String)>,
    },
}

impl AudioSource {
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
