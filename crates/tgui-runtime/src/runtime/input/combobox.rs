use super::*;
use crate::runtime::overlay::OverlayLayer;
use crate::ui::widget::{FocusTargetMeta, HitInteraction, PopoverVirtualListNavigation, WidgetKey};
use smallvec::SmallVec;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn popover_uses_list_keyboard_navigation(&self, source_id: WidgetId) -> bool {
        self.cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(source_id))
            .and_then(|resolved| resolved.popover.as_ref())
            .is_some_and(|popover| popover.list_keyboard_navigation)
    }

    fn open_list_popover_rect(&mut self, source_id: WidgetId) -> Option<Rect> {
        self.computed_scene()
            .overlay_close_handlers
            .iter()
            .rev()
            .find(|handler| {
                handler.layer == OverlayLayer::Popover
                    && handler.source_widget_id == Some(source_id)
            })
            .map(|handler| handler.rect)
    }

    pub(super) fn open_focused_text_input_popover(&mut self) -> bool {
        let Some(focused_id) = self.focused_text_input_id() else {
            return false;
        };
        let command = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(focused_id))
            .and_then(|resolved| resolved.popover.as_ref())
            .and_then(|popover| {
                (!popover.disabled.resolve() && !popover.is_open())
                    .then(|| popover.on_open_change.clone())
                    .flatten()
            });
        let Some(command) = command else {
            return false;
        };
        self.execute_value_command(&command, true);
        true
    }

    fn focused_open_popover_source(&mut self) -> Option<WidgetId> {
        let focused_id = self.focused_widget_id()?;

        if self.focused_text_input_id() == Some(focused_id) {
            if !self.popover_uses_list_keyboard_navigation(focused_id) {
                return None;
            }
            let state = self
                .cached_scene
                .as_ref()
                .and_then(|cached| cached.layout.as_ref())
                .and_then(|layout| layout.resolved_widget(focused_id))
                .and_then(|resolved| resolved.popover.as_ref())
                .and_then(|popover| {
                    (!popover.disabled.resolve())
                        .then(|| (popover.is_open(), popover.on_open_change.clone()))
                });
            let (open, open_command) = state?;
            if !open {
                open_command?;
                if !self.open_focused_text_input_popover() {
                    return None;
                }
                let _ = self.computed_scene();
            }
            let is_open = self
                .cached_scene
                .as_ref()
                .and_then(|cached| cached.layout.as_ref())
                .and_then(|layout| layout.resolved_widget(focused_id))
                .and_then(|resolved| resolved.popover.as_ref())
                .is_some_and(|popover| !popover.disabled.resolve() && popover.is_open());
            if is_open {
                return Some(focused_id);
            }
        }

        let source = {
            let (focused_point, handlers) = {
                let computed = self.computed_scene();
                let focused_point = computed.overlay_hit_regions.iter().find_map(|region| {
                    (region.focus.as_ref().map(|focus| focus.widget_id) == Some(focused_id))
                        .then_some(Point::new(
                            region.rect.x + region.rect.width * 0.5,
                            region.rect.y + region.rect.height * 0.5,
                        ))
                });
                let handlers = computed
                    .overlay_close_handlers
                    .iter()
                    .rev()
                    .filter_map(|handler| {
                        (handler.layer == OverlayLayer::Popover)
                            .then_some(handler.source_widget_id.map(|id| (id, handler.rect)))
                            .flatten()
                    })
                    .collect::<SmallVec<[_; 4]>>();
                (focused_point, handlers)
            };
            let Some(focused_point) = focused_point else {
                return None;
            };
            handlers.into_iter().find_map(|(source_id, rect)| {
                (rect.contains(focused_point)
                    && self.popover_uses_list_keyboard_navigation(source_id))
                .then_some(source_id)
            })
        }?;
        Some(source)
    }

    pub(super) fn focus_open_popover_option_from_input(&mut self, direction: i32) -> bool {
        let Some(source_id) = self.focused_open_popover_source() else {
            return false;
        };
        let virtual_navigation = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(source_id))
            .and_then(|resolved| resolved.popover.as_ref())
            .and_then(|popover| popover.virtual_list_navigation.clone());
        if let Some(navigation) = virtual_navigation {
            if self.focus_virtual_popover_option(source_id, &navigation, direction) {
                return true;
            }
        }
        let focused_id = self.focused_widget_id();
        let popover_rect = self.open_list_popover_rect(source_id);
        let computed = self.computed_scene();
        let mut options = SmallVec::<[_; 8]>::new();
        let mut seen_inline = SmallVec::<[WidgetId; 8]>::new();
        let mut seen_heap = None;
        for region in computed.overlay_hit_regions.iter() {
            let Some(focus) = region.focus.as_ref() else {
                continue;
            };
            if focus.widget_id == source_id
                || popover_rect.is_some_and(|rect| {
                    !rect.contains(Point::new(
                        region.rect.x + region.rect.width * 0.5,
                        region.rect.y + region.rect.height * 0.5,
                    ))
                })
                || !matches!(
                    &region.interaction,
                    HitInteraction::Widget { interactions, .. }
                        if interactions.on_click.is_some()
                )
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

    fn focus_virtual_popover_option(
        &mut self,
        source_id: WidgetId,
        navigation: &PopoverVirtualListNavigation,
        direction: i32,
    ) -> bool {
        let disabled = navigation.resolve_disabled();
        let enabled_indices = disabled
            .iter()
            .enumerate()
            .filter_map(|(index, disabled)| (!disabled).then_some(index))
            .collect::<SmallVec<[usize; 8]>>();
        if enabled_indices.is_empty() {
            return false;
        }

        let focused_id = self.focused_widget_id();
        let current_index = focused_id.and_then(|focused_id| {
            let cache = self.virtual_states.get(&navigation.list_id)?;
            enabled_indices.iter().position(|option_index| {
                cache
                    .widget_ids_by_key
                    .get(&WidgetKey::from(*option_index))
                    .copied()
                    == Some(focused_id)
            })
        });
        let next_enabled_index = current_index
            .map(|index| {
                if direction < 0 {
                    index.checked_sub(1).unwrap_or(enabled_indices.len() - 1)
                } else {
                    (index + 1) % enabled_indices.len()
                }
            })
            .unwrap_or_else(|| {
                if direction < 0 {
                    enabled_indices.len() - 1
                } else {
                    0
                }
            });
        let option_index = enabled_indices[next_enabled_index];
        let Some(focus) = self.materialize_virtual_popover_option(
            source_id,
            navigation,
            option_index,
            disabled.len(),
        ) else {
            return false;
        };
        self.update_focus(
            Some(FocusedWidget {
                widget_id: focus.widget_id,
                scope_path: focus.scope_path,
                on_blur: focus.on_blur.clone(),
            }),
            focus.on_focus,
            true,
        );
        true
    }

    fn materialize_virtual_popover_option(
        &mut self,
        source_id: WidgetId,
        navigation: &PopoverVirtualListNavigation,
        option_index: usize,
        option_count: usize,
    ) -> Option<FocusTargetMeta<VM>> {
        let _ = self.computed_scene();
        let scroll_region = self.cached_scene.as_ref().and_then(|cached| {
            cached
                .computed
                .scroll_regions
                .iter()
                .copied()
                .find(|region| region.id == navigation.list_id)
        });
        if let Some(region) =
            scroll_region.filter(|region| region.can_scroll_y() && option_count > 0)
        {
            let current = self.effective_scroll_offset(region.id, region.scroll_offset);
            let item_extent = region.content_bounds.height / option_count as f32;
            let row_top = item_extent * option_index as f32;
            let row_bottom = row_top + item_extent;
            let viewport_bottom = current.y + region.content_viewport.height;
            let next_y = if row_top < current.y {
                row_top
            } else if row_bottom > viewport_bottom {
                row_bottom - region.content_viewport.height
            } else {
                current.y
            }
            .clamp(Dp::ZERO, region.max_offset().y);
            if (next_y - current.y).abs() > 0.01 {
                self.cancel_scroll_motion(region.id);
                self.set_scroll_offset(region.id, Point::new(current.x, next_y));
                let _ = self.computed_scene();
            }
        }

        let option_id = self
            .virtual_states
            .get(&navigation.list_id)?
            .widget_ids_by_key
            .get(&WidgetKey::from(option_index))
            .copied()?;
        let popover_rect = self.open_list_popover_rect(source_id);
        self.computed_scene()
            .overlay_hit_regions
            .iter()
            .find_map(|region| {
                let focus = region.focus.as_ref()?;
                (focus.widget_id == option_id
                    && popover_rect.is_none_or(|rect| {
                        rect.contains(Point::new(
                            region.rect.x + region.rect.width * 0.5,
                            region.rect.y + region.rect.height * 0.5,
                        ))
                    }))
                .then(|| focus.clone())
            })
    }

    pub(super) fn activate_first_open_popover_option_from_input(&mut self) -> bool {
        let Some(source_id) = self.focused_open_popover_source() else {
            return false;
        };
        let focused_id = self.focused_widget_id();
        let popover_rect = self.open_list_popover_rect(source_id);
        let computed = self.computed_scene();
        let mut candidates = computed.overlay_hit_regions.iter().filter(|region| {
            region.focus.is_some()
                && popover_rect.is_none_or(|rect| {
                    rect.contains(Point::new(
                        region.rect.x + region.rect.width * 0.5,
                        region.rect.y + region.rect.height * 0.5,
                    ))
                })
        });
        let focused_command = focused_id.and_then(|focused_id| {
            candidates.clone().find_map(|region| {
                (region.focus.as_ref().map(|focus| focus.widget_id) == Some(focused_id))
                    .then(|| match &region.interaction {
                        HitInteraction::Widget { interactions, .. } => {
                            interactions.on_click.clone()
                        }
                        _ => None,
                    })
                    .flatten()
            })
        });
        let command = focused_command.or_else(|| {
            (focused_id == Some(source_id)).then(|| {
                candidates.find_map(|region| match &region.interaction {
                    HitInteraction::Widget { interactions, .. } => interactions.on_click.clone(),
                    _ => None,
                })
            })?
        });
        command.is_some_and(|command| {
            self.execute_command(&command);
            true
        })
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
