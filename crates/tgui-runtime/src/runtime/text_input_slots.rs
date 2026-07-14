use super::input::INPUT_CARET_WIDTH;
use super::state::{CaretDecorationBinding, TextInputSlotBinding};
use super::*;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    text_input_content_geometry, RenderCommand, ScenePrimitives, TextDecorationPrimitive,
    TextInputContentGeometry, TextPrimitive, TextPrimitiveSlot,
};
use std::borrow::Cow;
use std::sync::Arc;

struct TextInputSlotPatch {
    widget_id: WidgetId,
    texts: Vec<TextPrimitive>,
    selection: TextDecorationPrimitive,
    caret: TextDecorationPrimitive,
    caret_visible_color: Color,
    caret_rect: Rect,
    scroll_region: Option<ScrollRegion>,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn rebuild_text_input_slot_bindings(&mut self) {
        let binding = self.build_focused_text_input_slot_binding();
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        cached.text_input_slot_bindings.clear();
        if let Some(binding) = binding {
            cached
                .text_input_slot_bindings
                .insert(binding.widget_id, binding);
        }
    }

    pub(super) fn try_update_focused_text_input_slots(
        &mut self,
        now: Instant,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(widget_id) = self.focused_text_input_id_cached(&cached.computed) else {
            return false;
        };
        if !self.can_update_focused_text_input_slots(
            cached,
            widget_id,
            viewport,
            units,
            caret_visible,
            active_scrollbar,
        ) {
            return false;
        }
        let Some(binding) = cached.text_input_slot_bindings.get(&widget_id).cloned() else {
            return false;
        };
        let Some(patch) = self.build_text_input_slot_patch(widget_id, caret_visible) else {
            return false;
        };
        if !self.can_write_text_input_slot_patch(cached, &binding, &patch) {
            return false;
        }

        let focused_widget = self.focused_widget_id();
        let focus_visible = self.focus_visible;
        let pressed_widget = self.pressed_widget;
        let selected_text = self.selected_text;
        let theme_epoch = self.theme_store.version();
        let style_sheet_version = self.config.style_sheet.version();
        let density = self.theme.density;
        let reduced_motion = self.reduced_motion;
        let text_scale_bits = units.font_scale().to_bits();
        let animation_epoch = self.animation_epoch;
        let layout_animation_epoch = self.layout_animation_epoch;
        let accessibility_animation_epoch = self.accessibility_animation_epoch;
        let scroll_epoch = self.scroll_epoch;
        let hover_epoch = self.hover_epoch;
        let text_input_epoch = self.text_input_epoch;
        let external_portal_revision = self.external_portal_revision;
        let hovered_scrollbar = self.hovered_scrollbar;

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        let zero = SceneCounts::default();
        let Some(target_chunk) = cached.scene_chunks.get_mut(&widget_id) else {
            return false;
        };
        if !write_text_input_slot_patch(
            target_chunk,
            &binding,
            &zero,
            binding.scroll_region_index,
            &patch,
        ) {
            return false;
        }
        for (ancestor_id, offset, scroll_region_index) in &binding.ancestor_offsets {
            let Some(ancestor_chunk) = cached.scene_chunks.get_mut(ancestor_id) else {
                return false;
            };
            if !write_text_input_slot_patch(
                ancestor_chunk,
                &binding,
                offset,
                *scroll_region_index,
                &patch,
            ) {
                return false;
            }
        }
        if !write_text_input_slot_patch(
            &mut cached.computed,
            &binding,
            &binding.root_offset,
            binding.root_scroll_region_index,
            &patch,
        ) {
            return false;
        }

        cached.caret_decoration = Some(CaretDecorationBinding {
            overlay_text_decoration_index: binding.root_offset.overlay_text_decorations
                + binding.caret_slot.text_decoration_index,
            overlay_command_index: binding.root_offset.overlay_commands
                + binding.caret_slot.command_index,
            visible_color: patch.caret_visible_color,
        });
        cached.focused_widget = focused_widget;
        cached.focus_visible = focus_visible;
        cached.pressed_widget = pressed_widget;
        cached.selected_text = selected_text;
        cached.caret_visible = caret_visible;
        cached.theme_epoch = theme_epoch;
        cached.style_sheet_version = style_sheet_version;
        cached.density = density;
        cached.reduced_motion = reduced_motion;
        cached.text_scale_bits = text_scale_bits;
        cached.animation_epoch = animation_epoch;
        cached.layout_animation_epoch = layout_animation_epoch;
        cached.accessibility_animation_epoch = accessibility_animation_epoch;
        cached.scroll_epoch = scroll_epoch;
        cached.hover_epoch = hover_epoch;
        cached.text_input_epoch = text_input_epoch;
        cached.external_portal_revision = external_portal_revision;
        cached.hovered_scrollbar = hovered_scrollbar;
        cached.active_scrollbar = active_scrollbar;
        cached.computed_valid = true;
        let _ = now;
        self.scroll_dirty_widgets.remove(&widget_id);
        super::action_stats::record("text_input_slot_write");
        true
    }

