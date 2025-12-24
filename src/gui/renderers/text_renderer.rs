use crate::gui::core::Canvas;
use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use glyph_brush_draw_cache::DrawCache;
use std::sync::{Arc, OnceLock, RwLock};
use crate::gui::style::Color;

pub struct TextRenderer {
    pub font: FontArc,
    cache: RwLock<DrawCache>,
}

pub type SharedRenderer = Arc<TextRenderer>;
static GLOBAL_RENDERER: OnceLock<SharedRenderer> = OnceLock::new();

impl TextRenderer {
    pub fn global() -> SharedRenderer {
        GLOBAL_RENDERER
            .get_or_init(|| Arc::new(Self::with_system_font()))
            .clone()
    }

    /// 自动从系统查找默认字体
    pub fn with_system_font() -> Self {
        let font_families = [
            FamilyName::Title("Microsoft YaHei".into()),
            FamilyName::Title("PingFang SC".into()),
            FamilyName::Title("Noto Sans CJK SC".into()),
            FamilyName::Title("Source Han Sans SC".into()),
            FamilyName::SansSerif, // 最后保底使用系统通用无衬线字体
        ];

        let handle = SystemSource::new()
            .select_best_match(&font_families, &Properties::new())
            .expect("未能找到任何合适的系统字体");

        // 2. 加载字体数据 (保持不变)
        let font_data = match handle {
            font_kit::handle::Handle::Path { path, .. } => {
                std::fs::read(path).expect("读取系统字体文件失败")
            }
            font_kit::handle::Handle::Memory { bytes, .. } => bytes.to_vec(),
        };

        let font = FontArc::try_from_vec(font_data).expect("解析字体失败");
        Self {
            font,
            cache: RwLock::new(DrawCache::builder().build()),
        }
    }

    pub fn draw_text(
        &self,
        canvas: &mut Canvas,
        text: &str,
        x: i32,
        y: i32,
        scale: f32,
        color: Color,
    ) {
        let px_scale = PxScale::from(scale);
        let scaled_font = self.font.as_scaled(px_scale);

        // 1. 计算基线 (注意：这里的 x, y 是相对于当前 canvas 坐标系的局部坐标)
        let mut caret = ab_glyph::point(x as f32, y as f32 + scaled_font.ascent());

        let mut glyphs = Vec::new();
        for c in text.chars() {
            let glyph_id = self.font.glyph_id(c);
            let glyph = Glyph {
                id: glyph_id,
                scale: px_scale,
                position: caret,
            };
            caret.x += scaled_font.h_advance(glyph_id);
            glyphs.push(glyph);
        }

        // 2. 缓存维护 (保持不变)
        {
            let mut cache = self.cache.write().unwrap();
            for g in &glyphs {
                cache.queue_glyph(0, g.clone());
            }
            let fonts = [self.font.clone()];
            let _ = cache.cache_queued(&fonts, |_rect, _data| {});
        }

        // 3. 绘制逻辑：通过 canvas 提供的像素接口
        // 获取当前画布的平移偏移
        let (tx, ty) = canvas.get_transform();
        
        for glyph in glyphs {
            if let Some(outlined) = self.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|px_x, px_y, alpha| {
                    if alpha <= 0.001 {
                        return;
                    }

                    // 计算像素位置（相对于 glyph 的位置）
                    let sx = bounds.min.x.floor() as i32 + px_x as i32;
                    let sy = bounds.min.y.floor() as i32 + px_y as i32;

                    // 加上平移偏移转换为绝对坐标
                    canvas.draw_pixel(sx + tx, sy + ty, color.with_alpha_mult(alpha));
                });
            }
        }
    }

    pub fn blend(&self, back: u32, front: u32, alpha: f32) -> u32 {
        if alpha >= 0.99 {
            return front;
        }
        if alpha <= 0.01 {
            return back;
        }
        let rb = ((back >> 16) & 0xFF) as f32;
        let gb = ((back >> 8) & 0xFF) as f32;
        let bb = (back & 0xFF) as f32;
        let rf = ((front >> 16) & 0xFF) as f32;
        let gf = ((front >> 8) & 0xFF) as f32;
        let bf = (front & 0xFF) as f32;
        let r = (rb * rb * (1.0 - alpha) + rf * rf * alpha).sqrt();
        let g = (gb * gb * (1.0 - alpha) + gf * gf * alpha).sqrt();
        let b = (bb * bb * (1.0 - alpha) + bf * bf * alpha).sqrt();
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}
