use crate::gui::core::Canvas;
use super::{GuiEvent, Widget};

pub struct Panel {
    pub x: i32,
    pub y: i32,
    pub children: Vec<Box<dyn Widget>>,
}

impl Panel {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            children: vec![],
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }
}

impl Widget for Panel {
    fn draw(&self, canvas: &mut Canvas, ox: i32, oy: i32) {
        for child in &self.children {
            child.draw(canvas, ox + self.x, oy + self.y);
        }
    }

    fn handle_event(&mut self, event: &GuiEvent, ox: i32, oy: i32) -> bool {
        let absolute_x = ox + self.x;
        let absolute_y = oy + self.y;

        // 倒序遍历子组件（后渲染的在最上面，应该先接收事件）
        for child in self.children.iter_mut().rev() {
            if child.handle_event(event, absolute_x, absolute_y) {
                return true; // 如果子组件消费了事件，直接返回
            }
        }
        false
    }

    fn size(&self) -> (i32, i32) { (0, 0) } // 容器大小通常由内容决定
}