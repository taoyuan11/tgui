use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::foundation::color::Color;
use crate::media::{ContentFit, MediaBytes, MediaSource};
use crate::ui::unit::UnitContext;

use super::super::common::{ClipMask, Point, Rect, ScenePrimitives, TexturePrimitive};
use super::BuiltinIcon;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(not(feature = "video"), allow(dead_code))]
pub(crate) enum SvgIconId {
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    Close,
    Check,
    MoreHorizontal,
    Search,
    Star,
    StarHalf,
    User,
    Image,
    Plus,
    Minus,
    Info,
    Success,
    Warning,
    Error,
    Calendar,
    Clock,
    Home,
    Settings,
    Bell,
    Mail,
    Lock,
    Unlock,
    Eye,
    EyeOff,
    Edit,
    Copy,
    Download,
    Folder,
    RefreshCw,
    ExternalLink,
    Menu,
    Filter,
    SortAsc,
    SortDesc,
    MapPin,
    Link,
    Heart,
    Palette,
    Upload,
    File,
    Delete,
    Pending,
    CheckboxChecked,
    CheckboxUnchecked,
    CheckboxIndeterminate,
    PlayArrow,
    Pause,
    VolumeUp,
    VolumeDown,
    VolumeOff,
    VolumeMute,
}

impl From<BuiltinIcon> for SvgIconId {
    fn from(value: BuiltinIcon) -> Self {
        match value {
            BuiltinIcon::ChevronLeft => Self::ChevronLeft,
            BuiltinIcon::ChevronRight => Self::ChevronRight,
            BuiltinIcon::ChevronUp => Self::ChevronUp,
            BuiltinIcon::ChevronDown => Self::ChevronDown,
            BuiltinIcon::Close => Self::Close,
            BuiltinIcon::Check => Self::Check,
            BuiltinIcon::MoreHorizontal => Self::MoreHorizontal,
            BuiltinIcon::Search => Self::Search,
            BuiltinIcon::Star => Self::Star,
            BuiltinIcon::StarHalf => Self::StarHalf,
            BuiltinIcon::User => Self::User,
            BuiltinIcon::Image => Self::Image,
            BuiltinIcon::Plus => Self::Plus,
            BuiltinIcon::Minus => Self::Minus,
            BuiltinIcon::Info => Self::Info,
            BuiltinIcon::Success => Self::Success,
            BuiltinIcon::Warning => Self::Warning,
            BuiltinIcon::Error => Self::Error,
            BuiltinIcon::Calendar => Self::Calendar,
            BuiltinIcon::Clock => Self::Clock,
            BuiltinIcon::Home => Self::Home,
            BuiltinIcon::Settings => Self::Settings,
            BuiltinIcon::Bell => Self::Bell,
            BuiltinIcon::Mail => Self::Mail,
            BuiltinIcon::Lock => Self::Lock,
            BuiltinIcon::Unlock => Self::Unlock,
            BuiltinIcon::Eye => Self::Eye,
            BuiltinIcon::EyeOff => Self::EyeOff,
            BuiltinIcon::Edit => Self::Edit,
            BuiltinIcon::Copy => Self::Copy,
            BuiltinIcon::Download => Self::Download,
            BuiltinIcon::Upload => Self::Upload,
            BuiltinIcon::File => Self::File,
            BuiltinIcon::Folder => Self::Folder,
            BuiltinIcon::Trash => Self::Delete,
            BuiltinIcon::RefreshCw => Self::RefreshCw,
            BuiltinIcon::ExternalLink => Self::ExternalLink,
            BuiltinIcon::Menu => Self::Menu,
            BuiltinIcon::Filter => Self::Filter,
            BuiltinIcon::SortAsc => Self::SortAsc,
            BuiltinIcon::SortDesc => Self::SortDesc,
            BuiltinIcon::Play => Self::PlayArrow,
            BuiltinIcon::Pause => Self::Pause,
            BuiltinIcon::VolumeUp => Self::VolumeUp,
            BuiltinIcon::VolumeDown => Self::VolumeDown,
            BuiltinIcon::VolumeOff => Self::VolumeOff,
            BuiltinIcon::Palette => Self::Palette,
            BuiltinIcon::MapPin => Self::MapPin,
            BuiltinIcon::Link => Self::Link,
            BuiltinIcon::Heart => Self::Heart,
        }
    }
}

