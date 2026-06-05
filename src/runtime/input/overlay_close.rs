use crate::ui::widget::Point;

use super::BoundRuntimeHandler;
use crate::runtime::state::{FocusedWidget, TooltipDismissReason};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn close_overlay_handle(&mut self, handle: &crate::runtime::overlay::OverlayCloseHandle<VM>) {
        if let Some(command) = &handle.on_close {
            self.execute_value_command(command, handle.close_value);
            return;
        }

        if handle.layer == crate::runtime::overlay::OverlayLayer::Menu {
            if let Some(source_widget_id) = handle.source_widget_id {
                if self.close_context_menu(source_widget_id) {
                    return;
                }
                let _ = self.set_menu_open_state(source_widget_id, false);
                return;
            }
        }

        let _ = self.set_select_open_state(
            crate::ui::widget::WidgetId::from_raw(handle.overlay_id.0),
            false,
            None,
        );
    }

    pub(in crate::runtime) fn restore_overlay_focus_if_needed(
        &mut self,
        widget_id: crate::ui::widget::WidgetId,
    ) -> bool {
        let computed = self.computed_scene().clone();
        let target = computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| {
                let focus = region.focus.as_ref()?;
                (focus.widget_id == widget_id).then(|| FocusedWidget {
                    widget_id: focus.widget_id,
                    scope_path: focus.scope_path.clone(),
                    on_blur: focus.on_blur.clone(),
                })
            });
        if let Some(target) = target {
            self.update_focus(Some(target), None, true);
            true
        } else {
            false
        }
    }

    pub(super) fn backdrop_focus_restore_target(
        &self,
        widget_id: crate::ui::widget::WidgetId,
    ) -> Option<crate::ui::widget::WidgetId> {
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        layout.all_widget_ids().find_map(|id| {
            let resolved = layout.resolved_widget(id)?;
            if let Some(modal) = resolved.modal.as_ref() {
                if modal.open.resolve() && modal.backdrop_widget_id == widget_id {
                    return modal.return_focus_to;
                }
            }
            if let Some(drawer) = resolved.drawer.as_ref() {
                if drawer.open.resolve() && drawer.backdrop_widget_id == widget_id {
                    return drawer.return_focus_to;
                }
            }
            None
        })
    }

    /// 在指针按下时调用：所有 `close_on_outside_click=true` 的浮层，若 click_point 不在
    /// 浮层 rect 内，则触发其 `on_close`。
    ///
    /// 不修改 `overlay_close_handlers` 自身——下一帧 collect 时若浮层已关闭，
    /// 消费者不会再调 `emit_overlay`，handler 自然消失（清空-重建语义）。
    pub(in crate::runtime) fn consume_overlay_close_handlers_outside_click(
        &mut self,
        click_point: Point,
    ) -> Option<crate::ui::widget::WidgetId> {
        if self.dismiss_active_tooltip(TooltipDismissReason::OutsideClick) {
            return None;
        }
        let handlers: Vec<_> = self
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .filter(|h| h.close_on_outside_click)
            .cloned()
            .collect();

        let mut focus_restore = None;
        for handle in handlers.iter().rev() {
            if handle.rect.contains(click_point) {
                break;
            }
            self.close_overlay_handle(handle);
            if focus_restore.is_none() {
                focus_restore = handle.return_focus_to;
            }
        }
        focus_restore
    }

    /// Esc 按键时调用：消费栈顶的 `close_on_escape=true` 浮层并触发其 `on_close`。
    /// 若有任一浮层被消费，返回 true（调用方可据此决定是否吞掉按键）。
    pub(in crate::runtime) fn consume_topmost_overlay_close_handler_escape(&mut self) -> bool {
        if self.dismiss_active_tooltip(TooltipDismissReason::Escape) {
            return true;
        }
        let topmost = self
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .rev()
            .find(|h| h.close_on_escape)
            .cloned();

        if let Some(handle) = topmost {
            self.close_overlay_handle(&handle);
            if let Some(widget_id) = handle.return_focus_to {
                let _ = self.restore_overlay_focus_if_needed(widget_id);
            }
            true
        } else {
            false
        }
    }
}
