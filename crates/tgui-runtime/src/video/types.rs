use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::foundation::color::Color;
use crate::media::{
    normalize_media_extension_hint, IntrinsicSize, MediaPlaybackSource, MediaSource, TextureFrame,
};
use crate::text::font::FontWeight;

/// A video source accepted by [`VideoController`](crate::video::VideoController).
///
/// Local files are opened by path. URLs are opened through FFmpeg with optional
/// HTTP headers; header names and values are validated before a backend load is
/// accepted.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoSource {
    /// A local media file.
    File(PathBuf),
    /// A network media source and optional HTTP request headers.
    Url {
        /// The URL passed to FFmpeg.
        url: String,
        /// Extra HTTP headers sent with URL sources.
        headers: Vec<(String, String)>,
    },
    /// In-memory media bytes.
    ///
    /// FFmpeg backends materialize bytes sources as temporary media files for
    /// the lifetime of the decode session.
    Bytes {
        /// The complete encoded media payload.
        bytes: Arc<[u8]>,
        /// Optional container extension hint, such as `mp4`.
        extension: Option<Arc<str>>,
    },
}

impl VideoSource {
    /// Creates a URL-backed video source.
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url {
            url: url.into(),
            headers: Vec::new(),
        }
    }

    /// Creates an in-memory video source.
    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::Bytes {
            bytes: bytes.into(),
            extension: None,
        }
    }

    /// Creates an in-memory video source with a format extension hint.
    ///
    /// `extension` may be passed as `"mp4"` or `".mp4"`.
    pub fn bytes_with_extension(bytes: impl Into<Arc<[u8]>>, extension: impl Into<String>) -> Self {
        Self::bytes(bytes).with_extension(extension)
    }

    /// Appends one HTTP header to a URL source.
    ///
    /// Calling this on a file source is a no-op.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let Self::Url { headers, .. } = &mut self {
            headers.push((name.into(), value.into()));
        }
        self
    }

    /// Appends multiple HTTP headers to a URL source.
    ///
    /// Calling this on a file source is a no-op.
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

    /// Sets a format extension hint on an in-memory source.
    ///
    /// Calling this on a file or URL source is a no-op.
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

impl From<MediaSource> for VideoSource {
    fn from(value: MediaSource) -> Self {
        Self::from(MediaPlaybackSource::from(value))
    }
}

impl From<MediaPlaybackSource> for VideoSource {
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
mod video_source_tests {
    use crate::media::{MediaBytes, MediaPlaybackSource, MediaSource};

    use super::VideoSource;

    #[test]
    fn bytes_source_stores_payload_and_extension_hint() {
        let source = VideoSource::bytes_with_extension(vec![1, 2, 3], ".mp4");

        match source {
            VideoSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension.as_deref(), Some("mp4"));
            }
            _ => panic!("expected bytes source"),
        }
    }

    #[test]
    fn extension_hint_is_ignored_for_non_bytes_sources() {
        let source = VideoSource::url("https://example.com/demo.mp4").with_extension("webm");

        assert_eq!(source, VideoSource::url("https://example.com/demo.mp4"));
    }

    #[test]
    fn media_source_converts_to_video_source() {
        assert_eq!(
            VideoSource::from(MediaSource::path("demo.mp4")),
            VideoSource::File("demo.mp4".into())
        );
        assert_eq!(
            VideoSource::from(MediaSource::url("https://example.com/demo.mp4")),
            VideoSource::url("https://example.com/demo.mp4")
        );

        match VideoSource::from(MediaSource::bytes(MediaBytes::from_static(&[1, 2, 3]))) {
            VideoSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension, None);
            }
            _ => panic!("expected bytes source"),
        }
    }

    #[test]
    fn media_playback_source_preserves_video_headers_and_extension() {
        assert_eq!(
            VideoSource::from(
                MediaPlaybackSource::url("https://example.com/demo.mp4")
                    .with_header("Authorization", "Bearer token")
            ),
            VideoSource::url("https://example.com/demo.mp4")
                .with_header("Authorization", "Bearer token")
        );

        match VideoSource::from(MediaPlaybackSource::bytes_with_extension(
            MediaBytes::from_static(&[1, 2, 3]),
            ".mp4",
        )) {
            VideoSource::Bytes { bytes, extension } => {
                assert_eq!(&*bytes, &[1, 2, 3]);
                assert_eq!(extension.as_deref(), Some("mp4"));
            }
            _ => panic!("expected bytes source"),
        }
    }
}