#[derive(Clone, Copy)]
enum SvgPaint {
    Stroke,
    Fill,
}

#[derive(Clone, Copy)]
struct SvgPath {
    d: &'static str,
    paint: SvgPaint,
}

impl SvgPath {
    const fn stroke(d: &'static str) -> Self {
        Self {
            d,
            paint: SvgPaint::Stroke,
        }
    }

    const fn fill(d: &'static str) -> Self {
        Self {
            d,
            paint: SvgPaint::Fill,
        }
    }
}

type IconCache = Mutex<HashMap<(SvgIconId, Color), Arc<[u8]>>>;

static SVG_ICON_CACHE: OnceLock<IconCache> = OnceLock::new();

pub(crate) fn svg_icon_bytes(icon: SvgIconId, color: Color) -> MediaBytes {
    let cache = SVG_ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("svg icon cache lock poisoned");
    if let Some(bytes) = guard.get(&(icon, color)) {
        return MediaBytes::from_shared(bytes.clone());
    }

    let bytes: Arc<[u8]> = Arc::from(build_svg(icon, color).into_bytes().into_boxed_slice());
    guard.insert((icon, color), bytes.clone());
    MediaBytes::from_shared(bytes)
}

pub(crate) fn svg_icon_source(icon: SvgIconId, color: Color) -> MediaSource {
    MediaSource::bytes(svg_icon_bytes(icon, color))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_svg_icon_texture(
    scene: &mut ScenePrimitives,
    media: &crate::media::MediaManager,
    units: UnitContext,
    icon: SvgIconId,
    color: Color,
    frame: Rect,
    opacity: f32,
    quad: Option<[Point; 4]>,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    if color.a == 0 || opacity <= 0.0 || frame.is_empty() {
        return;
    }
    let source = svg_icon_source(icon, color);
    let layout =
        crate::media::MediaTextureLayout::new(frame, ContentFit::Contain, units.scale_factor());
    let (snapshot, target_frame, raster_request) = media.image_snapshot_for_layout(&source, layout);
    let Some(_raster_request) = raster_request else {
        return;
    };
    let Some(texture) = snapshot.texture.as_ref() else {
        return;
    };

    scene.push_texture(TexturePrimitive {
        texture: texture.clone(),
        media_key: None,
        media_layout: None,
        frame: target_frame,
        quad,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: opacity.clamp(0.0, 1.0),
        clip_rect,
        clip_mask,
    });
}

fn build_svg(icon: SvgIconId, color: Color) -> String {
    let hex = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let alpha = (color.a as f32 / 255.0).clamp(0.0, 1.0);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none">"#
    );
    for path in icon_paths(icon) {
        match path.paint {
            SvgPaint::Stroke => {
                svg.push_str(&format!(
                    r#"<path d="{}" fill="none" stroke="{}" stroke-opacity="{:.3}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    path.d, hex, alpha
                ));
            }
            SvgPaint::Fill => {
                svg.push_str(&format!(
                    r#"<path d="{}" fill="{}" fill-opacity="{:.3}" stroke="none"/>"#,
                    path.d, hex, alpha
                ));
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

macro_rules! svg_paths {
    ($($path:expr),+ $(,)?) => {{
        const PATHS: &[SvgPath] = &[$($path),+];
        PATHS
    }};
}

fn icon_paths(icon: SvgIconId) -> &'static [SvgPath] {
    match icon {
        SvgIconId::ChevronLeft => svg_paths![SvgPath::stroke("M15 6L9 12L15 18")],
        SvgIconId::ChevronRight => svg_paths![SvgPath::stroke("M9 6L15 12L9 18")],
        SvgIconId::ChevronUp => svg_paths![SvgPath::stroke("M6 15L12 9L18 15")],
        SvgIconId::ChevronDown => svg_paths![SvgPath::stroke("M6 9L12 15L18 9")],
        SvgIconId::Close => svg_paths![SvgPath::stroke("M7 7L17 17M17 7L7 17")],
        SvgIconId::Check => svg_paths![SvgPath::stroke("M5 12.5L9.2 16.5L19 7")],
        SvgIconId::MoreHorizontal => svg_paths![SvgPath::fill(
            "M5 12a2 2 0 1 0 .01 0ZM12 12a2 2 0 1 0 .01 0ZM19 12a2 2 0 1 0 .01 0Z",
        )],
        SvgIconId::Search => svg_paths![
            SvgPath::stroke("M11 18a7 7 0 1 0 0-14a7 7 0 0 0 0 14"),
            SvgPath::stroke("M16.5 16.5L21 21"),
        ],
        SvgIconId::Star => svg_paths![SvgPath::fill(
            "M12 3.5L14.7 8.9L20.7 9.8L16.35 14.05L17.4 20L12 17.15L6.6 20L7.65 14.05L3.3 9.8L9.3 8.9L12 3.5Z",
        )],
        SvgIconId::StarHalf => svg_paths![
            SvgPath::stroke(
                "M12 3.5L14.7 8.9L20.7 9.8L16.35 14.05L17.4 20L12 17.15L6.6 20L7.65 14.05L3.3 9.8L9.3 8.9L12 3.5Z",
            ),
            SvgPath::fill("M12 3.5V17.15L6.6 20L7.65 14.05L3.3 9.8L9.3 8.9L12 3.5Z"),
        ],
        SvgIconId::User => svg_paths![
            SvgPath::stroke("M12 12a4 4 0 1 0 0-8a4 4 0 0 0 0 8"),
            SvgPath::stroke("M4.5 21a7.5 5.5 0 0 1 15 0"),
        ],
        SvgIconId::Image => svg_paths![
            SvgPath::stroke("M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v13A1.5 1.5 0 0 1 18.5 20h-13A1.5 1.5 0 0 1 4 18.5v-13Z"),
            SvgPath::stroke("M7 16L10 13L12.5 15.5L15.5 12L20 17"),
            SvgPath::fill("M8 8a1.5 1.5 0 1 0 .01 0Z"),
        ],
        SvgIconId::Plus => svg_paths![SvgPath::stroke("M12 5V19M5 12H19")],
        SvgIconId::Minus => svg_paths![SvgPath::stroke("M5 12H19")],
        SvgIconId::Info => svg_paths![
            SvgPath::stroke("M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18"),
            SvgPath::stroke("M12 11v6"),
            SvgPath::fill("M12 7a1.3 1.3 0 1 0 .01 0Z"),
        ],
        SvgIconId::Success => svg_paths![
            SvgPath::stroke("M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18"),
            SvgPath::stroke("M7.5 12.5L10.5 15.5L16.5 8.5"),
        ],
        SvgIconId::Warning => svg_paths![
            SvgPath::stroke("M12 4L21 20H3L12 4Z"),
            SvgPath::stroke("M12 9v4.8"),
            SvgPath::stroke("M12 16.9h.01"),
        ],
        SvgIconId::Error => svg_paths![
            SvgPath::stroke("M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18"),
            SvgPath::stroke("M8.5 8.5L15.5 15.5M15.5 8.5L8.5 15.5"),
        ],
        SvgIconId::Calendar => svg_paths![
            SvgPath::stroke("M5 5h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z"),
            SvgPath::stroke("M8 3v4M16 3v4M3 10h18"),
        ],
        SvgIconId::Clock => svg_paths![
            SvgPath::stroke("M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18"),
            SvgPath::stroke("M12 7v5l3.5 2"),
        ],
        SvgIconId::Home => svg_paths![
            SvgPath::stroke("M3 11.5L12 4l9 7.5"),
            SvgPath::stroke("M5 10.5V20h5v-5h4v5h5v-9.5"),
        ],
        SvgIconId::Settings => svg_paths![
            SvgPath::stroke("M12 15.5a3.5 3.5 0 1 0 0-7a3.5 3.5 0 0 0 0 7"),
            SvgPath::stroke("M19.4 15a1.8 1.8 0 0 0 .36 1.98l.05.05a2 2 0 0 1-2.83 2.83l-.05-.05a1.8 1.8 0 0 0-1.98-.36a1.8 1.8 0 0 0-1.1 1.66V21a2 2 0 0 1-4 0v-.08a1.8 1.8 0 0 0-1.18-1.68a1.8 1.8 0 0 0-1.98.36l-.05.05a2 2 0 0 1-2.83-2.83l.05-.05a1.8 1.8 0 0 0 .36-1.98a1.8 1.8 0 0 0-1.66-1.1H3a2 2 0 0 1 0-4h.08a1.8 1.8 0 0 0 1.68-1.18a1.8 1.8 0 0 0-.36-1.98l-.05-.05a2 2 0 0 1 2.83-2.83l.05.05a1.8 1.8 0 0 0 1.98.36h.01A1.8 1.8 0 0 0 10.3 3V3a2 2 0 0 1 4 0v.08a1.8 1.8 0 0 0 1.18 1.68a1.8 1.8 0 0 0 1.98-.36l.05-.05a2 2 0 0 1 2.83 2.83l-.05.05a1.8 1.8 0 0 0-.36 1.98a1.8 1.8 0 0 0 1.66 1.1H21a2 2 0 0 1 0 4h-.08a1.8 1.8 0 0 0-1.52.69Z"),
        ],
        SvgIconId::Bell => svg_paths![
            SvgPath::stroke("M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9"),
            SvgPath::stroke("M10 21h4"),
        ],
        SvgIconId::Mail => svg_paths![
            SvgPath::stroke("M4 6h16v12H4z"),
            SvgPath::stroke("M4 7l8 6l8-6"),
        ],
        SvgIconId::Lock => svg_paths![
            SvgPath::stroke("M7 10V7a5 5 0 0 1 10 0v3"),
            SvgPath::stroke("M6 10h12v10H6z"),
        ],
        SvgIconId::Unlock => svg_paths![
            SvgPath::stroke("M7 10V7a5 5 0 0 1 9.5-2.2"),
            SvgPath::stroke("M6 10h12v10H6z"),
        ],
        SvgIconId::Eye => svg_paths![
            SvgPath::stroke("M2.5 12s3.5-6 9.5-6s9.5 6 9.5 6s-3.5 6-9.5 6s-9.5-6-9.5-6Z"),
            SvgPath::stroke("M12 15a3 3 0 1 0 0-6a3 3 0 0 0 0 6"),
        ],
        SvgIconId::EyeOff => svg_paths![
            SvgPath::stroke("M3 3l18 18"),
            SvgPath::stroke("M10.6 5.2A9.6 9.6 0 0 1 12 5c6 0 9.5 7 9.5 7a14.3 14.3 0 0 1-3.1 3.9"),
            SvgPath::stroke("M6.1 6.5A14.2 14.2 0 0 0 2.5 12s3.5 7 9.5 7a9.7 9.7 0 0 0 4-.85"),
            SvgPath::stroke("M9.9 9.9a3 3 0 0 0 4.2 4.2"),
        ],
        SvgIconId::Edit => svg_paths![
            SvgPath::stroke("M12 20h9"),
            SvgPath::stroke("M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1l1-4L16.5 3.5Z"),
        ],
        SvgIconId::Copy => svg_paths![
            SvgPath::stroke("M9 9h10v10H9z"),
            SvgPath::stroke("M5 15H4a1 1 0 0 1-1-1V5a2 2 0 0 1 2-2h9a1 1 0 0 1 1 1v1"),
        ],
        SvgIconId::Download => svg_paths![
            SvgPath::stroke("M12 4v11M8 11l4 4l4-4"),
            SvgPath::stroke("M5 19h14"),
        ],
        SvgIconId::Folder => svg_paths![SvgPath::stroke(
            "M3 6.5A2.5 2.5 0 0 1 5.5 4H10l2 2h6.5A2.5 2.5 0 0 1 21 8.5v8A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5v-10Z",
        )],
        SvgIconId::RefreshCw => svg_paths![
            SvgPath::stroke("M21 12a9 9 0 0 1-15.5 6.2L3 16"),
            SvgPath::stroke("M3 12A9 9 0 0 1 18.5 5.8L21 8"),
            SvgPath::stroke("M21 4v4h-4M3 20v-4h4"),
        ],
        SvgIconId::ExternalLink => svg_paths![
            SvgPath::stroke("M14 4h6v6"),
            SvgPath::stroke("M20 4l-9 9"),
            SvgPath::stroke("M20 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h5"),
        ],
        SvgIconId::Menu => svg_paths![
            SvgPath::stroke("M4 6h16"),
            SvgPath::stroke("M4 12h16"),
            SvgPath::stroke("M4 18h16"),
        ],
        SvgIconId::Filter => svg_paths![SvgPath::stroke(
            "M4 5h16l-6 7v5l-4 2v-7L4 5Z",
        )],
        SvgIconId::SortAsc => svg_paths![
            SvgPath::stroke("M7 17V5"),
            SvgPath::stroke("M3 9l4-4l4 4"),
            SvgPath::stroke("M14 7h6M14 12h4M14 17h2"),
        ],
        SvgIconId::SortDesc => svg_paths![
            SvgPath::stroke("M7 7v12"),
            SvgPath::stroke("M3 15l4 4l4-4"),
            SvgPath::stroke("M14 7h2M14 12h4M14 17h6"),
        ],
        SvgIconId::MapPin => svg_paths![
            SvgPath::stroke("M12 21s7-4.5 7-11a7 7 0 1 0-14 0c0 6.5 7 11 7 11Z"),
            SvgPath::stroke("M12 12a2 2 0 1 0 0-4a2 2 0 0 0 0 4"),
        ],
        SvgIconId::Link => svg_paths![
            SvgPath::stroke("M10 13a5 5 0 0 0 7.1 0l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1"),
            SvgPath::stroke("M14 11a5 5 0 0 0-7.1 0l-2 2a5 5 0 0 0 7.1 7.1l1.1-1.1"),
        ],
        SvgIconId::Heart => svg_paths![SvgPath::stroke(
            "M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.6l-1-1a5.5 5.5 0 0 0-7.8 7.8l1 1L12 21l7.8-7.6l1-1a5.5 5.5 0 0 0 0-7.8Z",
        )],
        SvgIconId::Palette => svg_paths![
            SvgPath::stroke("M12 4a8 8 0 0 0 0 16h1.2a1.8 1.8 0 0 0 1.3-3.05a1.45 1.45 0 0 1 1.05-2.45H17a3 3 0 0 0 3-3C20 7.35 16.42 4 12 4Z"),
            SvgPath::fill("M8 11a1.2 1.2 0 1 0 .01 0ZM10 7.5a1.2 1.2 0 1 0 .01 0ZM14 7.5a1.2 1.2 0 1 0 .01 0ZM16 11a1.2 1.2 0 1 0 .01 0Z"),
        ],
        SvgIconId::Upload => svg_paths![
            SvgPath::stroke("M12 15V4M8 8L12 4L16 8"),
            SvgPath::stroke("M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3"),
        ],
        SvgIconId::File => svg_paths![
            SvgPath::stroke("M7 3h7l5 5v13H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z"),
            SvgPath::stroke("M14 3v6h5M8 14h8M8 18h6"),
        ],
        SvgIconId::Delete => svg_paths![
            SvgPath::stroke("M5 7h14M10 11v6M14 11v6M8 7l1-3h6l1 3M7 7l1 14h8l1-14"),
        ],
        SvgIconId::Pending => svg_paths![
            SvgPath::stroke("M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18"),
            SvgPath::stroke("M12 7v5l3 2"),
        ],
        SvgIconId::CheckboxChecked => svg_paths![
            SvgPath::stroke("M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z"),
            SvgPath::stroke("M7.5 12.5L10.5 15.5L16.5 8.5"),
        ],
        SvgIconId::CheckboxUnchecked => svg_paths![SvgPath::stroke(
            "M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z",
        )],
        SvgIconId::CheckboxIndeterminate => svg_paths![
            SvgPath::stroke("M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z"),
            SvgPath::stroke("M8 12h8"),
        ],
        SvgIconId::PlayArrow => svg_paths![SvgPath::fill("M8 5v14l11-7z")],
        SvgIconId::Pause => svg_paths![SvgPath::fill("M6 5h4v14H6zm8 0h4v14h-4z")],
        SvgIconId::VolumeUp => svg_paths![
            SvgPath::stroke("M4 9v6h4l5 5V4L8 9z"),
            SvgPath::stroke("M15.5 8.5a5 5 0 0 1 0 7M18 6a9 9 0 0 1 0 12"),
        ],
        SvgIconId::VolumeDown => svg_paths![
            SvgPath::stroke("M4 9v6h4l5 5V4L8 9z"),
            SvgPath::stroke("M15.5 8.5a5 5 0 0 1 0 7"),
        ],
        SvgIconId::VolumeOff => svg_paths![SvgPath::stroke("M4 9v6h4l5 5V4L8 9z")],
        SvgIconId::VolumeMute => svg_paths![
            SvgPath::stroke("M4 9v6h4l5 5V4L8 9z"),
            SvgPath::stroke("M17 8l6 8M23 8l-6 8"),
        ],
    }
}
