use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::Url;
use resvg::{self, tiny_skia, usvg};
use usvg_remote_resolvers::HrefStringResolver;

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::ui::widget::Rect;

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);
static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
static RUSTLS_PROVIDER: OnceLock<()> = OnceLock::new();
const MAX_IMAGE_DIMENSION: u32 = 2048;
const MAX_SVG_RASTER_CACHE_ENTRIES: usize = 4;
const MAX_CANVAS_SHADOW_CACHE_ENTRIES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MediaSource {
    Path(PathBuf),
    Url(String),
    Bytes(MediaBytes),
}

impl MediaSource {
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

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
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self {
            storage: MediaBytesStorage::Static(bytes),
        }
    }

    pub fn from_shared(bytes: Arc<[u8]>) -> Self {
        Self {
            storage: MediaBytesStorage::Shared(bytes),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            MediaBytesStorage::Static(bytes) => bytes,
            MediaBytesStorage::Shared(bytes) => bytes,
        }
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RasterRequest {
    width: u32,
    height: u32,
}

impl RasterRequest {
    pub(crate) fn from_frame(frame: Rect) -> Option<Self> {
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return None;
        }

        let width = frame.width.round().max(1.0).get() as u32;
        let height = frame.height.round().max(1.0).get() as u32;
        Some(clamp_raster_request(width, height))
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
    canvas_shadows: Mutex<Vec<CanvasShadowEntry>>,
}

impl MediaManager {
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self {
            invalidation,
            images: Mutex::new(HashMap::new()),
            canvas_shadows: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn image_snapshot(
        &self,
        source: &MediaSource,
        raster_request: Option<RasterRequest>,
    ) -> ImageSnapshot {
        let entry = self.image_entry(source);
        let snapshot = entry
            .lock()
            .expect("image entry lock poisoned")
            .snapshot(raster_request, &self.invalidation);
        snapshot
    }

    fn image_entry(&self, source: &MediaSource) -> Arc<Mutex<ImageEntry>> {
        let mut images = self.images.lock().expect("image cache lock poisoned");
        images
            .entry(source.clone())
            .or_insert_with(|| {
                let entry = Arc::new(Mutex::new(ImageEntry::loading()));
                spawn_image_loader(entry.clone(), source.clone(), self.invalidation.clone());
                entry
            })
            .clone()
    }

    pub(crate) fn canvas_shadow_texture<F>(
        &self,
        cache_key: u64,
        width: u32,
        height: u32,
        render: F,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError>
    where
        F: FnOnce() -> Result<TextureFrame, TguiError>,
    {
        if width == 0 || height == 0 {
            return Ok(None);
        }

        let mut cache = self
            .canvas_shadows
            .lock()
            .expect("canvas shadow cache lock poisoned");
        if let Some(entry) = cache.iter_mut().find(|entry| {
            entry.cache_key == cache_key && entry.width == width && entry.height == height
        }) {
            entry.last_used = entry.last_used.saturating_add(1);
            return Ok(Some(entry.texture.clone()));
        }

        let texture = Arc::new(render()?);
        let next_tick = cache
            .iter()
            .map(|entry| entry.last_used)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        cache.push(CanvasShadowEntry {
            cache_key,
            width,
            height,
            texture: texture.clone(),
            last_used: next_tick,
        });
        while cache.len() > MAX_CANVAS_SHADOW_CACHE_ENTRIES {
            if let Some((oldest_index, _)) = cache
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_used)
            {
                cache.remove(oldest_index);
            } else {
                break;
            }
        }

        Ok(Some(texture))
    }
}

struct CanvasShadowEntry {
    cache_key: u64,
    width: u32,
    height: u32,
    texture: Arc<TextureFrame>,
    last_used: u64,
}

struct ImageEntry {
    document: Option<DocumentEntry>,
    loading: bool,
    error: Option<String>,
}

impl ImageEntry {
    fn loading() -> Self {
        Self {
            document: None,
            loading: true,
            error: None,
        }
    }

    fn snapshot(
        &mut self,
        raster_request: Option<RasterRequest>,
        invalidation: &InvalidationSignal,
    ) -> ImageSnapshot {
        let intrinsic_size = self
            .document
            .as_ref()
            .map(|document| document.intrinsic_size)
            .unwrap_or(IntrinsicSize::ZERO);

        let texture = if self.loading || self.error.is_some() {
            None
        } else if let (Some(document), Some(request)) = (self.document.as_mut(), raster_request) {
            match document.texture_for(request) {
                Ok(texture) => texture,
                Err(error) => {
                    self.error = Some(error.to_string());
                    invalidation.mark_dirty();
                    None
                }
            }
        } else {
            None
        };

        ImageSnapshot {
            intrinsic_size,
            texture,
            loading: self.loading,
            error: self.error.clone(),
        }
    }
}

struct DocumentEntry {
    intrinsic_size: IntrinsicSize,
    content: DocumentContent,
}

impl DocumentEntry {
    fn texture_for(
        &mut self,
        raster_request: RasterRequest,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let raster_request = clamp_raster_request(raster_request.width, raster_request.height);
        match &mut self.content {
            DocumentContent::Raster(texture) => Ok(Some(texture.clone())),
            DocumentContent::Svg(svg) => svg.texture_for(raster_request),
        }
    }
}

enum DocumentContent {
    Raster(Arc<TextureFrame>),
    Svg(SvgDocument),
}

struct SvgDocument {
    tree: Arc<usvg::Tree>,
    raster_cache: Vec<SvgRasterEntry>,
    next_access_tick: u64,
}

impl SvgDocument {
    fn new(tree: usvg::Tree) -> Self {
        Self {
            tree: Arc::new(tree),
            raster_cache: Vec::new(),
            next_access_tick: 1,
        }
    }

    fn texture_for(
        &mut self,
        raster_request: RasterRequest,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let tick = self.bump_access_tick();
        if let Some(entry) = self
            .raster_cache
            .iter_mut()
            .find(|entry| entry.request == raster_request)
        {
            entry.last_used = tick;
            return Ok(Some(entry.texture.clone()));
        }

        let texture = Arc::new(rasterize_svg_tree(&self.tree, raster_request)?);
        self.raster_cache.push(SvgRasterEntry {
            request: raster_request,
            texture: texture.clone(),
            last_used: tick,
        });
        self.evict_if_needed();
        Ok(Some(texture))
    }

    fn bump_access_tick(&mut self) -> u64 {
        let tick = self.next_access_tick;
        self.next_access_tick = self.next_access_tick.saturating_add(1);
        tick
    }

    fn evict_if_needed(&mut self) {
        while self.raster_cache.len() > MAX_SVG_RASTER_CACHE_ENTRIES {
            if let Some((oldest_index, _)) = self
                .raster_cache
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_used)
            {
                self.raster_cache.remove(oldest_index);
            } else {
                break;
            }
        }
    }
}

struct SvgRasterEntry {
    request: RasterRequest,
    texture: Arc<TextureFrame>,
    last_used: u64,
}

type ExternalErrorSlot = Arc<Mutex<Option<String>>>;

enum LoadedSource<'a> {
    File { bytes: Cow<'a, [u8]>, path: PathBuf },
    Url { bytes: Cow<'a, [u8]>, url: Url },
    Embedded { bytes: Cow<'a, [u8]> },
}

impl<'a> LoadedSource<'a> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::File { bytes, .. } | Self::Url { bytes, .. } | Self::Embedded { bytes } => {
                bytes.as_ref()
            }
        }
    }
}

