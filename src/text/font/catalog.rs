use std::path::PathBuf;
use std::sync::Arc;

use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight};
use cosmic_text::FontSystem;

use super::platform::face_family_name;

/// 表示文本渲染使用的字重。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
}

impl FontWeight {
    pub const NORMAL: Self = Self::Regular;
    pub const SEMIBOLD: Self = Self::SemiBold;

    pub const fn to_raw(self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
        }
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Regular
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FontCatalog {
    named_fonts: Vec<NamedFont>,
    pub(crate) default_font: Option<String>,
}

impl Default for FontCatalog {
    fn default() -> Self {
        Self {
            named_fonts: Vec::new(),
            default_font: None,
        }
    }
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

/// 表示一段文本最终解析得到的字体结果。
#[derive(Debug, Clone)]
pub struct ResolvedText {
    /// 本次文本布局选中的主字体族名称。
    pub primary_font: String,
}

/// 描述一次文本布局或测量时的字体请求。
#[derive(Debug, Clone)]
pub struct TextFontRequest<'a> {
    /// 优先尝试使用的字体族名称。
    pub preferred_font: Option<&'a str>,
    /// 本次请求使用的字重。
    pub weight: FontWeight,
}

pub(super) fn query_family_name(
    database: &cosmic_text::fontdb::Database,
    name: &str,
    weight: FontWeight,
) -> Option<String> {
    let families = [Family::Name(name)];
    let query = Query {
        families: &families,
        weight: Weight(weight.to_raw()),
        stretch: Stretch::Normal,
        style: Style::Normal,
    };

    database
        .query(&query)
        .and_then(|id| face_family_name(database, id))
        .or_else(|| {
            database.faces().find_map(|face| {
                face.families
                    .iter()
                    .find(|(family, _)| family.eq_ignore_ascii_case(name))
                    .map(|(family, _)| family.clone())
            })
        })
}

pub(super) fn default_family_name(
    database: &cosmic_text::fontdb::Database,
    weight: FontWeight,
) -> Option<String> {
    let families = [Family::SansSerif];
    let query = Query {
        families: &families,
        weight: Weight(weight.to_raw()),
        stretch: Stretch::Normal,
        style: Style::Normal,
    };

    database
        .query(&query)
        .and_then(|id| face_family_name(database, id))
}
