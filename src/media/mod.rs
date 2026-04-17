use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::ui::widget::Rect;

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);
static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
const MAX_IMAGE_DIMENSION: u32 = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MediaSource {
    Path(PathBuf),
    Url(String),
}

impl MediaSource {
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }
}

impl From<PathBuf> for MediaSource {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

impl From<&Path> for MediaSource {
    fn from(value: &Path) -> Self {
        Self::Path(value.to_path_buf())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContentFit {
    #[default]
    Contain,
    Cover,
    Fill,
}

#[derive(Clone, Debug)]
pub(crate) struct TextureFrame {
    id: u64,
    revision: u64,
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

impl TextureFrame {
    pub(crate) fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            id: NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed),
            revision: 1,
            width,
            height,
            pixels: Arc::from(pixels),
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntrinsicSize {
    pub width: f32,
    pub height: f32,
}

impl IntrinsicSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub fn from_pixels(width: u32, height: u32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
        }
    }

    pub fn aspect_ratio(self) -> Option<f32> {
        (self.width > 0.0 && self.height > 0.0).then_some(self.width / self.height)
    }
}

#[derive(Clone)]
pub(crate) struct ImageSnapshot {
    pub intrinsic_size: IntrinsicSize,
    pub texture: Option<Arc<TextureFrame>>,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for ImageSnapshot {
    fn default() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: false,
            error: None,
        }
    }
}

pub(crate) fn resolve_media_rect(frame: Rect, media: IntrinsicSize, fit: ContentFit) -> Rect {
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return Rect::new(frame.x, frame.y, 0.0, 0.0);
    }

    if media.width <= 0.0 || media.height <= 0.0 || fit == ContentFit::Fill {
        return frame;
    }

    let frame_ratio = frame.width / frame.height.max(1.0);
    let media_ratio = media.width / media.height.max(1.0);

    let (width, height) = match fit {
        ContentFit::Contain => {
            if media_ratio > frame_ratio {
                (frame.width, frame.width / media_ratio)
            } else {
                (frame.height * media_ratio, frame.height)
            }
        }
        ContentFit::Cover => {
            if media_ratio > frame_ratio {
                (frame.height * media_ratio, frame.height)
            } else {
                (frame.width, frame.width / media_ratio)
            }
        }
        ContentFit::Fill => (frame.width, frame.height),
    };

    Rect::new(
        frame.x + (frame.width - width) * 0.5,
        frame.y + (frame.height - height) * 0.5,
        width,
        height,
    )
}

pub(crate) struct MediaManager {
    invalidation: InvalidationSignal,
    images: Mutex<HashMap<MediaSource, Arc<Mutex<ImageEntry>>>>,
}

impl MediaManager {
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self {
            invalidation,
            images: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn image_snapshot(&self, source: &MediaSource) -> ImageSnapshot {
        let entry = {
            let mut images = self.images.lock().expect("image cache lock poisoned");
            images
                .entry(source.clone())
                .or_insert_with(|| {
                    let entry = Arc::new(Mutex::new(ImageEntry::loading()));
                    spawn_image_loader(entry.clone(), source.clone(), self.invalidation.clone());
                    entry
                })
                .clone()
        };

        let snapshot = entry.lock().expect("image entry lock poisoned").snapshot();
        snapshot
    }
}

struct ImageEntry {
    intrinsic_size: IntrinsicSize,
    texture: Option<Arc<TextureFrame>>,
    loading: bool,
    error: Option<String>,
}

impl ImageEntry {
    fn loading() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: true,
            error: None,
        }
    }

    fn snapshot(&self) -> ImageSnapshot {
        ImageSnapshot {
            intrinsic_size: self.intrinsic_size,
            texture: self.texture.clone(),
            loading: self.loading,
            error: self.error.clone(),
        }
    }
}

fn spawn_image_loader(
    entry: Arc<Mutex<ImageEntry>>,
    source: MediaSource,
    invalidation: InvalidationSignal,
) {
    thread::spawn(move || {
        let result = load_image_source(&source);
        let mut guard = entry.lock().expect("image entry lock poisoned");
        match result {
            Ok(texture) => {
                let (width, height) = texture.size();
                guard.intrinsic_size = IntrinsicSize::from_pixels(width, height);
                guard.texture = Some(Arc::new(texture));
                guard.loading = false;
                guard.error = None;
            }
            Err(error) => {
                guard.intrinsic_size = IntrinsicSize::ZERO;
                guard.texture = None;
                guard.loading = false;
                guard.error = Some(error.to_string());
            }
        }
        invalidation.mark_dirty();
    });
}

fn load_image_source(source: &MediaSource) -> Result<TextureFrame, TguiError> {
    let bytes = match source {
        MediaSource::Path(path) => fs::read(path).map_err(|error| {
            TguiError::Media(format!("failed to read image {:?}: {error}", path))
        })?,
        MediaSource::Url(url) => http_client()
            .get(url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| TguiError::Media(format!("failed to fetch image {url}: {error}")))?
            .bytes()
            .map_err(|error| TguiError::Media(format!("failed to read image body {url}: {error}")))?
            .to_vec(),
    };

    let mut image = image::load_from_memory(&bytes)
        .map_err(|error| TguiError::Media(format!("failed to decode image {source:?}: {error}")))?;
    let longest_edge = image.width().max(image.height());
    if longest_edge > MAX_IMAGE_DIMENSION {
        let scale = MAX_IMAGE_DIMENSION as f32 / longest_edge as f32;
        let width = (image.width() as f32 * scale).round().max(1.0) as u32;
        let height = (image.height() as f32 * scale).round().max(1.0) as u32;
        image = image.resize(width, height, image::imageops::FilterType::Triangle);
    }
    let rgba = image.to_rgba8();
    Ok(TextureFrame::new(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}

fn http_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client should build")
    })
}

pub(crate) fn media_placeholder_color(loading: bool, error: Option<&str>) -> Color {
    match (loading, error.is_some()) {
        (_, true) => Color::hexa(0x7F1D1DFF),
        (true, false) => Color::hexa(0x1E3A8AFF),
        (false, false) => Color::hexa(0x1F2937FF),
    }
}

pub(crate) fn media_placeholder_label(kind: &str, loading: bool, error: Option<&str>) -> String {
    if let Some(error) = error {
        let truncated = if error.chars().count() > 48 {
            let prefix = error.chars().take(45).collect::<String>();
            format!("{prefix}...")
        } else {
            error.to_string()
        };
        return format!("{kind} error: {truncated}");
    }

    if loading {
        format!("loading {kind}...")
    } else {
        format!("{kind} unavailable")
    }
}