#[derive(Clone)]
struct SvgHrefResolver {
    base_url: Option<Url>,
    allow_local_paths: bool,
    errors: ExternalErrorSlot,
}

impl SvgHrefResolver {
    fn new(base_url: Option<Url>, allow_local_paths: bool, errors: ExternalErrorSlot) -> Self {
        Self {
            base_url,
            allow_local_paths,
            errors,
        }
    }

    fn resolve_remote_url(&self, href: &str) -> Option<Url> {
        if href.trim().is_empty() {
            return None;
        }

        if let Ok(url) = Url::parse(href) {
            return matches!(url.scheme(), "http" | "https").then_some(url);
        }

        self.base_url
            .as_ref()
            .and_then(|base_url| base_url.join(href).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
    }
}

impl<'a> HrefStringResolver<'a> for SvgHrefResolver {
    fn is_target(&self, _href: &str) -> bool {
        true
    }

    fn get_image_kind(&self, href: &str, options: &usvg::Options<'_>) -> Option<usvg::ImageKind> {
        if let Some(url) = self.resolve_remote_url(href) {
            return fetch_remote_image_kind(&url, options, &self.errors);
        }

        if self.allow_local_paths {
            let kind = usvg::ImageHrefResolver::default_string_resolver()(href, options);
            if kind.is_none() && !href.trim().is_empty() {
                record_external_error(
                    &self.errors,
                    format!("failed to resolve SVG image reference `{href}`"),
                );
            }
            return kind;
        }

        record_external_error(
            &self.errors,
            format!("unsupported SVG image reference `{href}` for embedded SVG source"),
        );
        None
    }
}

fn spawn_image_loader(
    entry: Arc<Mutex<ImageEntry>>,
    source: MediaSource,
    invalidation: InvalidationSignal,
) {
    thread::spawn(move || {
        let result = load_media_document(&source);
        let mut guard = entry.lock().expect("image entry lock poisoned");
        match result {
            Ok(document) => {
                guard.document = Some(document);
                guard.loading = false;
                guard.error = None;
            }
            Err(error) => {
                guard.document = None;
                guard.loading = false;
                guard.error = Some(error.to_string());
            }
        }
        invalidation.mark_dirty();
    });
}

fn load_media_document(source: &MediaSource) -> Result<DocumentEntry, TguiError> {
    let loaded = load_media_source(source)?;
    match load_raster_document(loaded.bytes()) {
        Ok(document) => Ok(document),
        Err(raster_error) => {
            if !looks_like_svg(source, &loaded) {
                return Err(raster_error);
            }
            load_svg_document(&loaded)
        }
    }
}

fn load_media_source(source: &MediaSource) -> Result<LoadedSource<'_>, TguiError> {
    match source {
        MediaSource::Path(path) => Ok(LoadedSource::File {
            bytes: Cow::Owned(fs::read(path).map_err(|error| {
                TguiError::Media(format!("failed to read image {:?}: {error}", path))
            })?),
            path: path.clone(),
        }),
        MediaSource::Url(url) => {
            let parsed_url = Url::parse(url)
                .map_err(|error| TguiError::Media(format!("invalid image url {url}: {error}")))?;
            let bytes = http_client()
                .get(parsed_url.clone())
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| {
                    TguiError::Media(format!("failed to fetch image {parsed_url}: {error}"))
                })?
                .bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(|error| {
                    TguiError::Media(format!("failed to read image body {parsed_url}: {error}"))
                })?;
            Ok(LoadedSource::Url {
                bytes: Cow::Owned(bytes),
                url: parsed_url,
            })
        }
        MediaSource::Bytes(bytes) => Ok(LoadedSource::Embedded {
            bytes: Cow::Borrowed(bytes.as_slice()),
        }),
    }
}

