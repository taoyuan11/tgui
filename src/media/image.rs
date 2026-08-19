use crate::core::{
    DenseArena, Error, GenerationStamp, ImageHandle, ResourceRevision, Result, RevisionSet,
    WindowId,
};
use crate::diagnostics::{
    BudgetDomain, BudgetError, CacheBudgetSnapshot, FixedBudgetResourceManager,
};
use crate::state::{BackgroundMessage, DispatchBatch, RevisionMask, UiDispatcher, UiInbox};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Stable image input. Backend availability does not change this public type.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ImageSource {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
    Url(Arc<str>),
    Svg(Arc<str>),
}

impl ImageSource {
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::Bytes(bytes.into())
    }

    pub fn url(url: impl Into<Arc<str>>) -> Result<Self> {
        let url = url.into();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(Error::invalid_input(
                Some("image.url".to_owned()),
                "only http and https image URLs are supported",
            ));
        }
        Ok(Self::Url(url))
    }

    pub fn svg(svg: impl Into<Arc<str>>) -> Self {
        Self::Svg(svg.into())
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            _ => None,
        }
    }
}

impl fmt::Debug for ImageSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => formatter.debug_tuple("Path").field(path).finish(),
            Self::Bytes(bytes) => formatter
                .debug_struct("Bytes")
                .field("len", &bytes.len())
                .finish(),
            Self::Url(url) => formatter.debug_tuple("Url").field(url).finish(),
            Self::Svg(svg) => formatter
                .debug_struct("Svg")
                .field("len", &svg.len())
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

impl ImageSize {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::invalid_input(
                Some("image.size".to_owned()),
                "image dimensions must be non-zero",
            ));
        }
        Ok(Self { width, height })
    }

    pub const fn byte_len_rgba8(self) -> u64 {
        (self.width as u64)
            .saturating_mul(self.height as u64)
            .saturating_mul(4)
    }
}

/// Exact request identity used for in-flight and decoded-image deduplication.
///
/// SVG target size is part of the key because it changes raster output. Raster
/// sources normally leave it `None` and retain their encoded intrinsic size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageRequestKey {
    source: ImageSource,
    raster_size: Option<ImageSize>,
}

impl ImageRequestKey {
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            raster_size: None,
        }
    }

    pub fn with_raster_size(mut self, size: ImageSize) -> Self {
        self.raster_size = Some(size);
        self
    }

    pub fn source(&self) -> &ImageSource {
        &self.source
    }

    pub const fn raster_size(&self) -> Option<ImageSize> {
        self.raster_size
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    size: ImageSize,
    rgba8: Arc<[u8]>,
}

impl DecodedImage {
    pub fn new(size: ImageSize, rgba8: impl Into<Arc<[u8]>>) -> Result<Self> {
        let rgba8 = rgba8.into();
        if u64::try_from(rgba8.len()).unwrap_or(u64::MAX) != size.byte_len_rgba8() {
            return Err(Error::resource(
                None,
                "decoded RGBA8 byte count does not match image dimensions",
                true,
            ));
        }
        Ok(Self { size, rgba8 })
    }

    pub const fn size(&self) -> ImageSize {
        self.size
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }

    pub const fn byte_len(&self) -> u64 {
        self.size.byte_len_rgba8()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageLoadError {
    Io(Arc<str>),
    NetworkResolverRequired,
    Decode(Arc<str>),
    ImageFeatureDisabled,
    SvgFeatureDisabled,
    InvalidDimensions,
}

impl fmt::Display for ImageLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "image I/O failed: {message}"),
            Self::NetworkResolverRequired => {
                formatter.write_str("URL image source requires an ImageSourceResolver")
            }
            Self::Decode(message) => write!(formatter, "image decode failed: {message}"),
            Self::ImageFeatureDisabled => formatter.write_str("the image feature is disabled"),
            Self::SvgFeatureDisabled => formatter.write_str("the svg feature is disabled"),
            Self::InvalidDimensions => formatter.write_str("decoded image dimensions are invalid"),
        }
    }
}

impl std::error::Error for ImageLoadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImagePayload {
    Encoded(Arc<[u8]>),
    Svg(Arc<str>),
}