    fn can_update_focused_text_input_slots(
        &self,
        cached: &CachedScene<VM>,
        widget_id: WidgetId,
        viewport: Rect,
        units: UnitContext,
        _caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        let scroll_epoch_matches = cached.scroll_epoch == self.scroll_epoch
            || (self.scroll_dirty_widgets.len() == 1
                && self.scroll_dirty_widgets.contains(&widget_id));
        cached.computed_valid
            && cached.layout_valid
            && cached.layout.is_some()
            && cached.text_input_epoch != self.text_input_epoch
            && cached.viewport == viewport
            && cached.units == units
            && cached.focused_widget == self.focused_widget_id()
            && cached.focus_visible == self.focus_visible
            && cached.pressed_widget == self.pressed_widget
            && cached.selected_text == self.selected_text
            && cached.theme_epoch == self.theme_store.version()
            && cached.style_sheet_version == self.config.style_sheet.version()
            && cached.density == self.theme.density
            && cached.reduced_motion == self.reduced_motion
            && cached.text_scale_bits == units.font_scale().to_bits()
            && cached.animation_epoch == self.animation_epoch
            && cached.layout_animation_epoch == self.layout_animation_epoch
            && scroll_epoch_matches
            && cached.hover_epoch == self.hover_epoch
            && cached.external_portal_revision == self.external_portal_revision
            && cached.hovered_scrollbar == self.hovered_scrollbar
            && cached.active_scrollbar == active_scrollbar
    }

    fn build_focused_text_input_slot_binding(&self) -> Option<TextInputSlotBinding> {
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let widget_id = self.focused_text_input_id_cached(&cached.computed)?;
        let patch = self.build_text_input_slot_patch(widget_id, cached.caret_visible)?;
        let target_chunk = cached.scene_chunks.get(&widget_id)?;

        let text_slots = text_slots_for_patch(&target_chunk.scene, &patch.texts)?;
        let selection_slots = target_chunk
            .scene
            .matching_text_decoration_slots(|decoration| {
                decoration.corner_radius == patch.selection.corner_radius
                    && decoration.stroke_width == patch.selection.stroke_width
                    && decoration.clip_rect == patch.selection.clip_rect
                    && decoration.clip_mask == patch.selection.clip_mask
            });
        let caret_slots = target_chunk
            .scene
            .matching_overlay_text_decoration_slots(|decoration| {
                decoration.segments.len() == 1
                    && decoration.stroke_width == patch.caret.stroke_width
                    && decoration.clip_rect == patch.caret.clip_rect
                    && decoration.clip_mask == patch.caret.clip_mask
            });
        if selection_slots.len() != 1 || caret_slots.len() != 1 {
            return None;
        }
        let scroll_region_index = if patch.scroll_region.is_some() {
            let mut matches = target_chunk
                .scroll_regions
                .iter()
                .enumerate()
                .filter(|(_, region)| region.id == widget_id)
                .map(|(index, _)| index);
            let index = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(index)
        } else {
            None
        };

        let (root_offset, root_scroll_region_index, ancestor_offsets) =
            if widget_id == layout.root_id() {
                (SceneCounts::default(), scroll_region_index, Vec::new())
            } else {
                let offsets = layout.scene_splice_ancestor_offsets(
                    widget_id,
                    &cached.scene_chunk_parts,
                    &cached.scene_chunks,
                )?;
                let (root_id, root_offset, _, root_scroll_offset) = offsets.first().copied()?;
                if root_id != layout.root_id() {
                    return None;
                }
                (
                    root_offset,
                    scroll_region_index.map(|index| root_scroll_offset + index),
                    offsets
                        .into_iter()
                        .map(|(ancestor_id, offset, _, scroll_offset)| {
                            (
                                ancestor_id,
                                offset,
                                scroll_region_index.map(|index| scroll_offset + index),
                            )
                        })
                        .collect(),
                )
            };

        let binding = TextInputSlotBinding {
            widget_id,
            text_slots,
            selection_slot: selection_slots[0],
            caret_slot: caret_slots[0],
            scroll_region_index,
            root_offset,
            root_scroll_region_index,
            ancestor_offsets,
        };
        if self.can_write_text_input_slot_patch(cached, &binding, &patch) {
            Some(binding)
        } else {
            None
        }
    }