fn load_raster_document(bytes: &[u8]) -> Result<DocumentEntry, TguiError> {
    let mut image = image::load_from_memory(bytes)
        .map_err(|error| TguiError::Media(format!("failed to decode raster image: {error}")))?;
    let longest_edge = image.width().max(image.height());
    if longest_edge > MAX_IMAGE_DIMENSION {
        let scale = MAX_IMAGE_DIMENSION as f32 / longest_edge as f32;
        let width = (image.width() as f32 * scale).round().max(1.0) as u32;
        let height = (image.height() as f32 * scale).round().max(1.0) as u32;
        image = image.resize(width, height, image::imageops::FilterType::Triangle);
    }

    let rgba = image.to_rgba8();
    let texture = Arc::new(TextureFrame::new(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ));

    Ok(DocumentEntry {
        intrinsic_size: IntrinsicSize::from_pixels(texture.size().0, texture.size().1),
        content: DocumentContent::Raster(texture),
    })
}

fn load_svg_document(source: &LoadedSource<'_>) -> Result<DocumentEntry, TguiError> {
    let errors = Arc::new(Mutex::new(None));
    let mut options = svg_options(source, errors.clone());
    options.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_data(source.bytes(), &options)
        .map_err(|error| TguiError::Media(format!("failed to parse SVG image: {error}")))?;

    if let Some(error) = take_external_error(&errors) {
        return Err(TguiError::Media(error));
    }

    let size = tree.size();
    Ok(DocumentEntry {
        intrinsic_size: IntrinsicSize {
            width: size.width(),
            height: size.height(),
        },
        content: DocumentContent::Svg(SvgDocument::new(tree)),
    })
}