/// An audio stream discovered in the currently loaded video source.
///
/// Track metadata is source-local. Use [`VideoController::audio_tracks`]
/// to observe the list after a source opens, then pass [`stream_index`](Self::stream_index)
/// to [`VideoAudioTrackSelection::Stream`] when selecting a specific track.
///
/// [`VideoController::audio_tracks`]: crate::video::VideoController::audio_tracks
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoAudioTrack {
    /// FFmpeg stream index used to select this track.
    pub stream_index: usize,
    /// Optional display title from stream metadata.
    pub title: Option<String>,
    /// Optional language tag from stream metadata.
    pub language: Option<String>,
    /// Decoder-reported channel count, when known.
    pub channels: u16,
    /// Decoder-reported sample rate, when known.
    pub sample_rate: u32,
}

/// Audio track selection for video playback.
///
/// The selection is a user preference stored on the controller. It is not
/// cleared when loading a new source, but source-specific discovered tracks are
/// cleared until the backend opens the new media. Selecting a stream index that
/// does not exist in the current source causes the backend load/reopen to fail
/// with a media error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VideoAudioTrackSelection {
    /// Use FFmpeg's best audio stream, if any.
    #[default]
    Auto,
    /// Do not decode or play audio.
    Disabled,
    /// Select a specific FFmpeg audio stream index.
    Stream(usize),
}

/// A subtitle stream discovered in the currently loaded video source.
///
/// Subtitle metadata is source-local. Use
/// [`VideoController::subtitle_tracks`](crate::video::VideoController::subtitle_tracks)
/// to observe the list after a source opens, then pass
/// [`stream_index`](Self::stream_index) to [`VideoSubtitleTrackSelection::Stream`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoSubtitleTrack {
    /// FFmpeg stream index used to select this subtitle track.
    pub stream_index: usize,
    /// Optional display title from stream metadata.
    pub title: Option<String>,
    /// Optional language tag from stream metadata.
    pub language: Option<String>,
    /// Codec name reported by FFmpeg, when known.
    pub codec: Option<String>,
}

/// Subtitle track selection for video playback.
///
/// The default is [`Disabled`](Self::Disabled). Selection is retained across
/// loads and stops until changed explicitly, while discovered subtitle tracks
/// are cleared with each new source. Selecting a stream index that does not
/// exist in the current source causes the backend load/reopen to fail with a
/// media error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VideoSubtitleTrackSelection {
    /// Do not decode or display subtitles.
    #[default]
    Disabled,
    /// Select a specific FFmpeg subtitle stream index.
    Stream(usize),
}

/// A decoded subtitle cue active on the video timeline.
///
/// Text cues are emitted for subtitle formats that FFmpeg exposes as plain text
/// or ASS/SSA text. Bitmap subtitle formats are surfaced separately through
/// [`VideoSubtitleBitmapCue`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoSubtitleCue {
    /// Subtitle text with basic ASS formatting stripped.
    pub text: String,
    /// Cue start time on the media timeline.
    pub start: Duration,
    /// Cue end time on the media timeline.
    pub end: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VideoSubtitleCuePlacement {
    pub horizontal: VideoSubtitleHorizontalAlign,
    pub vertical: VideoSubtitleVerticalAlign,
}

impl VideoSubtitleCuePlacement {
    pub(crate) const DEFAULT: Self = Self {
        horizontal: VideoSubtitleHorizontalAlign::Center,
        vertical: VideoSubtitleVerticalAlign::Bottom,
    };

    pub(crate) fn from_ass_alignment(alignment: u8) -> Option<Self> {
        let horizontal = match alignment {
            1 | 4 | 7 => VideoSubtitleHorizontalAlign::Left,
            2 | 5 | 8 => VideoSubtitleHorizontalAlign::Center,
            3 | 6 | 9 => VideoSubtitleHorizontalAlign::Right,
            _ => return None,
        };
        let vertical = match alignment {
            1..=3 => VideoSubtitleVerticalAlign::Bottom,
            4..=6 => VideoSubtitleVerticalAlign::Middle,
            7..=9 => VideoSubtitleVerticalAlign::Top,
            _ => return None,
        };
        Some(Self {
            horizontal,
            vertical,
        })
    }
}