/// Resolves transport separately from decoding. Applications can provide a
/// network implementation without forcing a particular HTTP/TLS stack on tgui.
pub trait ImageSourceResolver: Send + Sync + 'static {
    fn resolve(&self, source: &ImageSource) -> std::result::Result<ImagePayload, ImageLoadError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalImageSourceResolver;

impl ImageSourceResolver for LocalImageSourceResolver {
    fn resolve(&self, source: &ImageSource) -> std::result::Result<ImagePayload, ImageLoadError> {
        match source {
            ImageSource::Path(path)
                if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) =>
            {
                fs::read_to_string(path)
                    .map(|svg| ImagePayload::Svg(svg.into()))
                    .map_err(|error| ImageLoadError::Io(error.to_string().into()))
            }
            ImageSource::Path(path) => fs::read(path)
                .map(|bytes| ImagePayload::Encoded(bytes.into()))
                .map_err(|error| ImageLoadError::Io(error.to_string().into())),
            ImageSource::Bytes(bytes) => Ok(ImagePayload::Encoded(bytes.clone())),
            ImageSource::Url(_) => Err(ImageLoadError::NetworkResolverRequired),
            ImageSource::Svg(svg) => Ok(ImagePayload::Svg(svg.clone())),
        }
    }
}

pub fn decode_image<R>(
    key: &ImageRequestKey,
    resolver: &R,
) -> std::result::Result<DecodedImage, ImageLoadError>
where
    R: ImageSourceResolver + ?Sized,
{
    match resolver.resolve(key.source())? {
        ImagePayload::Encoded(bytes) => decode_raster(&bytes),
        ImagePayload::Svg(svg) => decode_svg(&svg, key.raster_size()),
    }
}

#[cfg(feature = "image")]
fn decode_raster(bytes: &[u8]) -> std::result::Result<DecodedImage, ImageLoadError> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ImageLoadError::Decode(error.to_string().into()))?;
    let image = reader
        .decode()
        .map_err(|error| ImageLoadError::Decode(error.to_string().into()))?
        .into_rgba8();
    let size = ImageSize::new(image.width(), image.height())
        .map_err(|_| ImageLoadError::InvalidDimensions)?;
    DecodedImage::new(size, image.into_raw())
        .map_err(|error| ImageLoadError::Decode(error.to_string().into()))
}

#[cfg(not(feature = "image"))]
fn decode_raster(_bytes: &[u8]) -> std::result::Result<DecodedImage, ImageLoadError> {
    Err(ImageLoadError::ImageFeatureDisabled)
}

#[cfg(feature = "svg")]
fn decode_svg(
    svg: &str,
    raster_size: Option<ImageSize>,
) -> std::result::Result<DecodedImage, ImageLoadError> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|error| ImageLoadError::Decode(error.to_string().into()))?;
    let intrinsic = tree.size();
    let size = match raster_size {
        Some(size) => size,
        None => ImageSize::new(
            intrinsic.width().ceil() as u32,
            intrinsic.height().ceil() as u32,
        )
        .map_err(|_| ImageLoadError::InvalidDimensions)?,
    };
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width, size.height)
        .ok_or(ImageLoadError::InvalidDimensions)?;
    let scale_x = size.width as f32 / intrinsic.width();
    let scale_y = size.height as f32 / intrinsic.height();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );
    DecodedImage::new(size, pixmap.take())
        .map_err(|error| ImageLoadError::Decode(error.to_string().into()))
}