fn svg_options<'a>(source: &LoadedSource<'_>, errors: ExternalErrorSlot) -> usvg::Options<'a> {
    let base_url = match source {
        LoadedSource::Url { url, .. } => Some(url.clone()),
        _ => None,
    };
    let allow_local_paths = matches!(source, LoadedSource::File { .. });
    let data_errors = errors.clone();
    let default_data_resolver = usvg::ImageHrefResolver::default_data_resolver();

    let mut options = usvg::Options::default();
    if let LoadedSource::File { path, .. } = source {
        options.resources_dir = path.parent().map(Path::to_path_buf);
    }
    options.image_href_resolver.resolve_data = Box::new(move |mime, data, options| {
        let resolved = default_data_resolver(mime, data, options);
        if resolved.is_none() {
            record_external_error(
                &data_errors,
                "failed to resolve SVG data URL image reference".to_string(),
            );
        }
        resolved
    });
    options.image_href_resolver.resolve_string =
        SvgHrefResolver::new(base_url, allow_local_paths, errors).into_fn();
    options
}

fn rasterize_svg_tree(
    tree: &Arc<usvg::Tree>,
    request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    let mut pixmap = tiny_skia::Pixmap::new(request.width, request.height).ok_or_else(|| {
        TguiError::Media(format!(
            "failed to allocate SVG raster surface {}x{}",
            request.width, request.height
        ))
    })?;
    let svg_size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        request.width as f32 / svg_size.width().max(1.0),
        request.height as f32 / svg_size.height().max(1.0),
    );
    resvg::render(tree.as_ref(), transform, &mut pixmap.as_mut());
    Ok(TextureFrame::new(
        request.width,
        request.height,
        pixmap.data().to_vec(),
    ))
}

fn fetch_remote_image_kind(
    url: &Url,
    options: &usvg::Options<'_>,
    errors: &ExternalErrorSlot,
) -> Option<usvg::ImageKind> {
    let response = match http_client().get(url.clone()).send() {
        Ok(response) => response,
        Err(error) => {
            record_external_error(
                errors,
                format!("failed to fetch SVG image reference {url}: {error}"),
            );
            return None;
        }
    };
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            record_external_error(
                errors,
                format!("failed to fetch SVG image reference {url}: {error}"),
            );
            return None;
        }
    };
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let bytes = match response.bytes() {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => {
            record_external_error(
                errors,
                format!("failed to read SVG image reference {url}: {error}"),
            );
            return None;
        }
    };
    match bytes_to_image_kind(bytes, content_type.as_deref(), options) {
        Ok(kind) => Some(kind),
        Err(error) => {
            record_external_error(errors, format!("{error}"));
            None
        }
    }
}