    fn build_text_input_slot_patch(
        &self,
        widget_id: WidgetId,
        caret_visible: bool,
    ) -> Option<TextInputSlotPatch> {
        let region = self.text_input_regions.get(&widget_id)?;
        let session = self.text_input_buffers.get(&widget_id)?;
        let content = session.current_text.as_str();
        let layout = session.layout_snapshot.as_ref()?;
        let state = self
            .text_edit_state(widget_id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(widget_id, content))
            .clamped_to(content);

        let DisplayTextState {
            content: display_content,
            cursor: display_cursor,
            selection_range,
        } = display_text_state(content, &state);
        if session.display_text != display_content.as_ref() {
            return None;
        }

        let input = region.context(content);
        let content_viewport = input.content_viewport(&self.theme, self.unit_context());
        let (text_request, font_size, line_height, letter_spacing) =
            resolved_input_text_metrics(&self.theme, self.unit_context(), input.text_style);
        let TextInputContentGeometry {
            content_frame,
            content_width,
            content_height,
            ..
        } = text_input_content_geometry(
            layout,
            line_height,
            content_viewport,
            region.multiline,
            region.auto_wrap,
            Point::new(state.scroll_x, state.scroll_y),
            INPUT_CARET_WIDTH,
        );

        let existing = self.find_current_text_input_primitives(widget_id)?;
        let current_scroll_region = if region.multiline {
            Some(self.find_current_text_input_scroll_region(widget_id)?)
        } else {
            None
        };
        let resolved_font = self
            .font_manager
            .resolve_text(display_content.as_ref(), text_request);
        let font_family = Some(Arc::from(resolved_font.primary_font));
        let texts = if region.multiline {
            multiline_text_primitives_for_layout(
                layout,
                display_content.as_ref(),
                content_frame,
                content_viewport,
                line_height,
                &existing.texts,
                font_family.clone(),
                font_size,
                letter_spacing,
            )?
        } else {
            let mut text = existing.texts.first()?.clone();
            text.content = Arc::from(display_content.as_ref().to_string());
            text.frame = content_frame;
            text.font_family = font_family;
            text.font_size = font_size;
            text.line_height = line_height;
            text.letter_spacing = letter_spacing;
            vec![text]
        };

        let selection_segments = selection_range
            .map(|range| {
                selection_segments_for_range(
                    layout,
                    display_content.len(),
                    content_frame,
                    line_height,
                    range,
                )
            })
            .filter(|segments| !segments.is_empty())
            .unwrap_or_else(|| {
                vec![Rect::new(
                    content_frame.x,
                    content_viewport.y,
                    0.0,
                    Dp::new(line_height),
                )]
            });
        let mut selection = existing.selection.clone();
        selection.segments = Arc::from(selection_segments);

        let caret_index = display_cursor.min(display_content.len());
        let caret_x = if region.multiline && region.auto_wrap {
            (content_frame.x + layout.x_for_index(caret_index))
                .min((content_viewport.right() - INPUT_CARET_WIDTH).max(content_viewport.x))
        } else {
            content_frame.x + layout.x_for_index(caret_index)
        };
        let caret_rect = Rect::new(
            caret_x,
            content_frame.y + Dp::new(layout.top_for_index(caret_index)),
            INPUT_CARET_WIDTH,
            Dp::new(layout.line_height_for_index(caret_index).max(line_height)),
        );
        let caret_visible_color = existing.caret_visible_color;
        let mut caret = existing.caret.clone();
        caret.segments = Arc::from(vec![caret_rect]);
        caret.color = if caret_visible {
            caret_visible_color
        } else {
            caret_visible_color.with_alpha_factor(0.0)
        };

        let scroll_region = current_scroll_region.map(|current| {
            let content_bounds = Rect::new(
                content_viewport.x,
                content_viewport.y,
                content_width,
                content_height.max(content_viewport.height),
            );
            let overflow_x = if region.auto_wrap {
                Overflow::Hidden
            } else {
                Overflow::Scroll
            };
            let overflow_y = Overflow::Scroll;
            let mut scrollbar_layout = ContainerLayout::flow();
            scrollbar_layout.overflow_x = overflow_x;
            scrollbar_layout.overflow_y = overflow_y;
            let max_scroll = Point {
                x: (content_bounds.right() - content_viewport.right()).max(0.0),
                y: (content_bounds.bottom() - content_viewport.bottom()).max(0.0),
            };
            let scroll_offset = self
                .scroll_states
                .get(&widget_id)
                .copied()
                .unwrap_or(Point::ZERO);
            let clamped_scroll = Point {
                x: if overflow_x == Overflow::Scroll {
                    scroll_offset.x.clamp(0.0, max_scroll.x)
                } else {
                    Dp::ZERO
                },
                y: scroll_offset.y.clamp(0.0, max_scroll.y),
            };
            let scrollbar_geometry = compute_scrollbar_geometry(
                region.frame.inset(region.padding),
                content_bounds,
                clamped_scroll,
                &scrollbar_layout,
                &self.theme,
                self.unit_context(),
            );
            ScrollRegion {
                id: widget_id,
                content_viewport,
                visible_frame: current.visible_frame,
                content_bounds,
                gpu_base_scroll_offset: clamped_scroll,
                scroll_offset: clamped_scroll,
                overflow_x,
                overflow_y,
                horizontal_track: scrollbar_geometry.horizontal_track,
                horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                vertical_track: scrollbar_geometry.vertical_track,
                vertical_thumb: scrollbar_geometry.vertical_thumb,
            }
        });

        Some(TextInputSlotPatch {
            widget_id,
            texts,
            selection,
            caret,
            caret_visible_color,
            caret_rect,
            scroll_region,
        })
    }

