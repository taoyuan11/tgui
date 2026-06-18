use std::fs;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use reqwest::Url;

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;

use super::manager::{DocumentContent, DocumentEntry, ImageEntry, RasterDocument};
use super::raster::{decode_raster_asset, load_raster_dimensions, DecodedRasterAsset};
use super::svg::load_svg_document;
use super::types::{
    IntrinsicSize, MediaBytes, MediaCompletion, MediaSource, MediaTextureKey, RasterRequest,
};

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
static RUSTLS_PROVIDER: OnceLock<()> = OnceLock::new();

pub(super) fn spawn_image_loader(
    entry: Arc<Mutex<ImageEntry>>,
    source: MediaSource,
    invalidation: InvalidationSignal,
    completions: Arc<Mutex<Vec<MediaCompletion>>>,
) {
    thread::spawn(move || {
        let result = load_media_document(&source);
        let mut guard = entry.lock().expect("image entry lock poisoned");
        match result {
            Ok(document) => {
                *guard = ImageEntry::ready(document);
            }
            Err(error) => {
                *guard = ImageEntry::failed(error);
            }
        }
        completions
            .lock()
            .expect("media completion queue lock poisoned")
            .push(MediaCompletion::SourceLoaded { source });
        invalidation.mark_media_dirty();
    });
}

pub(super) fn spawn_raster_texture_loader(
    bytes: MediaBytes,
    source: MediaSource,
    raster_request: RasterRequest,
    slot: Arc<Mutex<Option<Result<DecodedRasterAsset, String>>>>,
    invalidation: InvalidationSignal,
    completions: Arc<Mutex<Vec<MediaCompletion>>>,
) {
    thread::spawn(move || {
        let result = decode_raster_asset(&bytes, raster_request).map_err(|error| error.to_string());
        let mut guard = slot.lock().expect("pending raster lock poisoned");
        *guard = Some(result);
        completions
            .lock()
            .expect("media completion queue lock poisoned")
            .push(MediaCompletion::RasterFinished {
                key: MediaTextureKey::new(source, raster_request),
            });
        invalidation.mark_media_dirty();
    });
}

pub(super) fn load_image_entry(source: &MediaSource) -> ImageEntry {
    match load_media_document(source) {
        Ok(document) => ImageEntry::ready(document),
        Err(error) => ImageEntry::failed(error),
    }
}

pub(super) fn load_media_document(source: &MediaSource) -> Result<DocumentEntry, TguiError> {
    let loaded = load_media_source(source)?;
    match load_raster_document(&loaded) {
        Ok(document) => Ok(document),
        Err(raster_error) => {
            if !looks_like_svg(source, &loaded) {
                return Err(raster_error);
            }
            load_svg_document(&loaded)
        }
    }
}

pub(super) enum LoadedSource<'a> {
    File {
        bytes: MediaBytes,
        path: PathBuf,
    },
    Url {
        bytes: MediaBytes,
        url: Url,
    },
    Embedded {
        bytes: MediaBytes,
        _marker: PhantomData<&'a ()>,
    },
}

impl<'a> LoadedSource<'a> {
    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::File { bytes, .. } | Self::Url { bytes, .. } | Self::Embedded { bytes, .. } => {
                bytes.as_slice()
            }
        }
    }

    pub(super) fn media_bytes(&self) -> MediaBytes {
        match self {
            Self::File { bytes, .. } | Self::Url { bytes, .. } | Self::Embedded { bytes, .. } => {
                bytes.clone()
            }
        }
    }

    pub(super) fn media_source(&self) -> MediaSource {
        match self {
            Self::File { path, .. } => MediaSource::Path(path.clone()),
            Self::Url { url, .. } => MediaSource::Url(url.to_string()),
            Self::Embedded { bytes, .. } => MediaSource::Bytes(bytes.clone()),
        }
    }
}

fn load_media_source(source: &MediaSource) -> Result<LoadedSource<'_>, TguiError> {
    match source {
        MediaSource::Path(path) => Ok(LoadedSource::File {
            bytes: MediaBytes::from(fs::read(path).map_err(|error| {
                TguiError::Media(format!("failed to read image {:?}: {error}", path))
            })?),
            path: path.clone(),
        }),
        MediaSource::Url(url) => {
            let parsed_url = Url::parse(url)
                .map_err(|error| TguiError::Media(format!("invalid image url {url}: {error}")))?;
            let bytes = http_client()?
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
                bytes: MediaBytes::from(bytes),
                url: parsed_url,
            })
        }
        MediaSource::Bytes(bytes) => Ok(LoadedSource::Embedded {
            bytes: bytes.clone(),
            _marker: PhantomData,
        }),
    }
}

fn load_raster_document(source: &LoadedSource<'_>) -> Result<DocumentEntry, TguiError> {
    let (width, height) = load_raster_dimensions(source.bytes())?;
    Ok(DocumentEntry {
        intrinsic_size: IntrinsicSize::from_pixels(width, height),
        content: DocumentContent::Raster(RasterDocument::new(
            source.media_source(),
            source.media_bytes(),
        )),
    })
}

fn looks_like_svg(source: &MediaSource, loaded: &LoadedSource<'_>) -> bool {
    source_path_looks_like_svg(source)
        || matches!(loaded, LoadedSource::Url { url, .. } if url.path().ends_with(".svg") || url.path().ends_with(".svgz"))
        || super::svg::looks_like_svg_bytes(loaded.bytes())
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

pub(super) fn http_client() -> Result<&'static reqwest::blocking::Client, TguiError> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }

    let _ = RUSTLS_PROVIDER.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    let client = reqwest::blocking::Client::builder()
        .pool_max_idle_per_host(8)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| {
            TguiError::Media(format!("failed to build HTTP client for media: {error}"))
        })?;
    Ok(HTTP_CLIENT.get_or_init(|| client))
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