#[cfg(not(feature = "svg"))]
fn decode_svg(
    _svg: &str,
    _raster_size: Option<ImageSize>,
) -> std::result::Result<DecodedImage, ImageLoadError> {
    Err(ImageLoadError::SvgFeatureDisabled)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageState {
    Loading,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
struct ImageRecord {
    key: ImageRequestKey,
    state: ImageState,
    intrinsic_size: Option<ImageSize>,
    fallback: Option<ImageHandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRequest {
    pub handle: ImageHandle,
    pub needs_decode: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImagePresentation {
    Texture(ImageHandle),
    Placeholder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageCompletion {
    Ready {
        handle: ImageHandle,
        intrinsic_size_changed: bool,
    },
    Failed {
        handle: ImageHandle,
    },
    Stale,
}

/// UI-owned handle registry. Removing or replacing a source advances the slot
/// generation, so late worker results cannot bind to a new image.
pub struct ImageRegistry {
    records: DenseArena<ImageRecord, ImageHandle>,
    requests: HashMap<ImageRequestKey, ImageHandle>,
    _not_send: PhantomData<Rc<()>>,
}

impl ImageRegistry {
    pub fn new() -> Self {
        Self {
            records: DenseArena::new(),
            requests: HashMap::new(),
            _not_send: PhantomData,
        }
    }

    pub fn request(&mut self, key: ImageRequestKey) -> ImageRequest {
        if let Some(handle) = self.requests.get(&key).copied() {
            if self.records.contains(handle) {
                return ImageRequest {
                    handle,
                    needs_decode: false,
                };
            }
        }
        let handle = self.records.insert(ImageRecord {
            key: key.clone(),
            state: ImageState::Loading,
            intrinsic_size: None,
            fallback: None,
        });
        self.requests.insert(key, handle);
        ImageRequest {
            handle,
            needs_decode: true,
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Rebinds one logical image and retains its old texture as a loading
    /// fallback. The returned handle always has a different generation.
    pub fn replace(&mut self, handle: ImageHandle, key: ImageRequestKey) -> Option<ImageRequest> {
        let old = self.records.remove(handle)?;
        self.requests.remove(&old.key);
        if let Some(existing) = self.requests.get(&key).copied() {
            if self.records.contains(existing) {
                return Some(ImageRequest {
                    handle: existing,
                    needs_decode: false,
                });
            }
        }
        let fallback = (old.state == ImageState::Ready).then_some(handle);
        let next = self.records.insert(ImageRecord {
            key: key.clone(),
            state: ImageState::Loading,
            intrinsic_size: old.intrinsic_size,
            fallback,
        });
        self.requests.insert(key, next);
        Some(ImageRequest {
            handle: next,
            needs_decode: true,
        })
    }

    pub fn remove(&mut self, handle: ImageHandle) -> bool {
        let Some(record) = self.records.remove(handle) else {
            return false;
        };
        self.requests.remove(&record.key);
        true
    }

    pub fn state(&self, handle: ImageHandle) -> Option<ImageState> {
        self.records.get(handle).map(|record| record.state)
    }

    pub fn key(&self, handle: ImageHandle) -> Option<&ImageRequestKey> {
        self.records.get(handle).map(|record| &record.key)
    }

    pub fn intrinsic_size(&self, handle: ImageHandle) -> Option<ImageSize> {
        self.records
            .get(handle)
            .and_then(|record| record.intrinsic_size)
    }

    pub fn presentation(&self, handle: ImageHandle) -> ImagePresentation {
        let Some(record) = self.records.get(handle) else {
            return ImagePresentation::Placeholder;
        };
        match record.state {
            ImageState::Ready => ImagePresentation::Texture(handle),
            ImageState::Loading | ImageState::Failed => record
                .fallback
                .map_or(ImagePresentation::Placeholder, ImagePresentation::Texture),
        }
    }

    pub fn accepts(&self, source: GenerationStamp, key: &ImageRequestKey) -> bool {
        let handle = ImageHandle::from_parts(source.slot(), source.generation());
        self.records
            .get(handle)
            .is_some_and(|record| &record.key == key)
    }

    /// Applies a result after its requested revision has been validated. Prefer
    /// [`Self::drain_decode_results`] when consuming a [`UiInbox`].
    pub fn complete(
        &mut self,
        source: GenerationStamp,
        result: &ImageDecodeResult,
    ) -> ImageCompletion {
        let handle = ImageHandle::from_parts(source.slot(), source.generation());
        let Some(record) = self.records.get_mut(handle) else {
            return ImageCompletion::Stale;
        };
        if record.key != result.key || result.handle != handle {
            return ImageCompletion::Stale;
        }
        match &result.decoded {
            Ok(image) => {
                let old_size = record.intrinsic_size;
                record.state = ImageState::Ready;
                record.intrinsic_size = Some(image.size());
                record.fallback = None;
                ImageCompletion::Ready {
                    handle,
                    intrinsic_size_changed: old_size != Some(image.size()),
                }
            }
            Err(_) => {
                record.state = ImageState::Failed;
                ImageCompletion::Failed { handle }
            }
        }
    }

    pub fn drain_decode_results(
        &mut self,
        inbox: &UiInbox<ImageDecodeResult>,
        target: WindowId,
        current_revisions: RevisionSet,
        cpu_cache: &mut CpuImageCache,
    ) -> Result<ImageCompletionBatch> {
        let DispatchBatch {
            accepted,
            stale: rejected,
        } = inbox.drain_valid(|message| {
            message.target == target
                && message
                    .revision_mask
                    .matches(message.requested_revisions, current_revisions)
                && self.accepts(message.source, &message.payload.key)
                && message.payload.handle.stamp() == message.source
        })?;
        let mut completions = Vec::with_capacity(accepted.len());
        for message in accepted {
            let mut payload = message.payload;
            match &payload.decoded {
                Ok(decoded) => {
                    if let Err(error) = cpu_cache.insert(payload.key.clone(), decoded.clone()) {
                        payload.decoded = Err(ImageLoadError::Decode(
                            format!("CPU image cache rejected decoded output: {error}").into(),
                        ));
                    }
                }
                Err(_) => cpu_cache.record_failure(),
            }
            completions.push(self.complete(message.source, &payload));
        }
        Ok(ImageCompletionBatch {
            completions,
            stale: rejected,
        })
    }
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDecodeRequest {
    pub target: WindowId,
    pub handle: ImageHandle,
    pub key: ImageRequestKey,
    pub requested_revisions: RevisionSet,
    pub revision_mask: RevisionMask,
}

impl ImageDecodeRequest {
    pub fn new(
        target: WindowId,
        handle: ImageHandle,
        key: ImageRequestKey,
        requested_revisions: RevisionSet,
    ) -> Self {
        Self {
            target,
            handle,
            key,
            requested_revisions,
            revision_mask: RevisionMask::RESOURCE,
        }
    }

    pub fn with_revision_mask(mut self, revision_mask: RevisionMask) -> Self {
        self.revision_mask = revision_mask;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDecodeResult {
    pub handle: ImageHandle,
    pub key: ImageRequestKey,
    pub decoded: std::result::Result<DecodedImage, ImageLoadError>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageCompletionBatch {
    pub completions: Vec<ImageCompletion>,
    pub stale: usize,
}

pub fn spawn_image_decode<R>(
    request: ImageDecodeRequest,
    resolver: Arc<R>,
    dispatcher: UiDispatcher<ImageDecodeResult>,
) -> JoinHandle<()>
where
    R: ImageSourceResolver + ?Sized,
{
    thread::spawn(move || {
        let decoded = decode_image(&request.key, resolver.as_ref());
        let payload = ImageDecodeResult {
            handle: request.handle,
            key: request.key,
            decoded,
        };
        let _ = dispatcher.send(BackgroundMessage::new_with_mask(
            request.target,
            request.handle.stamp(),
            request.requested_revisions,
            request.revision_mask,
            payload,
        ));
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub failures: u64,
    pub evictions: u64,
    pub resident_bytes: u64,
    pub peak_bytes: u64,
    pub uploads: u64,
    pub upload_bytes: u64,
}

pub struct CpuImageCache {
    entries: FixedBudgetResourceManager<ImageRequestKey, Arc<DecodedImage>>,
    failures: u64,
}

impl CpuImageCache {
    pub fn new(soft_limit_bytes: u64, hard_limit_bytes: u64) -> Result<Self, BudgetError> {
        Ok(Self {
            entries: FixedBudgetResourceManager::new(
                BudgetDomain::CpuCache,
                soft_limit_bytes,
                hard_limit_bytes,
            )?,
            failures: 0,
        })
    }

    pub fn get(&mut self, key: &ImageRequestKey) -> Option<Arc<DecodedImage>> {
        self.entries.get(key).cloned()
    }

    pub fn peek(&self, key: &ImageRequestKey) -> Option<&Arc<DecodedImage>> {
        self.entries.peek(key)
    }

    pub fn insert(
        &mut self,
        key: ImageRequestKey,
        image: DecodedImage,
    ) -> std::result::Result<(), BudgetError> {
        let bytes = image.byte_len();
        let result = self.entries.insert(key, Arc::new(image), bytes);
        if result.is_err() {
            self.failures = self.failures.saturating_add(1);
        }
        result
    }

    pub fn record_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
    }

    pub fn stats(&self) -> ImageCacheStats {
        stats_from_budget(
            self.entries.len(),
            self.entries.budget_snapshot(),
            self.failures,
            0,
        )
    }

    pub fn budget_snapshot(&self) -> CacheBudgetSnapshot {
        self.entries.budget_snapshot()
    }
}

pub trait ImageTextureUploader {
    type Texture;

    fn upload_image(&mut self, handle: ImageHandle, image: &DecodedImage) -> Result<Self::Texture>;
}

pub struct GpuTexture<T> {
    pub handle: ImageHandle,
    pub size: ImageSize,
    pub resource_revision: ResourceRevision,
    pub texture: T,
}

pub struct GpuTextureCache<T> {
    entries: FixedBudgetResourceManager<ImageHandle, GpuTexture<T>>,
    uploads: u64,
    failures: u64,
}

impl<T> GpuTextureCache<T> {
    pub fn new(soft_limit_bytes: u64, hard_limit_bytes: u64) -> Result<Self, BudgetError> {
        Ok(Self {
            entries: FixedBudgetResourceManager::new(
                BudgetDomain::GpuCache,
                soft_limit_bytes,
                hard_limit_bytes,
            )?,
            uploads: 0,
            failures: 0,
        })
    }

    pub fn get(&mut self, handle: ImageHandle) -> Option<&GpuTexture<T>> {
        self.entries.get(&handle)
    }

    pub fn peek(&self, handle: ImageHandle) -> Option<&GpuTexture<T>> {
        self.entries.peek(&handle)
    }

    pub fn resolve(
        &mut self,
        primary: ImageHandle,
        fallback: Option<ImageHandle>,
    ) -> Option<&GpuTexture<T>> {
        let key = if self.entries.contains_key(&primary) {
            primary
        } else {
            fallback?
        };
        self.entries.get(&key)
    }

    pub fn upload<U>(
        &mut self,
        uploader: &mut U,
        handle: ImageHandle,
        image: &DecodedImage,
        resource_revision: ResourceRevision,
    ) -> std::result::Result<(), GpuImageCacheError>
    where
        U: ImageTextureUploader<Texture = T>,
    {
        let texture = uploader.upload_image(handle, image).map_err(|error| {
            self.failures = self.failures.saturating_add(1);
            GpuImageCacheError::Upload(error)
        })?;
        let bytes = image.byte_len();
        self.entries
            .insert_with_upload(
                handle,
                GpuTexture {
                    handle,
                    size: image.size(),
                    resource_revision,
                    texture,
                },
                bytes,
                bytes,
            )
            .map_err(|error| {
                self.failures = self.failures.saturating_add(1);
                GpuImageCacheError::Budget(error)
            })?;
        self.uploads = self.uploads.saturating_add(1);
        Ok(())
    }

    pub fn mark_committed(&mut self, handle: ImageHandle) -> bool {
        self.entries.mark_committed(&handle)
    }

    pub fn release_committed(&mut self, handle: ImageHandle) -> bool {
        self.entries.release_committed(&handle)
    }

    pub fn mark_in_flight(&mut self, handle: ImageHandle) -> bool {
        self.entries.mark_in_flight(&handle)
    }

    pub fn release_in_flight(&mut self, handle: ImageHandle) -> bool {
        self.entries.release_in_flight(&handle)
    }

    pub fn stats(&self) -> ImageCacheStats {
        stats_from_budget(
            self.entries.len(),
            self.entries.budget_snapshot(),
            self.failures,
            self.uploads,
        )
    }

    pub fn budget_snapshot(&self) -> CacheBudgetSnapshot {
        self.entries.budget_snapshot()
    }
}

#[derive(Debug)]
pub enum GpuImageCacheError {
    Upload(Error),
    Budget(BudgetError),
}

impl fmt::Display for GpuImageCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upload(error) => error.fmt(formatter),
            Self::Budget(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GpuImageCacheError {}

fn stats_from_budget(
    entries: usize,
    budget: CacheBudgetSnapshot,
    failures: u64,
    uploads: u64,
) -> ImageCacheStats {
    ImageCacheStats {
        entries,
        hits: budget.hits,
        misses: budget.misses,
        failures,
        evictions: budget.evictions,
        resident_bytes: budget.current_bytes,
        peak_bytes: budget.peak_bytes,
        uploads,
        upload_bytes: budget.upload_bytes,
    }
}