    fn find_current_text_input_primitives(
        &self,
        widget_id: WidgetId,
    ) -> Option<CurrentTextInputPrimitives> {
        let cached = self.cached_scene.as_ref()?;
        let binding = cached.text_input_slot_bindings.get(&widget_id);
        let target_chunk = cached.scene_chunks.get(&widget_id)?;
        let zero = SceneCounts::default();
        if let Some(binding) = binding {
            let mut current =
                current_text_input_primitives_from_scene(target_chunk, binding, &zero)?;
            if let Some(caret) = cached.caret_decoration {
                current.caret_visible_color = caret.visible_color;
            }
            return Some(current);
        }

        let texts = target_chunk
            .scene
            .texts
            .iter()
            .filter(|text| text.rich_spans.is_none() && !text.force_color)
            .cloned()
            .collect();
        let selection = target_chunk
            .scene
            .text_decorations
            .iter()
            .find(|decoration| !decoration.segments.is_empty())?
            .clone();
        let caret = target_chunk
            .scene
            .overlay_text_decorations
            .iter()
            .find(|decoration| decoration.segments.len() == 1 && decoration.stroke_width == 0.0)?
            .clone();
        Some(CurrentTextInputPrimitives {
            texts,
            selection,
            caret_visible_color: caret.color,
            caret,
        })
    }

