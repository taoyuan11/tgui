//! CPU text-to-paint bridge.
//!
//! Shaping remains cached in [`TextSystem`], while glyph residency is owned by
//! [`GlyphAtlas`]. This module only joins the two at paint collection time.

use super::paint::{PaintCommand, TextRun};
use crate::core::{Color, DpiScale, ElementId, Error, Rect, ResourceId, Result};
use crate::text::{GlyphAtlas, GlyphContentType, GlyphLookup, TextRequest, TextStyle, TextSystem};

/// Collects raster-backed glyph commands for one text element.
pub struct TextPainter<'a> {
    pub text_system: &'a mut TextSystem,
    pub glyph_atlas: &'a mut GlyphAtlas,
    pub dpi: DpiScale,
}

impl<'a> TextPainter<'a> {
    pub fn new(
        text_system: &'a mut TextSystem,
        glyph_atlas: &'a mut GlyphAtlas,
        dpi: DpiScale,
    ) -> Self {
        Self {
            text_system,
            glyph_atlas,
            dpi,
        }
    }

    pub fn paint(
        &mut self,
        element: ElementId,
        bounds: Rect,
        content: &str,
        opacity: f32,
    ) -> Result<Vec<PaintCommand>> {
        let request = TextRequest::new(content, TextStyle::default())
            .with_width(bounds.size.width.max(0.0))
            .with_dpi(self.dpi);
        let layout = match self.text_system.layout(&request) {
            Ok(layout) => layout,
            Err(_error) if !self.text_system.backend_enabled() => {
                return Ok(fallback_run(element, bounds, content, opacity));
            }
            Err(error) => return Err(error),
        };
        let run_id = ResourceId::from_parts(element.slot(), element.generation());
        let mut commands = Vec::new();
        let mut first_font = None;
        let mut first_page = None;
        let mut glyph_commands = Vec::new();
        for glyph in layout.glyphs() {
            let Some((content_type, raster)) =
                self.text_system.rasterize_glyph(&glyph.raster_key)?
            else {
                continue;
            };
            let glyph_key = glyph.raster_key.glyph_key(content_type)?;
            let lookup = self.glyph_atlas.lookup(glyph_key, run_id)?;
            let placement = match lookup {
                GlyphLookup::Resident(placement) => placement,
                GlyphLookup::Rasterize(request) => {
                    let completion = self.glyph_atlas.complete_raster(request, raster)?;
                    match completion {
                        crate::text::GlyphCompletionOutcome::Ready(completion) => {
                            completion.placement
                        }
                        crate::text::GlyphCompletionOutcome::Stale(request) => {
                            self.glyph_atlas.placement(&request.glyph).ok_or_else(|| {
                                Error::resource(None, "glyph raster completion became stale", true)
                            })?
                        }
                    }
                }
            };
            first_font.get_or_insert(glyph.font);
            first_page.get_or_insert(placement.page);
            let dpi = self.dpi.get() as f32;
            // The raster cache key is built from cosmic-text's quantized
            // physical position. Use that same origin when placing the image
            // so hinting and subpixel bins cannot drift from the cached mask.
            let rect = Rect::from_xywh(
                bounds.origin.x + (glyph.physical_position.0 + placement.left) as f32 / dpi,
                bounds.origin.y + (glyph.physical_position.1 - placement.top) as f32 / dpi,
                placement.pixels.width as f32 / dpi,
                placement.pixels.height as f32 / dpi,
            );
            // Mask glyphs use the text color. Color glyphs are sampled without
            // tinting so emoji and other embedded-color glyphs retain RGBA.
            let color = match content_type {
                GlyphContentType::Mask => Color::rgba8(0, 0, 0, alpha_byte(opacity)),
                GlyphContentType::Color => Color::rgba8(255, 255, 255, alpha_byte(opacity)),
            };
            glyph_commands.push(PaintCommand::DrawGlyphAtlas {
                rect,
                uv: placement.uv,
                page: placement.page,
                color,
            });
        }
        if glyph_commands.is_empty() {
            return Ok(Vec::new());
        }
        commands.push(PaintCommand::DrawTextRun(TextRun {
            layout: run_id,
            font: first_font.expect("glyph command supplied a font"),
            glyph_page: first_page,
            bounds,
            color: Color::rgba8(0, 0, 0, alpha_byte(opacity)),
            glyph_count: u32::try_from(layout.measure().glyph_count).unwrap_or(u32::MAX),
            content_revision: content_fingerprint(content),
        }));
        commands.extend(glyph_commands);
        Ok(commands)
    }
}

fn alpha_byte(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn content_fingerprint(content: &str) -> u64 {
    content
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn fallback_run(
    element: ElementId,
    bounds: Rect,
    content: &str,
    opacity: f32,
) -> Vec<PaintCommand> {
    // A disabled text backend still keeps a stable paint identity for callers
    // that explicitly request rendering in degraded/headless mode.
    vec![PaintCommand::DrawTextRun(TextRun {
        layout: ResourceId::from_parts(element.slot(), element.generation()),
        font: crate::core::FontHandle::from_parts(0, 1),
        glyph_page: None,
        bounds,
        color: Color::rgba8(0, 0, 0, alpha_byte(opacity)),
        glyph_count: u32::try_from(content.chars().count()).unwrap_or(u32::MAX),
        content_revision: content_fingerprint(content),
    })]
}
