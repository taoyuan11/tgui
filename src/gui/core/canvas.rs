use crate::gui::core::blend_colors;
use crate::gui::style::Color;

pub struct Canvas<'a> {
    buffer: &'a mut [u32],
    width: u32,
    height: u32,

    // 当前绘图状态 (类似于 Context2D)
    pub fill_style: Color,
    pub stroke_style: Color,
    pub line_width: f32,
    pub global_alpha: f32,

    // 变换
    current_tx: i32,
    current_ty: i32,
}


impl <'a> Canvas<'a> {

    pub fn new(buffer: &'a mut [u32], width: u32, height: u32) -> Self {
        Self {
            buffer, width, height,
            fill_style: Color::BLACK,
            stroke_style: Color::BLACK,
            line_width: 1.0,
            global_alpha: 1.0,
            current_tx: 0,
            current_ty: 0,
        }
    }

    pub fn fill(&mut self, color: Color) {
        self.buffer.fill(color.to_u32_no_alpha())
    }

    pub fn save(&mut self) -> (i32, i32, f32) {
        (self.current_tx, self.current_ty, self.global_alpha)
    }

    pub fn restore(&mut self, state: (i32, i32, f32)) {
        self.current_tx = state.0;
        self.current_ty = state.1;
        self.global_alpha = state.2;
    }

    // --- 状态变换 (Transform) ---
    pub fn translate(&mut self, x: i32, y: i32) {
        self.current_tx += x;
        self.current_ty += y;
    }

    // 获取当前的平移偏移
    pub fn get_transform(&self) -> (i32, i32) {
        (self.current_tx, self.current_ty)
    }

    // --- 矩形绘制 (Rectangles) ---
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let (ax, ay) = (x + self.current_tx, y + self.current_ty);
        for row in 0..h {
            for col in 0..w {
                self.draw_pixel(ax + col, ay + row, self.fill_style);
            }
        }
    }

    /// 绘制圆角矩形 (类似 ctx.roundRect + ctx.fill)
    pub fn fill_rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: f32) {
        let (ax, ay) = (x + self.current_tx, y + self.current_ty);
        for row in 0..h {
            for col in 0..w {
                let alpha = self.calc_aa_alpha(col as f32, row as f32, w as f32, h as f32, r);
                if alpha > 0.0 {
                    self.draw_pixel(ax + col, ay + row, self.fill_style.with_alpha_mult(alpha));
                }
            }
        }
    }

    // --- 像素绘制（绝对坐标） ---
    pub(crate) fn draw_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return; }
        let idx = (y * self.width as i32 + x) as usize;

        let alpha = color.a * self.global_alpha;
        if alpha >= 0.999 {
            self.buffer[idx] = color.to_u32_no_alpha();
        } else if alpha > 0.001 {
            let back = self.buffer[idx];
            self.buffer[idx] = blend_colors(back, color.to_u32_no_alpha(), alpha);
        }
    }

    fn calc_aa_alpha(&self, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
        // 确定当前像素是否在四个圆角矩形的判定区
        let (cx, cy) = if x < r && y < r { (r, r) } // 左上
        else if x > w - r && y < r { (w - r, r) } // 右上
        else if x < r && y > h - r { (r, h - r) } // 左下
        else if x > w - r && y > h - r { (w - r, h - r) } // 右下
        else { return 1.0; }; // 处于非圆角区域，完全不透明

        let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();

        // 软化边缘：在半径 r 左右 0.5 像素范围内线性淡出
        if dist < r - 0.5 {
            1.0
        } else if dist > r + 0.5 {
            0.0
        } else {
            (r + 0.5 - dist).clamp(0.0, 1.0)
        }
    }


}