    fn find_current_text_input_scroll_region(&self, widget_id: WidgetId) -> Option<ScrollRegion> {
        let cached = self.cached_scene.as_ref()?;
        let binding = cached.text_input_slot_bindings.get(&widget_id);
        let target_chunk = cached.scene_chunks.get(&widget_id)?;
        if let Some(binding) = binding {
            let index = binding.scroll_region_index?;
            return target_chunk.scroll_regions.get(index).copied();
        }

        let mut matches = target_chunk
            .scroll_regions
            .iter()
            .copied()
            .filter(|region| region.id == widget_id);
        let region = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(region)
    }

    fn can_write_text_input_slot_patch(
        &self,
        cached: &CachedScene<VM>,
        binding: &TextInputSlotBinding,
        patch: &TextInputSlotPatch,
    ) -> bool {
        let zero = SceneCounts::default();
        let Some(target_chunk) = cached.scene_chunks.get(&binding.widget_id) else {
            return false;
        };
        if !can_write_text_input_slot_patch(
            target_chunk,
            binding,
            &zero,
            binding.scroll_region_index,
            patch,
        ) {
            return false;
        }
        for (ancestor_id, offset, scroll_region_index) in &binding.ancestor_offsets {
            let Some(ancestor_chunk) = cached.scene_chunks.get(ancestor_id) else {
                return false;
            };
            if !can_write_text_input_slot_patch(
                ancestor_chunk,
                binding,
                offset,
                *scroll_region_index,
                patch,
            ) {
                return false;
            }
        }
        if !can_write_text_input_slot_patch(
            &cached.computed,
            binding,
            &binding.root_offset,
            binding.root_scroll_region_index,
            patch,
        ) {
            return false;
        }
        computed_overlay_slot_matches(&cached.computed, binding, &binding.root_offset, patch)
    }
}

struct CurrentTextInputPrimitives {
    texts: Vec<TextPrimitive>,
    selection: TextDecorationPrimitive,
    caret: TextDecorationPrimitive,
    caret_visible_color: Color,
}

struct DisplayTextState<'a> {
    content: Cow<'a, str>,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
}

fn display_text_state<'a>(content: &'a str, state: &TextEditState) -> DisplayTextState<'a> {
    if let Some(composition) = state.composition.as_ref() {
        let start = composition.replace_range.0.min(content.len());
        let end = composition.replace_range.1.min(content.len());
        let mut display = String::with_capacity(
            content.len() + composition.text.len().saturating_sub(end - start),
        );
        display.push_str(&content[..start]);
        display.push_str(&composition.text);
        display.push_str(&content[end..]);
        let composition_end = start + composition.text.len();
        let caret_offset = composition
            .cursor
            .map(|(_, end)| end.min(composition.text.len()))
            .unwrap_or(composition.text.len());
        return DisplayTextState {
            content: Cow::Owned(display),
            cursor: start + caret_offset,
            selection_range: (start < composition_end).then_some((start, composition_end)),
        };
    }

    DisplayTextState {
        content: Cow::Borrowed(content),
        cursor: state.cursor,
        selection_range: state.selection_range(),
    }
}

fn selection_segments_for_range(
    layout: &crate::text::font::TextLayoutInfo,
    content_len: usize,
    content_frame: Rect,
    line_height: f32,
    range: (usize, usize),
) -> Vec<Rect> {
    let start = range.0.min(content_len);
    let end = range.1.min(content_len);
    if start >= end {
        return Vec::new();
    }
    let start_line = layout.line_index_for_index(start);
    let end_line = layout.line_index_for_index(end);
    let mut segments = Vec::new();
    for line_index in start_line..=end_line {
        let line_start = start.max(layout.line_start(line_index));
        let line_end = end.min(layout.line_end(line_index));
        let x0 = layout.x_for_index(line_start);
        let x1 = layout.x_for_index(line_end);
        let width = (x1 - x0).max(0.0);
        if width <= 0.0 {
            continue;
        }
        segments.push(Rect::new(
            content_frame.x + x0,
            content_frame.y + Dp::new(layout.line_top(line_index)),
            width,
            Dp::new(layout.line_height(line_index).max(line_height)),
        ));
    }
    segments
}

