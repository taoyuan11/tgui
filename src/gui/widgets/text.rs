use crate::gui::core::Canvas;
use crate::gui::renderers::text_renderer::SharedRenderer;
use crate::gui::renderers::TextRenderer;
use crate::gui::style::Color;
use crate::gui::widgets::{GuiEvent, Widget};

#[derive(Clone)]
pub struct Text {
    pub x: i32, pub y: i32,
    pub content: String,
    pub size: f32,
    pub color: Color,
    pub renderer: SharedRenderer,
}

impl Text {
    pub fn build(content: &str) -> Self {
        Self {
            x: 0, y: 0,
            content: content.to_string(),
            size: 14.0,
            color: Color::from_hex("#ffffff"),
            renderer: TextRenderer::global(),
        }
    }
}


impl Widget for Text {
    fn draw(&self, canvas: &mut Canvas, ox: i32, oy: i32) {
        // 直接调用共享渲染器的 draw_text，不需要关心字体加载
        // ox, oy 是父组件传入的偏移，self.x, self.y 是 Text 自身的位置
        self.renderer.draw_text(
            canvas,
            &self.content,
            ox + self.x,
            oy + self.y,
            self.size,
            self.color
        );
    }

    fn handle_event(&mut self, event: &GuiEvent, ox: i32, oy: i32) -> bool {
        false
    }

    fn size(&self) -> (i32, i32) {
        use ab_glyph::{Font, PxScale, ScaleFont};

        // 1. 设置缩放
        let px_scale = PxScale::from(self.size);
        let scaled_font = self.renderer.font.as_scaled(px_scale);

        // 2. 计算宽度：累加每个字符的水平推进值 (h_advance)
        let width: f32 = self.content
            .chars()
            .map(|c| scaled_font.h_advance(self.renderer.font.glyph_id(c)))
            .sum();

        // 3. 计算高度：使用字体的上行高度 (ascent) 减去 下行高度 (descent)
        // 注意：descent 通常是负值，所以减去它等于加上绝对值
        let height = scaled_font.ascent() - scaled_font.descent();

        (width.ceil() as i32, height.ceil() as i32)
    }
}