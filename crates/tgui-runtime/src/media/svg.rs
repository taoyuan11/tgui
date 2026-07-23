use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use reqwest::Url;
use resvg::{self, tiny_skia, usvg};
use usvg_remote_resolvers::HrefStringResolver;

use crate::foundation::error::TguiError;

use super::loader::{fetch_http_bytes, LoadedSource, SVG_EXTERNAL_IMAGE_MAX_BODY_BYTES};
use super::manager::{DocumentContent, DocumentEntry, SvgDocument};
use super::types::{IntrinsicSize, RasterRequest, TextureFrame};

type ExternalErrorSlot = Arc<Mutex<Option<String>>>;

/// `fontdb::Database::load_system_fonts` scans every platform font directory and parses the
/// metadata of every discovered face. SVG parsing used to repeat that scan for every document,
/// including the tiny path-only SVGs backing built-in icons. A page containing dozens of icons
/// consequently spent seconds in the scene collector in an unoptimized build.
///
/// A font database is immutable once installed in `usvg::Options`, and `usvg` already represents
/// it as an `Arc`. Keep one process-wide snapshot so all SVG documents retain identical text
/// support while paying the system scan only once.
static SVG_SYSTEM_FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
static SVG_SYSTEM_FONTS_PRELOAD: OnceLock<()> = OnceLock::new();

fn svg_system_fonts() -> Arc<usvg::fontdb::Database> {
    SVG_SYSTEM_FONTS
        .get_or_init(|| {
            let mut fontdb = usvg::fontdb::Database::new();
            fontdb.load_system_fonts();
            Arc::new(fontdb)
        })
        .clone()
}

pub(crate) fn preload_svg_system_fonts() {
    if SVG_SYSTEM_FONTS.get().is_some() {
        return;
    }
    SVG_SYSTEM_FONTS_PRELOAD.get_or_init(|| {
        std::thread::spawn(|| {
            let _ = svg_system_fonts();
        });
    });
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

pub(super) fn load_svg_document(source: &LoadedSource<'_>) -> Result<DocumentEntry, TguiError> {
    let errors = Arc::new(Mutex::new(None));
    let mut options = svg_options(source, errors.clone());
    options.fontdb = svg_system_fonts();

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::media::loader::load_media_document;
    use crate::media::manager::DocumentContent;
    use crate::media::MediaSource;

    use super::{preload_svg_system_fonts, svg_system_fonts};

    #[test]
    fn svg_documents_share_one_system_font_database() {
        const FIRST: &[u8] =
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><path d="M0 0h8v8Z"/></svg>"#;
        const SECOND: &[u8] =
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"><text x="0" y="7">A</text></svg>"#;

        let first = load_media_document(&MediaSource::bytes(FIRST)).expect("first SVG should load");
        let second =
            load_media_document(&MediaSource::bytes(SECOND)).expect("second SVG should load");
        let DocumentContent::Svg(first) = first.content else {
            panic!("first document should be SVG");
        };
        let DocumentContent::Svg(second) = second.content else {
            panic!("second document should be SVG");
        };

        assert!(Arc::ptr_eq(first.font_database(), second.font_database()));
        assert!(Arc::ptr_eq(first.font_database(), &svg_system_fonts()));
        assert!(first.font_database().faces().next().is_some());
    }

    #[test]
    fn svg_font_preload_is_concurrent_and_idempotent() {
        let callers = (0..16)
            .map(|_| std::thread::spawn(preload_svg_system_fonts))
            .collect::<Vec<_>>();
        for caller in callers {
            caller.join().expect("preload caller should not panic");
        }

        let first = svg_system_fonts();
        preload_svg_system_fonts();
        let second = svg_system_fonts();
        assert!(Arc::ptr_eq(&first, &second));
    }
}
