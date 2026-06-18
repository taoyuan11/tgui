use super::*;
use crate::runtime::overlay::OverlayLayer;
use crate::ui::widget::HitInteraction;
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn focused_open_popover_source(
        &mut self,
    ) -> Option<(WidgetId, crate::ui::widget::ComputedScene<VM>)> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene().clone();

        if self.focused_text_input_id() == Some(focused_id) {
            let open = self
                .cached_scene
                .as_ref()
                .and_then(|cached| cached.layout.as_ref())
                .and_then(|layout| layout.resolved_widget(focused_id))
                .and_then(|resolved| resolved.popover.as_ref())
                .map(|popover| !popover.disabled.resolve() && popover.open.resolve())
                .unwrap_or(false);
            if open {
                return Some((focused_id, computed));
            }
        }

        let focused_is_overlay = computed
            .overlay_hit_regions
            .iter()
            .any(|region| region.focus.as_ref().map(|focus| focus.widget_id) == Some(focused_id));
        if !focused_is_overlay {
            return None;
        }

        let source = computed
            .overlay_close_handlers
            .iter()
            .rev()
            .find(|handler| {
                handler.layer == OverlayLayer::Popover && handler.source_widget_id.is_some()
            })
            .and_then(|handler| handler.source_widget_id)?;
        Some((source, computed))
    }

    pub(super) fn focus_open_popover_option_from_input(&mut self, direction: i32) -> bool {
        let Some((source_id, computed)) = self.focused_open_popover_source() else {
            return false;
        };
        let focused_id = self.focused_widget_id();
        let mut options = SmallVec::<[_; 8]>::new();
        let mut seen_inline = SmallVec::<[WidgetId; 8]>::new();
        let mut seen_heap = None;
        for region in computed.overlay_hit_regions.iter() {
            let Some(focus) = region.focus.as_ref() else {
                continue;
            };
            if focus.widget_id == source_id
                || !insert_seen_option(&mut seen_inline, &mut seen_heap, focus.widget_id)
            {
                continue;
            }
            options.push(focus.clone());
        }
        if options.is_empty() {
            return false;
        }

        let next_index = focused_id
            .and_then(|id| options.iter().position(|focus| focus.widget_id == id))
            .map(|index| {
                if direction < 0 {
                    index.checked_sub(1).unwrap_or(options.len() - 1)
                } else {
                    (index + 1) % options.len()
                }
            })
            .unwrap_or_else(|| if direction < 0 { options.len() - 1 } else { 0 });
        let next = options
            .into_iter()
            .nth(next_index)
            .expect("combobox option focus index should be valid");
        self.update_focus(
            Some(FocusedWidget {
                widget_id: next.widget_id,
                scope_path: next.scope_path,
                on_blur: next.on_blur,
            }),
            next.on_focus,
            true,
        );
        true
    }

    pub(super) fn activate_first_open_popover_option_from_input(&mut self) -> bool {
        let Some((source_id, computed)) = self.focused_open_popover_source() else {
            return false;
        };
        for region in computed.overlay_hit_regions.iter() {
            let HitInteraction::Widget {
                id, interactions, ..
            } = &region.interaction
            else {
                continue;
            };
            if *id == source_id {
                continue;
            }
            if let Some(command) = interactions.on_click.as_ref() {
                self.execute_command(command);
                return true;
            }
        }
        false
    }
}

fn insert_seen_option(
    seen_inline: &mut SmallVec<[WidgetId; 8]>,
    seen_heap: &mut Option<std::collections::HashSet<WidgetId>>,
    widget_id: WidgetId,
) -> bool {
    if let Some(seen) = seen_heap.as_mut() {
        return seen.insert(widget_id);
    }

    if seen_inline.contains(&widget_id) {
        return false;
    }

    if seen_inline.len() < 8 {
        seen_inline.push(widget_id);
        return true;
    }

    let mut seen = seen_inline
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let inserted = seen.insert(widget_id);
    *seen_heap = Some(seen);
    inserted
}
