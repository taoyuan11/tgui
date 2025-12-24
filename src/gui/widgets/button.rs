use winit::event::MouseButton;
use crate::gui::core::Canvas;
use crate::gui::style::Color;
use super::{GuiEvent, Text, Widget};

pub struct Button {
    pub x: i32, pub y: i32, pub w: i32, pub h: i32,
    pub label: Text,
    pub color: Color,
    pub border_radius: i32,
    pub on_click: Box<dyn FnMut()>,
    pub is_hovered: bool,
}

impl Button {
    pub fn build(label: &str) -> Self {
        Button {
            x: 0, y: 0, w: 100, h: 30,
            label: Text::build(label),
            color: Color::from_hex("#4CC2FF"),
            border_radius: 5,
            on_click: Box::new(|| {}),
            is_hovered: false,
        }
    }

    pub fn on_click<F>(mut self, callback: F) -> Self where F: FnMut() + 'static {
        self.on_click = Box::new(callback);
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_border_radius(mut self, radius: i32) -> Self {
        self.border_radius = radius;
        self
    }

    pub fn with_position(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn with_size(mut self, w: i32, h: i32) -> Self {
        self.w = w;
        self.h = h;
        self
    }


    fn check_boundary(&self, mx: f64, my: f64, ax: i32, ay: i32) -> bool {
        mx >= ax as f64 && mx <= (ax + self.w) as f64 &&
            my >= ay as f64 && my <= (ay + self.h) as f64
    }
}

impl Widget for Button {
    fn draw(&self, canvas: &mut Canvas, ox: i32, oy: i32) {
        // 1. 保存当前画布状态并平移坐标系 (类似于 ctx.save() + ctx.translate())
        canvas.translate(self.x + ox, self.y + oy);

        // 2. 设置绘图样式
        canvas.fill_style = self.color;

        // 3. 绘制圆角矩形背景 (内部已包含你之前的抗锯齿和 Alpha 混合逻辑)
        let (w, h) = self.size();
        canvas.fill_rounded_rect(0, 0, w, h, self.border_radius as f32);

        // 4. 计算文字居中位置
        let (tw, th) = self.label.size();
        let text_x = (w - tw) / 2;
        let text_y = (h - th) / 2;

        // 5. 直接绘制文字，使用 renderer 而不是 Widget trait
        self.label.renderer.draw_text(
            canvas,
            &self.label.content,
            text_x,
            text_y,
            self.label.size,
            self.label.color
        );

        // 6. 恢复坐标系偏移
        canvas.translate(-(self.x + ox), -(self.y + oy));

    }

    fn handle_event(&mut self, event: &GuiEvent, ox: i32, oy: i32) -> bool {
        let abs_x = ox + self.x;
        let abs_y = oy + self.y;

        match event {
            GuiEvent::MouseMove { x, y } => {
                self.is_hovered = self.check_boundary(*x, *y, abs_x, abs_y);
                false // 鼠标移动通常不拦截，除非你有拖拽逻辑
            }
            GuiEvent::MouseDown { x, y, button: MouseButton::Left } => {
                if self.check_boundary(*x, *y, abs_x, abs_y) {
                    (self.on_click)();
                    return true; // 消费点击事件
                }
                false
            }
            _ => false
        }
    }

    fn size(&self) -> (i32, i32) { (self.w, self.h) }
}