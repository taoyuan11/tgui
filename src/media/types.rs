use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ui::widget::Rect;

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);
const MAX_IMAGE_DIMENSION: u32 = 2048;

/// 表示媒体资源的来源。
///
/// 可用于本地路径、远程 URL 或内嵌字节数据。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MediaSource {
    Path(PathBuf),
    Url(String),
    Bytes(MediaBytes),
}

impl MediaSource {
    /// 使用本地文件路径构造媒体来源。
    ///
    /// 参数：
    /// - `path`：本地文件路径。
    ///
    /// 返回值：
    /// - 返回 `MediaSource::Path`。
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    /// 使用远程 URL 构造媒体来源。
    ///
    /// 参数：
    /// - `url`：远程资源地址。
    ///
    /// 返回值：
    /// - 返回 `MediaSource::Url`。
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

    /// 使用内存字节构造媒体来源。
    ///
    /// 参数：
    /// - `bytes`：媒体二进制内容。
    ///
    /// 返回值：
    /// - 返回 `MediaSource::Bytes`。
    pub fn bytes(bytes: impl Into<MediaBytes>) -> Self {
        Self::Bytes(bytes.into())
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

impl From<MediaBytes> for MediaSource {
    fn from(value: MediaBytes) -> Self {
        Self::Bytes(value)
    }
}

/// 表示一段可复用的媒体字节数据。
///
/// 该类型同时支持静态字节切片和共享引用计数缓冲区。
#[derive(Clone)]
pub struct MediaBytes {
    storage: MediaBytesStorage,
}

#[derive(Clone)]
enum MediaBytesStorage {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum MediaBytesKey {
    Static { ptr: usize, len: usize },
    Shared { ptr: usize, len: usize },
}

impl MediaBytes {
    /// 从静态字节切片构造媒体内容。
    ///
    /// 参数：
    /// - `bytes`：静态生命周期字节切片。
    ///
    /// 返回值：
    /// - 返回新的 `MediaBytes`。
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self {
            storage: MediaBytesStorage::Static(bytes),
        }
    }

    /// 从共享字节缓冲区构造媒体内容。
    ///
    /// 参数：
    /// - `bytes`：共享字节缓冲区。
    ///
    /// 返回值：
    /// - 返回新的 `MediaBytes`。
    pub fn from_shared(bytes: Arc<[u8]>) -> Self {
        Self {
            storage: MediaBytesStorage::Shared(bytes),
        }
    }

    /// 返回底层字节切片。
    ///
    /// 返回值：
    /// - 媒体内容的只读字节切片。
    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            MediaBytesStorage::Static(bytes) => bytes,
            MediaBytesStorage::Shared(bytes) => bytes,
        }
    }

    /// 返回底层字节长度。
    ///
    /// 返回值：
    /// - 字节数量。
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// 返回底层字节是否为空。
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    fn key(&self) -> MediaBytesKey {
        match &self.storage {
            MediaBytesStorage::Static(bytes) => MediaBytesKey::Static {
                ptr: bytes.as_ptr() as usize,
                len: bytes.len(),
            },
            MediaBytesStorage::Shared(bytes) => MediaBytesKey::Shared {
                ptr: bytes.as_ptr() as usize,
                len: bytes.len(),
            },
        }
    }
}

impl fmt::Debug for MediaBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MediaBytes")
            .field("len", &self.len())
            .finish()
    }
}

impl PartialEq for MediaBytes {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for MediaBytes {}

impl Hash for MediaBytes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl From<Arc<[u8]>> for MediaBytes {
    fn from(value: Arc<[u8]>) -> Self {
        Self::from_shared(value)
    }
}

impl From<Vec<u8>> for MediaBytes {
    fn from(value: Vec<u8>) -> Self {
        Self::from_shared(Arc::from(value))
    }
}

impl From<Box<[u8]>> for MediaBytes {
    fn from(value: Box<[u8]>) -> Self {
        Self::from_shared(Arc::from(value))
    }
}

impl From<&'static [u8]> for MediaBytes {
    fn from(value: &'static [u8]) -> Self {
        Self::from_static(value)
    }
}

impl<const N: usize> From<&'static [u8; N]> for MediaBytes {
    fn from(value: &'static [u8; N]) -> Self {
        Self::from_static(value.as_slice())
    }
}

/// 指定媒体内容在目标区域中的适配方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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

impl PartialEq for TextureFrame {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.revision == other.revision
    }
}

impl Eq for TextureFrame {}

impl TextureFrame {
    pub(crate) fn allocate_id() -> u64 {
        NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            id: Self::allocate_id(),
            revision: 1,
            width,
            height,
            pixels: Arc::from(pixels),
        }
    }

    #[cfg(feature = "video")]
    pub(crate) fn with_id_and_revision(
        id: u64,
        revision: u64,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            id,
            revision: revision.max(1),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RasterRequest {
    pub(super) width: u32,
    pub(super) height: u32,
}

impl RasterRequest {
    pub(crate) fn new_clamped(width: u32, height: u32) -> Self {
        clamp_raster_request(width, height)
    }

    pub(crate) fn from_frame(frame: Rect, scale_factor: f32) -> Option<Self> {
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return None;
        }

        let scale_factor = scale_factor.max(1.0 / 64.0);
        let width = (frame.width.get() * scale_factor).ceil().max(1.0) as u32;
        let height = (frame.height.get() * scale_factor).ceil().max(1.0) as u32;
        Some(clamp_raster_request(width, height))
    }

    pub(crate) fn width(self) -> u32 {
        self.width
    }

    pub(crate) fn height(self) -> u32 {
        self.height
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

pub(super) fn clamp_raster_request(width: u32, height: u32) -> RasterRequest {
    let longest_edge = width.max(height);
    if longest_edge <= MAX_IMAGE_DIMENSION {
        return RasterRequest { width, height };
    }

    let scale = MAX_IMAGE_DIMENSION as f32 / longest_edge as f32;
    RasterRequest {
        width: (width as f32 * scale).round().max(1.0) as u32,
        height: (height as f32 * scale).round().max(1.0) as u32,
    }
}
