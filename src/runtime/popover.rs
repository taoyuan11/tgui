use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn resolve_active_hover_popover(&mut self) -> Option<WidgetId> {
        let resolved = self.resolve_active_hover_popover_from_cache();
        // 命令执行（如点击浮层内的交互元素）会把 `cached_scene` 硬清空，这一帧无法从上一帧
        // overlay rect 推断光标是否仍在浮层内，`resolve_active_hover_popover_from_cache` 会返回
        // None，浮层因此误关闭。点击不移动光标，复用上一次解析出的锚点即可让浮层在重建帧里存活；
        // 下一帧 cache 恢复后会重新正常解析。
        if resolved.is_some() {
            self.hover_popover_anchor = resolved;
            resolved
        } else if self.cached_scene.is_none() && self.cursor_position.is_some() {
            self.hover_popover_anchor
        } else {
            self.hover_popover_anchor = None;
            None
        }
    }

    fn resolve_active_hover_popover_from_cache(&mut self) -> Option<WidgetId> {
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let cursor = self.cursor_position;
        for hovered in self.hovered_widgets.iter().rev() {
            let HoverTargetId::Widget(widget_id) = hovered.target_id else {
                continue;
            };
            let Some(resolved) = layout.resolved_widget(widget_id) else {
                continue;
            };
            let Some(popover) = resolved.popover.as_ref() else {
                continue;
            };
            if popover.disabled.resolve() {
                continue;
            }
            if !popover.trigger_mode.allows_hover() {
                continue;
            }
            return Some(widget_id);
        }
        let Some(cursor) = cursor else {
            return None;
        };
        for handle in cached.computed.overlay_close_handlers.iter().rev() {
            if handle.layer != crate::runtime::overlay::OverlayLayer::Popover {
                continue;
            }
            let Some(widget_id) = handle.source_widget_id else {
                continue;
            };
            if !handle.rect.contains(cursor) {
                continue;
            }
            let Some(resolved) = layout.resolved_widget(widget_id) else {
                continue;
            };
            let Some(popover) = resolved.popover.as_ref() else {
                continue;
            };
            if popover.disabled.resolve() || !popover.trigger_mode.allows_hover() {
                continue;
            }
            return Some(widget_id);
        }
        None
    }
}
