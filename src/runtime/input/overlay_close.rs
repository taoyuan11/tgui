use crate::ui::widget::Point;

use super::BoundRuntimeHandler;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    /// 在指针按下时调用：所有 `close_on_outside_click=true` 的浮层，若 click_point 不在
    /// 浮层 rect 内，则触发其 `on_close`。
    ///
    /// 不修改 `overlay_close_handlers` 自身——下一帧 collect 时若浮层已关闭，
    /// 消费者不会再调 `emit_overlay`，handler 自然消失（清空-重建语义）。
    pub(in crate::runtime) fn consume_overlay_close_handlers_outside_click(
        &mut self,
        click_point: Point,
    ) {
        let handlers: Vec<_> = self
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .filter(|h| h.close_on_outside_click)
            .cloned()
            .collect();

        for handle in handlers.iter().rev() {
            if !handle.rect.contains(click_point) {
                if let Some(command) = &handle.on_close {
                    self.execute_command(command);
                }
            }
        }
    }

    /// Esc 按键时调用：消费栈顶的 `close_on_escape=true` 浮层并触发其 `on_close`。
    /// 若有任一浮层被消费，返回 true（调用方可据此决定是否吞掉按键）。
    pub(in crate::runtime) fn consume_topmost_overlay_close_handler_escape(&mut self) -> bool {
        let topmost = self
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .rev()
            .find(|h| h.close_on_escape)
            .cloned();

        if let Some(handle) = topmost {
            if let Some(command) = &handle.on_close {
                self.execute_command(command);
            }
            true
        } else {
            false
        }
    }
}
