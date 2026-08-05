use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn popover_trigger_ancestor(&self, mut widget_id: WidgetId) -> Option<WidgetId> {
        let layout = self.cached_scene.as_ref()?.layout.as_ref()?;
        loop {
            if layout
                .resolved_widget(widget_id)
                .and_then(|resolved| resolved.popover.as_ref())
                .is_some()
            {
                return Some(widget_id);
            }
            widget_id = layout.parent_of(widget_id)?;
        }
    }

    pub(super) fn toggle_popover_from_trigger_descendant(&mut self, widget_id: WidgetId) -> bool {
        let Some((current, command)) =
            self.popover_trigger_ancestor(widget_id)
                .and_then(|source_id| {
                    let popover = self
                        .cached_scene
                        .as_ref()?
                        .layout
                        .as_ref()?
                        .resolved_widget(source_id)?
                        .popover
                        .as_ref()?;
                    if popover.disabled.resolve() || !popover.trigger_mode.allows_click() {
                        return None;
                    }
                    Some((popover.is_open(), popover.on_open_change.clone()?))
                })
        else {
            return false;
        };
        self.execute_value_command(&command, !current);
        true
    }

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
            let inside_hover_region = handle.rect.contains(cursor)
                || layout
                    .widget_bounds(widget_id)
                    .map(|anchor_rect| rect_union_contains(anchor_rect, handle.rect, cursor))
                    .unwrap_or(false);
            if !inside_hover_region {
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

fn rect_union_contains(a: Rect, b: Rect, point: Point) -> bool {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = a.right().max(b.right());
    let bottom = a.bottom().max(b.bottom());
    Rect::new(left, top, right - left, bottom - top).contains(point)
}
