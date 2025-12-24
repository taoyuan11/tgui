pub mod button;
pub mod text;
pub mod panel;

// 重新导出，方便外部调用
pub use button::Button;
pub use text::Text;
pub use panel::Panel;
use crate::gui::core::Canvas;

pub enum GuiEvent {
    MouseDown { x: f64, y: f64, button: winit::event::MouseButton },
    MouseUp { x: f64, y: f64, button: winit::event::MouseButton },
    MouseMove { x: f64, y: f64 },
    KeyDown { key: winit::keyboard::KeyCode },
    KeyUp { key: winit::keyboard::KeyCode },
    // 可以根据需要继续添加，例如：
    // FocusGained,
    // FocusLost,
}

pub trait Widget {
    /// 渲染：需要 buffer, 宽度, 偏移量和渲染器
    fn draw(&self, canvas: &mut Canvas, ox: i32, oy: i32);

    /// 通用的事件处理器
    /// 返回 bool 通常用于表示事件是否被“消费”（Consumed），防止事件继续冒泡
    fn handle_event(&mut self, event: &GuiEvent, ox: i32, oy: i32) -> bool;

    /// 布局（可选）：返回组件所需的大小，用于自动布局
    fn size(&self) -> (i32, i32);

    /// 返回子组件列表（核心：支持递归树）
    fn children(&self) -> Option<&[Box<dyn Widget>]> { None }
}