fn multiline_text_primitives_for_layout(
    layout: &crate::text::font::TextLayoutInfo,
    display_content: &str,
    content_frame: Rect,
    content_viewport: Rect,
    line_height: f32,
    existing: &[TextPrimitive],
    font_family: Option<Arc<str>>,
    font_size: f32,
    letter_spacing: f32,
) -> Option<Vec<TextPrimitive>> {
    let expected = multiline_visible_text_slots(
        layout,
        display_content,
        content_frame,
        content_viewport,
        line_height,
        existing.len(),
    );
    if expected.len() != existing.len() {
        return None;
    }
    let mut texts = Vec::with_capacity(expected.len());
    for (template, (content, frame)) in existing.iter().zip(expected) {
        if template.rich_spans.is_some() || template.force_color {
            return None;
        }
        let mut text = template.clone();
        text.content = Arc::from(content.to_string());
        text.frame = frame;
        text.font_family = font_family.clone();
        text.font_size = font_size;
        text.line_height = line_height;
        text.letter_spacing = letter_spacing;
        texts.push(text);
    }
    Some(texts)
}

fn multiline_visible_text_slots<'a>(
    layout: &crate::text::font::TextLayoutInfo,
    display_content: &'a str,
    content_frame: Rect,
    content_viewport: Rect,
    line_height: f32,
    slot_count: usize,
) -> Vec<(&'a str, Rect)> {
    let viewport_top = content_viewport.y.get();
    let viewport_bottom = content_viewport.bottom().get();
    let visible_range = layout.line_range_for_vertical_span(
        viewport_top - content_frame.y.get(),
        viewport_bottom - content_frame.y.get(),
    );
    let start_line = visible_range.start;
    let mut slots = Vec::with_capacity(slot_count);
    for slot_index in 0..slot_count {
        let line_index = start_line + slot_index;
        if line_index < visible_range.end {
            let start = layout.line_start(line_index).min(display_content.len());
            let end = layout.line_end(line_index).min(display_content.len());
            let line_top = content_frame.y.get() + layout.line_top(line_index);
            let line_height_value = layout.line_height(line_index).max(line_height);
            let content = if start < end {
                &display_content[start..end]
            } else {
                ""
            };
            let width = if start < end {
                layout.line_width(line_index).max(1.0)
            } else {
                1.0
            };
            slots.push((
                content,
                Rect::new(content_frame.x, line_top, width, line_height_value),
            ));
        } else {
            slots.push((
                "",
                Rect::new(
                    content_frame.x,
                    content_viewport.y.get() + slot_index as f32 * line_height.max(1.0),
                    1.0,
                    line_height.max(1.0),
                ),
            ));
        }
    }
    slots
}

fn text_slots_for_patch(
    scene: &ScenePrimitives,
    patch_texts: &[TextPrimitive],
) -> Option<Vec<TextPrimitiveSlot>> {
    if patch_texts.is_empty() {
        return Some(Vec::new());
    }
    let slots = scene.matching_text_slots(|text| {
        patch_texts
            .iter()
            .any(|expected| text_input_text_primitive_matches(text, expected))
    });
    if slots.len() != patch_texts.len() {
        return None;
    }
    let mut verified = Vec::with_capacity(slots.len());
    for (slot, expected) in slots.iter().zip(patch_texts) {
        let actual = scene.texts.get(slot.text_index)?;
        if !text_input_text_primitive_matches(actual, expected) {
            return None;
        }
        verified.push(*slot);
    }
    Some(verified)
}

fn text_input_text_primitive_matches(actual: &TextPrimitive, expected: &TextPrimitive) -> bool {
    actual.rich_spans.is_none()
        && expected.rich_spans.is_none()
        && actual.content.as_ref() == expected.content.as_ref()
        && actual.frame == expected.frame
        && actual.quad == expected.quad
        && actual.color == expected.color
        && actual.force_color == expected.force_color
        && actual.font_family.as_deref() == expected.font_family.as_deref()
        && actual.font_size == expected.font_size
        && actual.font_weight == expected.font_weight
        && actual.line_height == expected.line_height
        && actual.letter_spacing == expected.letter_spacing
        && actual.wrap == expected.wrap
        && actual.overflow == expected.overflow
        && actual.horizontal_align == expected.horizontal_align
        && actual.vertical_align == expected.vertical_align
        && actual.clip_rect == expected.clip_rect
        && actual.clip_mask == expected.clip_mask
}

