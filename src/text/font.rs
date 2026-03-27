use std::cell::RefCell;
use std::sync::Arc;

use cosmic_text::fontdb::{Family, ID, Query, Stretch, Style, Weight};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            bytes,
        });
    }

    pub(crate) fn set_default_font(&mut self, name: impl Into<String>) {
        self.default_font = Some(name.into());
    }

    pub(crate) fn font_sources(&self) -> Vec<cosmic_text::fontdb::Source> {
        self.named_fonts
            .iter()
            .map(|font| cosmic_text::fontdb::Source::Binary(Arc::new(font.bytes.to_vec())))
            .collect()
    }
}

#[derive(Debug, Clone)]
struct NamedFont {
    name: String,
    bytes: &'static [u8],
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
}

impl FontManager {
    pub(crate) fn new(catalog: &FontCatalog) -> Self {
        let mut font_system = FontSystem::new();
        let mut aliases = Vec::with_capacity(catalog.named_fonts.len());
        for font in &catalog.named_fonts {
            let ids = font_system
                .db_mut()
                .load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
                    font.bytes.to_vec(),
                )));
            let actual_family = ids
                .iter()
                .find_map(|id| face_family_name(font_system.db(), *id))
                .unwrap_or_else(|| font.name.clone());
            aliases.push((font.name.clone(), actual_family));
        }

        Self {
            font_system: RefCell::new(font_system),
            aliases,
            default_font: catalog.default_font.clone(),
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
        if text.is_empty() {
            return (32.0, (font_size * 1.6).max(24.0));
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

        (width.max(32.0).ceil(), height.max(24.0).ceil())
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

fn face_family_name(database: &cosmic_text::fontdb::Database, id: ID) -> Option<String> {
    database
        .face(id)
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
}
