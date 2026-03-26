use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont, point};
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use crate::graphics::TextStyle;

#[derive(Clone)]
pub(crate) struct RasterizedText {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Default)]
struct FontRegistry {
    fonts: HashMap<String, FontArc>,
    load_order: Vec<String>,
    default_font_name: Option<String>,
}

static FONT_REGISTRY: OnceLock<RwLock<FontRegistry>> = OnceLock::new();
static SYSTEM_FONT: OnceLock<FontArc> = OnceLock::new();
static SYSTEM_NAMED_FONTS: OnceLock<RwLock<HashMap<String, FontArc>>> = OnceLock::new();

fn registry() -> &'static RwLock<FontRegistry> {
    FONT_REGISTRY.get_or_init(|| RwLock::new(FontRegistry::default()))
}

fn system_font() -> &'static FontArc {
    SYSTEM_FONT.get_or_init(|| load_system_default_font().expect("failed to load system default font"))
}

fn system_named_fonts() -> &'static RwLock<HashMap<String, FontArc>> {
    SYSTEM_NAMED_FONTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn load_system_default_font() -> Result<FontArc, String> {
    let families = [
        FamilyName::Title("Microsoft YaHei".into()),
        FamilyName::Title("PingFang SC".into()),
        FamilyName::Title("Noto Sans CJK SC".into()),
        FamilyName::Title("Source Han Sans SC".into()),
        FamilyName::SansSerif,
    ];
    let handle = SystemSource::new()
        .select_best_match(&families, &Properties::new())
        .map_err(|e| format!("failed to resolve system default font: {e}"))?;

    let font_data = match handle {
        font_kit::handle::Handle::Path { path, .. } => {
            std::fs::read(path).map_err(|e| format!("failed to read system font file: {e}"))?
        }
        font_kit::handle::Handle::Memory { bytes, .. } => bytes.to_vec(),
    };

    FontArc::try_from_vec(font_data)
        .map_err(|e| format!("failed to parse system default font bytes: {e}"))
}

fn load_system_font_by_name(name: &str) -> Option<FontArc> {
    if let Ok(cache) = system_named_fonts().read()
        && let Some(font) = cache.get(name)
    {
        return Some(font.clone());
    }

    let families = [FamilyName::Title(name.into())];
    let handle = SystemSource::new()
        .select_best_match(&families, &Properties::new())
        .ok()?;

    let font_data = match handle {
        font_kit::handle::Handle::Path { path, .. } => std::fs::read(path).ok()?,
        font_kit::handle::Handle::Memory { bytes, .. } => bytes.to_vec(),
    };
    let font = FontArc::try_from_vec(font_data).ok()?;

    if let Ok(mut cache) = system_named_fonts().write() {
        cache.insert(name.to_string(), font.clone());
    }
    Some(font)
}

pub fn load_font(name: &str, bytes: &[u8]) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("font name must not be empty".to_string());
    }
    let font = FontArc::try_from_vec(bytes.to_vec())
        .map_err(|e| format!("failed to parse font bytes: {e}"))?;
    if let Ok(mut fonts) = registry().write() {
        if !fonts.fonts.contains_key(name) {
            fonts.load_order.push(name.to_string());
        }
        fonts.fonts.insert(name.to_string(), font);
        Ok(())
    } else {
        Err("failed to acquire font registry lock".to_string())
    }
}

pub fn set_default_font(name: &str) -> Result<(), String> {
    if let Ok(mut fonts) = registry().write() {
        if fonts.fonts.contains_key(name) {
            fonts.default_font_name = Some(name.to_string());
            Ok(())
        } else if let Some(system_named) = load_system_font_by_name(name) {
            fonts
                .fonts
                .insert(format!("system::{name}"), system_named);
            fonts.default_font_name = Some(format!("system::{name}"));
            Ok(())
        } else {
            eprintln!(
                "[tgui] warning: default font '{name}' not found; falling back to system default font"
            );
            fonts.default_font_name = None;
            Ok(())
        }
    } else {
        Err("failed to acquire font registry lock".to_string())
    }
}

fn resolve_font(style: &TextStyle) -> FontArc {
    if let Ok(fonts) = registry().read() {
        if let Some(name) = &style.font_name {
            if let Some(font) = fonts.fonts.get(name) {
                return font.clone();
            }
        }
        if let Some(default_name) = &fonts.default_font_name {
            if let Some(font) = fonts.fonts.get(default_name) {
                return font.clone();
            }
        }
    }
    system_font().clone()
}