fn bytes_to_image_kind(
    bytes: Vec<u8>,
    content_type: Option<&str>,
    options: &usvg::Options<'_>,
) -> Result<usvg::ImageKind, TguiError> {
    if content_type_is_svg(content_type) || looks_like_svg_bytes(&bytes) {
        return usvg::Tree::from_data_nested(&bytes, options)
            .map(usvg::ImageKind::SVG)
            .map_err(|error| {
                TguiError::Media(format!("failed to parse nested SVG image: {error}"))
            });
    }

    match image::guess_format(&bytes)
        .map_err(|error| TguiError::Media(format!("failed to decode nested image: {error}")))?
    {
        image::ImageFormat::Jpeg => Ok(usvg::ImageKind::JPEG(Arc::new(bytes))),
        image::ImageFormat::Png => Ok(usvg::ImageKind::PNG(Arc::new(bytes))),
        image::ImageFormat::Gif => Ok(usvg::ImageKind::GIF(Arc::new(bytes))),
        image::ImageFormat::WebP => Ok(usvg::ImageKind::WEBP(Arc::new(bytes))),
        format => Err(TguiError::Media(format!(
            "unsupported nested image format {format:?}"
        ))),
    }
}

fn looks_like_svg(source: &MediaSource, loaded: &LoadedSource<'_>) -> bool {
    source_path_looks_like_svg(source)
        || matches!(loaded, LoadedSource::Url { url, .. } if url.path().ends_with(".svg") || url.path().ends_with(".svgz"))
        || looks_like_svg_bytes(loaded.bytes())
}

fn source_path_looks_like_svg(source: &MediaSource) -> bool {
    match source {
        MediaSource::Path(path) => path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| {
                extension.eq_ignore_ascii_case("svg") || extension.eq_ignore_ascii_case("svgz")
            })
            .unwrap_or(false),
        MediaSource::Url(url) => {
            let lowercase = url.to_ascii_lowercase();
            lowercase.ends_with(".svg") || lowercase.ends_with(".svgz")
        }
        MediaSource::Bytes(_) => false,
    }
}

fn looks_like_svg_bytes(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1F, 0x8B]) {
        return true;
    }

    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with("<svg")
        || trimmed.starts_with("<?xml")
        || trimmed
            .get(..trimmed.len().min(256))
            .map(|prefix| prefix.contains("<svg"))
            .unwrap_or(false)
}

fn content_type_is_svg(content_type: Option<&str>) -> bool {
    content_type
        .map(|value| value.split(';').next().unwrap_or(value).trim())
        .map(|value| value.eq_ignore_ascii_case("image/svg+xml"))
        .unwrap_or(false)
}

