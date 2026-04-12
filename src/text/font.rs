use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight, ID};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMIBOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FontCatalog {
    named_fonts: Vec<NamedFont>,
    default_font: Option<String>,
}

impl FontCatalog {
    pub(crate) fn register_font(&mut self, name: impl Into<String>, bytes: &'static [u8]) {
        self.named_fonts.push(NamedFont {
            name: name.into(),
            source: FontSource::Binary(bytes),
        });
    }

    pub(crate) fn register_font_file(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) {
        self.named_fonts.push(NamedFont {
            name: name.into(),
            source: FontSource::File(path.into()),
        });
    }

    pub(crate) fn set_default_font(&mut self, name: impl Into<String>) {
        self.default_font = Some(name.into());
    }

    pub(crate) fn configure_font_system(
        &self,
        font_system: &mut FontSystem,
    ) -> Vec<(String, String)> {
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        load_mobile_system_fonts(font_system.db_mut());

        let mut aliases = Vec::with_capacity(self.named_fonts.len());
        for font in &self.named_fonts {
            let ids = font_system.db_mut().load_font_source(match &font.source {
                FontSource::Binary(bytes) => {
                    cosmic_text::fontdb::Source::Binary(Arc::new(bytes.to_vec()))
                }
                FontSource::File(path) => cosmic_text::fontdb::Source::File(path.clone().into()),
            });
            let actual_family = ids
                .iter()
                .find_map(|id| face_family_name(font_system.db(), *id))
                .unwrap_or_else(|| font.name.clone());
            aliases.push((font.name.clone(), actual_family));
        }

        aliases
    }
}

#[derive(Debug, Clone)]
struct NamedFont {
    name: String,
    source: FontSource,
}

#[derive(Debug, Clone)]
enum FontSource {
    Binary(&'static [u8]),
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ResolvedText {
    pub primary_font: String,
}

#[derive(Debug, Clone)]
pub struct TextFontRequest<'a> {
    pub preferred_font: Option<&'a str>,
    pub weight: FontWeight,
}

pub(crate) struct FontManager {
    font_system: RefCell<FontSystem>,
    aliases: Vec<(String, String)>,
    default_font: Option<String>,
    measure_cache: RefCell<HashMap<TextMeasureKey, (f32, f32)>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextMeasureKey {
    text: String,
    preferred_font: Option<String>,
    weight: FontWeight,
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
}

impl FontManager {
    pub(crate) fn new(catalog: &FontCatalog) -> Self {
        let mut font_system = FontSystem::new();
        let aliases = catalog.configure_font_system(&mut font_system);

        Self {
            font_system: RefCell::new(font_system),
            aliases,
            default_font: catalog.default_font.clone(),
            measure_cache: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn resolve_text(&self, _text: &str, request: TextFontRequest<'_>) -> ResolvedText {
        let _weight = request.weight;
        let preferred = request
            .preferred_font
            .and_then(|name| self.resolve_family_name(name, request.weight));

        ResolvedText {
            primary_font: preferred
                .or_else(|| {
                    self.default_font
                        .as_deref()
                        .and_then(|name| self.resolve_family_name(name, request.weight))
                })
                .or_else(|| self.system_default_family(request.weight))
                .unwrap_or_else(|| "sans-serif".to_string()),
        }
    }

    pub(crate) fn measure_text(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        let measured = self.measure_text_raw(text, request, font_size, line_height, letter_spacing);
        (measured.0.max(32.0).ceil(), measured.1.max(24.0).ceil())
    }

    pub(crate) fn measure_text_raw(
        &self,
        text: &str,
        request: TextFontRequest<'_>,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, line_height.ceil());
        }

        let cache_key = TextMeasureKey {
            text: text.to_string(),
            preferred_font: request.preferred_font.map(ToString::to_string),
            weight: request.weight,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
        };
        if let Some(cached) = self.measure_cache.borrow().get(&cache_key) {
            return *cached;
        }

        let resolved = self.resolve_text(text, request.clone());
        let mut font_system = self.font_system.borrow_mut();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, line_height));
        buffer.set_size(&mut font_system, None, None);
        buffer.set_wrap(&mut font_system, Wrap::None);
        let attrs = Attrs::new()
            .family(Family::Name(&resolved.primary_font))
            .weight(Weight(request.weight.0))
            .letter_spacing(letter_spacing / font_size.max(1.0));
        buffer.set_text(&mut font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);

        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }

        let measured = (width.max(0.0).ceil(), height.max(line_height).ceil());
        let mut cache = self.measure_cache.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(cache_key, measured);
        measured
    }

    fn resolve_family_name(&self, name: &str, weight: FontWeight) -> Option<String> {
        if let Some((_, family)) = self.aliases.iter().find(|(alias, _)| alias == name) {
            return Some(family.clone());
        }

        let families = [Family::Name(name)];
        let query = Query {
            families: &families,
            weight: Weight(weight.0),
            stretch: Stretch::Normal,
            style: Style::Normal,
        };

        self.font_system
            .borrow()
            .db()
            .query(&query)
            .and_then(|id| face_family_name(self.font_system.borrow().db(), id))
            .or_else(|| {
                self.font_system.borrow().db().faces().find_map(|face| {
                    face.families
                        .iter()
                        .find(|(family, _)| family.eq_ignore_ascii_case(name))
                        .map(|(family, _)| family.clone())
                })
            })
    }

    fn system_default_family(&self, weight: FontWeight) -> Option<String> {
        let families = [Family::SansSerif];
        let query = Query {
            families: &families,
            weight: Weight(weight.0),
            stretch: Stretch::Normal,
            style: Style::Normal,
        };

        self.font_system
            .borrow()
            .db()
            .query(&query)
            .and_then(|id| face_family_name(self.font_system.borrow().db(), id))
    }
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn load_mobile_system_fonts(database: &mut cosmic_text::fontdb::Database) {
    for path in mobile_font_dirs() {
        let path = std::path::Path::new(path);
        if path.exists() {
            database.load_fonts_dir(path);
        }
    }

    let sans_family = first_matching_family(database, mobile_sans_candidates())
        .or_else(|| first_loaded_family(database));

    let serif_family =
        first_matching_family(database, mobile_serif_candidates()).or_else(|| sans_family.clone());

    let monospace_family = first_matching_family(database, mobile_monospace_candidates())
        .or_else(|| sans_family.clone());

    if let Some(family) = sans_family {
        database.set_sans_serif_family(family.clone());
        database.set_cursive_family(family.clone());
        database.set_fantasy_family(family);
    }
    if let Some(family) = serif_family {
        database.set_serif_family(family);
    }
    if let Some(family) = monospace_family {
        database.set_monospace_family(family);
    }
}

#[cfg(target_os = "android")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system_ext/fonts",
        "/product/fonts",
        "/vendor/fonts",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system/etc/fonts",
        "/system/fonts/visibility",
        "/data/service/el1/public/font",
    ]
}

#[cfg(target_os = "android")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "Roboto",
        "Roboto Static",
        "Roboto Flex",
        "Droid Sans",
        "Noto Sans CJK SC",
        "Noto Sans CJK TC",
        "Noto Sans CJK JP",
        "Noto Sans CJK KR",
        "Noto Sans",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans SC",
        "HarmonyOS Sans",
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Noto Sans",
    ]
}

#[cfg(target_os = "android")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif",
        "Noto Serif CJK SC",
        "Noto Serif CJK TC",
        "Noto Serif CJK JP",
        "Noto Serif CJK KR",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif CJK SC",
        "Noto Serif SC",
        "Noto Serif",
        "HarmonyOS Sans SC",
    ]
}

#[cfg(target_os = "android")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "Droid Sans Mono",
        "Cutive Mono",
        "Roboto Mono",
        "Noto Sans Mono",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans Mono",
        "Roboto Mono",
        "Noto Sans Mono",
        "HarmonyOS Sans SC",
    ]
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn first_matching_family(
    database: &cosmic_text::fontdb::Database,
    candidates: &[&str],
) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        database.faces().find_map(|face| {
            face.families
                .iter()
                .find(|(family, _)| family.eq_ignore_ascii_case(candidate))
                .map(|(family, _)| family.clone())
        })
    })
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn first_loaded_family(database: &cosmic_text::fontdb::Database) -> Option<String> {
    database
        .faces()
        .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
}

fn face_family_name(database: &cosmic_text::fontdb::Database, id: ID) -> Option<String> {
    database
        .face(id)
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
}