fn current_text_input_primitives_from_scene<VM>(
    computed: &ComputedScene<VM>,
    binding: &TextInputSlotBinding,
    offset: &SceneCounts,
) -> Option<CurrentTextInputPrimitives> {
    let mut texts = Vec::with_capacity(binding.text_slots.len());
    for text_slot in &binding.text_slots {
        let text_index = offset.texts.checked_add(text_slot.text_index)?;
        texts.push(computed.scene.texts.get(text_index)?.clone());
    }
    let selection_index = offset
        .text_decorations
        .checked_add(binding.selection_slot.text_decoration_index)?;
    let caret_index = offset
        .overlay_text_decorations
        .checked_add(binding.caret_slot.text_decoration_index)?;
    let selection = computed
        .scene
        .text_decorations
        .get(selection_index)?
        .clone();
    let caret = computed
        .scene
        .overlay_text_decorations
        .get(caret_index)?
        .clone();
    Some(CurrentTextInputPrimitives {
        texts,
        selection,
        caret_visible_color: caret.color,
        caret,
    })
}

fn can_write_text_input_slot_patch<VM>(
    computed: &ComputedScene<VM>,
    binding: &TextInputSlotBinding,
    offset: &SceneCounts,
    scroll_region_index: Option<usize>,
    patch: &TextInputSlotPatch,
) -> bool {
    let scroll_region_matches = match (patch.scroll_region, scroll_region_index) {
        (Some(region), Some(index)) => computed
            .scroll_regions
            .get(index)
            .is_some_and(|current| current.id == region.id),
        (None, None) => true,
        _ => false,
    };
    binding.text_slots.len() == patch.texts.len()
        && binding
            .text_slots
            .iter()
            .all(|slot| computed.can_write_text_color_slot(offset, *slot))
        && computed.can_write_text_decoration_slot(offset, binding.selection_slot)
        && computed.can_write_overlay_text_decoration_slot(offset, binding.caret_slot)
        && scroll_region_matches
}

fn write_text_input_slot_patch<VM>(
    computed: &mut ComputedScene<VM>,
    binding: &TextInputSlotBinding,
    offset: &SceneCounts,
    scroll_region_index: Option<usize>,
    patch: &TextInputSlotPatch,
) -> bool {
    if binding.text_slots.len() != patch.texts.len() {
        return false;
    }
    for (slot, text) in binding.text_slots.iter().zip(&patch.texts) {
        if !computed.write_text_slot(offset, *slot, text.clone()) {
            return false;
        }
    }
    if !computed.write_text_decoration_slot(offset, binding.selection_slot, patch.selection.clone())
    {
        return false;
    }
    if !computed.write_overlay_text_decoration_slot(offset, binding.caret_slot, patch.caret.clone())
    {
        return false;
    }
    match (patch.scroll_region, scroll_region_index) {
        (Some(region), Some(index)) => {
            let Some(current) = computed.scroll_regions.get_mut(index) else {
                return false;
            };
            if current.id != region.id {
                return false;
            }
            *current = region;
        }
        (None, None) => {}
        _ => return false,
    }
    computed.ime_cursor_area = Some(patch.caret_rect);
    computed.register_caret_overlay_anchor(patch.widget_id, patch.caret_rect);
    true
}

fn computed_overlay_slot_matches<VM>(
    computed: &ComputedScene<VM>,
    binding: &TextInputSlotBinding,
    offset: &SceneCounts,
    patch: &TextInputSlotPatch,
) -> bool {
    let Some(index) = offset
        .overlay_text_decorations
        .checked_add(binding.caret_slot.text_decoration_index)
    else {
        return false;
    };
    let Some(command_index) = offset
        .overlay_commands
        .checked_add(binding.caret_slot.command_index)
    else {
        return false;
    };
    let Some(decoration) = computed.scene.overlay_text_decorations.get(index) else {
        return false;
    };
    let Some(RenderCommand::TextDecoration(command)) =
        computed.scene.overlay_commands.get(command_index)
    else {
        return false;
    };
    decoration.segments.len() == 1
        && command.segments.len() == 1
        && decoration.clip_rect == patch.caret.clip_rect
        && command.clip_rect == patch.caret.clip_rect
}