fn candidate_fonts(style: &TextStyle) -> Vec<FontArc> {
    let mut result = Vec::new();
    if let Ok(fonts) = registry().read() {
        let mut seen_names: HashMap<String, ()> = HashMap::new();
        if let Some(name) = &style.font_name {
            if let Some(font) = fonts.fonts.get(name) {
                result.push(font.clone());
                seen_names.insert(name.clone(), ());
            } else if let Some(system_named) = load_system_font_by_name(name) {
                result.push(system_named);
                seen_names.insert(format!("system::{name}"), ());
            }
        }
        if let Some(default_name) = &fonts.default_font_name {
            if let Some(font) = fonts.fonts.get(default_name)
                && !seen_names.contains_key(default_name)
            {
                seen_names.insert(default_name.clone(), ());
                result.push(font.clone());
            } else if !seen_names.contains_key(default_name) {
                let plain_name = default_name.strip_prefix("system::").unwrap_or(default_name);
                if let Some(system_named) = load_system_font_by_name(plain_name) {
                    seen_names.insert(default_name.clone(), ());
                    result.push(system_named);
                }
            }
        }

        result.push(system_font().clone());

        for name in &fonts.load_order {
            if seen_names.contains_key(name) {
                continue;
            }
            if let Some(font) = fonts.fonts.get(name) {
                seen_names.insert(name.clone(), ());
                result.push(font.clone());
            }
        }
    } else {
        result.push(system_font().clone());
    }

    if result.is_empty() {
        result.push(resolve_font(style));
    }
    result
}

fn select_font_for_char(candidates: &[FontArc], c: char) -> FontArc {
    for font in candidates {
        let glyph_id = font.glyph_id(c);
        if glyph_id.0 != 0 {
            return font.clone();
        }
    }
    candidates[0].clone()
}

pub(crate) fn measure_text(content: &str, style: &TextStyle) -> (f32, f32) {
    let candidates = candidate_fonts(style);
    let px_scale = PxScale::from(style.font_size.max(1.0));

    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    let mut count = 0_u32;
    for c in content.chars() {
        let font = select_font_for_char(&candidates, c);
        let scaled_font = font.as_scaled(px_scale);
        width += scaled_font.h_advance(font.glyph_id(c));
        height = height.max(scaled_font.ascent() - scaled_font.descent());
        count += 1;
    }
    if count > 1 {
        width += style.letter_spacing * (count - 1) as f32;
    }

    if let Some(line_height) = style.line_height {
        height = height.max(line_height);
    }
    (width.max(1.0), height.max(1.0))
}

pub(crate) fn rasterize_text(content: &str, style: &TextStyle) -> RasterizedText {
    let candidates = candidate_fonts(style);
    let px_scale = PxScale::from(style.font_size.max(1.0));

    let mut chosen = Vec::new();
    let mut max_ascent = 0.0_f32;
    let mut max_descent = 0.0_f32;
    for c in content.chars() {
        let font = select_font_for_char(&candidates, c);
        let scaled = font.as_scaled(px_scale);
        max_ascent = max_ascent.max(scaled.ascent());
        max_descent = max_descent.max(-scaled.descent());
        chosen.push((c, font));
    }
    if max_ascent <= 0.0 {
        let fallback = resolve_font(style);
        let scaled = fallback.as_scaled(px_scale);
        max_ascent = scaled.ascent().max(1.0);
        max_descent = (-scaled.descent()).max(0.0);
    }

    let mut caret_x = 0.0_f32;
    let baseline_y = max_ascent;
    let mut outlines = Vec::new();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (c, font) in chosen {
        let scaled_font = font.as_scaled(px_scale);
        let glyph_id = font.glyph_id(c);
        let glyph = Glyph {
            id: glyph_id,
            scale: px_scale,
            position: point(caret_x, baseline_y),
        };
        caret_x += scaled_font.h_advance(glyph_id) + style.letter_spacing;

        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            min_x = min_x.min(bounds.min.x.floor());
            min_y = min_y.min(bounds.min.y.floor());
            max_x = max_x.max(bounds.max.x.ceil());
            max_y = max_y.max(bounds.max.y.ceil());
            outlines.push(outline);
        }
    }

    if outlines.is_empty() {
        let width = caret_x.ceil().max(1.0) as u32;
        let measured_height = if let Some(line_height) = style.line_height {
            line_height.max(max_ascent + max_descent)
        } else {
            max_ascent + max_descent
        };
        let height = measured_height.ceil().max(1.0) as u32;
        return RasterizedText {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        };
    }

    let min_x_i = min_x as i32;
    let min_y_i = min_y as i32;
    let width = (max_x - min_x).max(1.0) as u32;
    let height = (max_y - min_y).max(1.0) as u32;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];

    for outline in outlines {
        let bounds = outline.px_bounds();
        let glyph_min_x = bounds.min.x.floor() as i32;
        let glyph_min_y = bounds.min.y.floor() as i32;
        outline.draw(|x, y, alpha| {
            if alpha <= 0.0 {
                return;
            }
            let dst_x = glyph_min_x + x as i32 - min_x_i;
            let dst_y = glyph_min_y + y as i32 - min_y_i;
            if dst_x < 0 || dst_y < 0 || dst_x >= width as i32 || dst_y >= height as i32 {
                return;
            }

            let idx = ((dst_y as u32 * width + dst_x as u32) * 4) as usize;
            let a = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            pixels[idx] = 255;
            pixels[idx + 1] = 255;
            pixels[idx + 2] = 255;
            pixels[idx + 3] = a;
        });
    }

    RasterizedText {
        width,
        height,
        pixels,
    }
}
