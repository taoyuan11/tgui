use std::path::Path;
use std::sync::{Arc, Mutex};

use reqwest::Url;
use resvg::{self, tiny_skia, usvg};
use usvg_remote_resolvers::HrefStringResolver;

use crate::foundation::error::TguiError;

use super::loader::{fetch_http_bytes, LoadedSource, SVG_EXTERNAL_IMAGE_MAX_BODY_BYTES};
use super::manager::{DocumentContent, DocumentEntry, SvgDocument};
use super::types::{IntrinsicSize, RasterRequest, TextureFrame};

type ExternalErrorSlot = Arc<Mutex<Option<String>>>;

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

pub(super) fn load_svg_document(source: &LoadedSource<'_>) -> Result<DocumentEntry, TguiError> {
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

pub(super) fn rasterize_svg_tree(
    tree: &Arc<usvg::Tree>,
    request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    let mut pixmap =
        tiny_skia::Pixmap::new(request.width(), request.height()).ok_or_else(|| {
            TguiError::Media(format!(
                "failed to allocate SVG raster surface {}x{}",
                request.width(),
                request.height()
            ))
        })?;
    let svg_size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        request.width() as f32 / svg_size.width().max(1.0),
        request.height() as f32 / svg_size.height().max(1.0),
    );
    resvg::render(tree.as_ref(), transform, &mut pixmap.as_mut());
    Ok(TextureFrame::new(
        request.width(),
        request.height(),
        pixmap.data().to_vec(),
    ))
}

fn fetch_remote_image_kind(
    url: &Url,
    options: &usvg::Options<'_>,
    errors: &ExternalErrorSlot,
) -> Option<usvg::ImageKind> {
    let fetched = match fetch_http_bytes(
        url,
        SVG_EXTERNAL_IMAGE_MAX_BODY_BYTES,
        "SVG image reference",
    ) {
        Ok(fetched) => fetched,
        Err(error) => {
            record_external_error(errors, format!("{error}"));
            return None;
        }
    };
    match bytes_to_image_kind(fetched.bytes, fetched.content_type.as_deref(), options) {
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

pub(super) fn looks_like_svg_bytes(bytes: &[u8]) -> bool {
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