fn clamp_raster_request(width: u32, height: u32) -> RasterRequest {
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

fn record_external_error(errors: &ExternalErrorSlot, message: String) {
    let mut guard = errors.lock().expect("external SVG error lock poisoned");
    if guard.is_none() {
        *guard = Some(message);
    }
}

fn take_external_error(errors: &ExternalErrorSlot) -> Option<String> {
    errors
        .lock()
        .expect("external SVG error lock poisoned")
        .take()
}

fn http_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        let _ = RUSTLS_PROVIDER.get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
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

#[cfg(test)]
mod tests {
    use super::{load_media_document, DocumentContent, MediaManager, MediaSource, RasterRequest};
    use crate::foundation::binding::InvalidationSignal;
    use std::collections::HashMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const ONE_BY_ONE_GIF: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02,
        0x01, 0x4C, 0x00, 0x3B,
    ];
    const SIMPLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#22c55e"/></svg>"##;

    #[test]
    fn loads_image_from_embedded_bytes() {
        let document = load_media_document(&MediaSource::bytes(ONE_BY_ONE_GIF))
            .expect("embedded bytes should decode");

        assert_eq!(document.intrinsic_size.width, 1.0);
        assert_eq!(document.intrinsic_size.height, 1.0);
        assert!(matches!(document.content, DocumentContent::Raster(_)));
    }

    #[test]
    fn loads_svg_document_metadata_from_embedded_bytes() {
        let document = load_media_document(&MediaSource::bytes(SIMPLE_SVG))
            .expect("embedded SVG should decode");

        assert_eq!(document.intrinsic_size.width, 10.0);
        assert_eq!(document.intrinsic_size.height, 20.0);
        assert!(matches!(document.content, DocumentContent::Svg(_)));
    }

    #[test]
    fn svg_rasterizes_per_requested_size_and_reuses_cached_texture() {
        let mut document = load_media_document(&MediaSource::bytes(SIMPLE_SVG))
            .expect("embedded SVG should decode");

        let first = document
            .texture_for(RasterRequest {
                width: 20,
                height: 40,
            })
            .expect("SVG rasterization should work")
            .expect("SVG should produce a texture");
        let second = document
            .texture_for(RasterRequest {
                width: 20,
                height: 40,
            })
            .expect("SVG rasterization should work")
            .expect("SVG should produce a texture");
        let third = document
            .texture_for(RasterRequest {
                width: 40,
                height: 80,
            })
            .expect("SVG rasterization should work")
            .expect("SVG should produce a texture");

        assert_eq!(first.size(), (20, 40));
        assert_eq!(third.size(), (40, 80));
        assert_eq!(first.id(), second.id());
        assert_ne!(first.id(), third.id());
    }

    #[test]
    fn svg_raster_request_is_clamped_to_max_dimension() {
        let mut document = load_media_document(&MediaSource::bytes(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="2048"><rect width="4096" height="2048" fill="#2563eb"/></svg>"##,
        ))
        .expect("large SVG should decode");

        let texture = document
            .texture_for(RasterRequest {
                width: 4096,
                height: 2048,
            })
            .expect("SVG rasterization should work")
            .expect("SVG should produce a texture");

        assert_eq!(texture.size(), (2048, 1024));
    }

    #[test]
    fn canvas_shadow_cache_reuses_matching_texture() {
        let media = MediaManager::new(InvalidationSignal::new());
        let first = media
            .canvas_shadow_texture(42, 16, 16, || {
                Ok(super::TextureFrame::new(16, 16, vec![0; 16 * 16 * 4]))
            })
            .expect("shadow rasterization should succeed")
            .expect("shadow texture should be cached");
        let second = media
            .canvas_shadow_texture(42, 16, 16, || {
                Ok(super::TextureFrame::new(16, 16, vec![255; 16 * 16 * 4]))
            })
            .expect("shadow cache lookup should succeed")
            .expect("shadow texture should be cached");
        let third = media
            .canvas_shadow_texture(43, 16, 16, || {
                Ok(super::TextureFrame::new(16, 16, vec![255; 16 * 16 * 4]))
            })
            .expect("new shadow cache entry should succeed")
            .expect("shadow texture should be cached");

        assert_eq!(first.id(), second.id());
        assert_ne!(first.id(), third.id());
    }

    #[test]
    fn svg_from_path_resolves_relative_local_images() {
        let temp_dir = unique_temp_dir();
        fs::create_dir_all(&temp_dir).expect("temporary directory should exist");
        let image_path = temp_dir.join("pixel.gif");
        let svg_path = temp_dir.join("doc.svg");
        fs::write(&image_path, ONE_BY_ONE_GIF).expect("image fixture should be written");
        fs::write(
            &svg_path,
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#,
        )
        .expect("svg fixture should be written");

        let document = load_media_document(&MediaSource::path(&svg_path))
            .expect("SVG with relative local image should decode");
        assert_eq!(document.intrinsic_size.width, 8.0);
        assert_eq!(document.intrinsic_size.height, 8.0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn svg_from_url_resolves_relative_http_images() {
        let server = TestServer::new(HashMap::from([
            (
                "/doc.svg".to_string(),
                TestResponse::new(
                    "image/svg+xml",
                    br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#.to_vec(),
                ),
            ),
            (
                "/pixel.gif".to_string(),
                TestResponse::new("image/gif", ONE_BY_ONE_GIF.to_vec()),
            ),
        ]));

        let document =
            load_media_document(&MediaSource::url(format!("{}/doc.svg", server.base_url)))
                .expect("SVG with relative HTTP image should decode");
        assert_eq!(document.intrinsic_size.width, 8.0);
        assert_eq!(document.intrinsic_size.height, 8.0);
    }

    #[test]
    fn embedded_svg_rejects_relative_local_image_references() {
        let error = match load_media_document(&MediaSource::bytes(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#,
        )) {
            Ok(_) => panic!("embedded SVG should reject relative local image references"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("unsupported SVG image reference"));
    }

    #[test]
    fn svg_external_image_failures_surface_as_media_errors() {
        let server = TestServer::new(HashMap::from([(
            "/doc.svg".to_string(),
            TestResponse::new(
                "image/svg+xml",
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="missing.gif" width="8" height="8"/></svg>"#.to_vec(),
            ),
        )]));

        let error =
            match load_media_document(&MediaSource::url(format!("{}/doc.svg", server.base_url))) {
                Ok(_) => panic!("missing external image should fail the SVG"),
                Err(error) => error,
            };
        assert!(error
            .to_string()
            .contains("failed to fetch SVG image reference"));
    }

    #[test]
    fn media_manager_uses_intrinsic_size_before_svg_rasterization() {
        let media = MediaManager::new(InvalidationSignal::new());
        let source = MediaSource::bytes(SIMPLE_SVG);

        let metadata = wait_for_snapshot(&media, &source, None);
        assert_eq!(metadata.intrinsic_size.width, 10.0);
        assert_eq!(metadata.intrinsic_size.height, 20.0);
        assert!(metadata.texture.is_none());

        let rasterized = wait_for_snapshot(
            &media,
            &source,
            Some(RasterRequest {
                width: 20,
                height: 40,
            }),
        );
        assert_eq!(
            rasterized
                .texture
                .expect("SVG should rasterize once a target size is requested")
                .size(),
            (20, 40)
        );
    }

    fn wait_for_snapshot(
        media: &MediaManager,
        source: &MediaSource,
        raster_request: Option<RasterRequest>,
    ) -> super::ImageSnapshot {
        for _ in 0..150 {
            let snapshot = media.image_snapshot(source, raster_request);
            if !snapshot.loading {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for media snapshot");
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tests")
            .as_nanos();
        std::env::temp_dir().join(format!("tgui-svg-test-{nanos}"))
    }

    struct TestResponse {
        content_type: &'static str,
        body: Vec<u8>,
        status_line: &'static str,
    }

    impl TestResponse {
        fn new(content_type: &'static str, body: Vec<u8>) -> Self {
            Self {
                content_type,
                body,
                status_line: "HTTP/1.1 200 OK",
            }
        }
    }

    struct TestServer {
        base_url: String,
        shutdown_tx: mpsc::Sender<()>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn new(routes: HashMap<String, TestResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
            listener
                .set_nonblocking(true)
                .expect("test server should be non-blocking");
            let address = listener
                .local_addr()
                .expect("test server should expose an address");
            let base_url = format!("http://{address}");
            let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

            let handle = thread::spawn(move || loop {
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0u8; 4096];
                        let bytes_read = stream.read(&mut buffer).unwrap_or(0);
                        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");

                        let response = routes.get(path);
                        let (status_line, content_type, body) = if let Some(response) = response {
                            (
                                response.status_line,
                                response.content_type,
                                response.body.clone(),
                            )
                        } else {
                            ("HTTP/1.1 404 Not Found", "text/plain", b"missing".to_vec())
                        };

                        let header = format!(
                            "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("test server accept failed: {error}"),
                }
            });

            Self {
                base_url,
                shutdown_tx,
                handle: Some(handle),
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let _ = self.shutdown_tx.send(());
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }
}