impl Default for VideoSubtitleCuePlacement {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct VideoSubtitleCueStyle {
    pub primary_color: Option<Color>,
    pub outline_color: Option<Color>,
    pub shadow_color: Option<Color>,
    pub font_weight: Option<FontWeight>,
    pub font_size_centi_px: Option<u16>,
    pub outline_width_centi_px: Option<u16>,
    pub shadow_depth_centi_px: Option<u16>,
}

impl VideoSubtitleCueStyle {
    pub(crate) fn is_empty(self) -> bool {
        self.primary_color.is_none()
            && self.outline_color.is_none()
            && self.shadow_color.is_none()
            && self.font_weight.is_none()
            && self.font_size_centi_px.is_none()
            && self.outline_width_centi_px.is_none()
            && self.shadow_depth_centi_px.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum VideoSubtitleHorizontalAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum VideoSubtitleVerticalAlign {
    Top,
    Middle,
    Bottom,
}

/// A decoded bitmap subtitle cue active on the video timeline.
///
/// Bitmap cues carry RGBA pixels plus the subtitle rectangle position in the
/// decoded video coordinate space. The built-in [`VideoSurface`](crate::video::VideoSurface)
/// can render these directly over the current frame.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoSubtitleBitmapCue {
    /// Left position in decoded video pixels.
    pub x: u32,
    /// Top position in decoded video pixels.
    pub y: u32,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// RGBA pixels, four bytes per pixel.
    pub pixels: Arc<[u8]>,
    /// Cue start time on the media timeline.
    pub start: Duration,
    /// Cue end time on the media timeline.
    pub end: Duration,
    pub(crate) texture_id: u64,
    pub(crate) texture_revision: u64,
}

impl VideoSubtitleBitmapCue {
    pub(crate) fn new(
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        pixels: Arc<[u8]>,
        start: Duration,
        end: Duration,
    ) -> Option<Self> {
        let expected_len = u64::from(width)
            .checked_mul(u64::from(height))?
            .checked_mul(4)?
            .try_into()
            .ok();
        if width == 0 || height == 0 || expected_len != Some(pixels.len()) || end <= start {
            return None;
        }

        Some(Self {
            x,
            y,
            width,
            height,
            pixels,
            start,
            end,
            texture_id: TextureFrame::allocate_id(),
            texture_revision: 1,
        })
    }

    pub(crate) fn texture_frame(&self) -> TextureFrame {
        TextureFrame::with_id_revision_and_pixels(
            self.texture_id,
            self.texture_revision,
            self.width,
            self.height,
            Arc::clone(&self.pixels),
        )
    }
}

/// Playback state reported by a [`VideoController`](crate::video::VideoController).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum VideoPlaybackState {
    /// No source is loaded or playback was stopped.
    #[default]
    Idle,
    /// The current source is being opened or seeked.
    Loading,
    /// The source is opened and ready to play.
    Ready,
    /// Playback is advancing.
    Playing,
    /// Playback is paused at the current position.
    Paused,
    /// Playback is waiting for enough decoded media to resume.
    Buffering,
    /// Playback reached the end of the source.
    Ended,
    /// The backend failed to load or decode the source.
    Error(String),
}

/// The decoded video dimensions in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VideoSize {
    /// Width in pixels, or `0` when unknown.
    pub width: u32,
    /// Height in pixels, or `0` when unknown.
    pub height: u32,
}

impl VideoSize {
    /// Returns `true` when either dimension is unknown or zero.
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub(crate) fn intrinsic_size(self) -> IntrinsicSize {
        IntrinsicSize::from_pixels(self.width, self.height)
    }
}

/// Timeline and decoded-size metrics for a video controller.
///
/// Metrics are updated lazily after they are observed through controller
/// signals such as [`VideoController::position`](crate::video::VideoController::position).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoMetrics {
    /// Total media duration, when the container reports one.
    pub duration: Option<Duration>,
    /// Current playback position.
    pub position: Duration,
    /// Buffered position, when the backend can estimate it.
    pub buffered: Option<Duration>,
    /// Current decoded video width in pixels.
    pub video_width: u32,
    /// Current decoded video height in pixels.
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

#[derive(Clone, PartialEq)]
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
