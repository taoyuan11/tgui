use std::borrow::Cow;

use super::*;
use crate::ui::widget::common;

impl<VM> ResolvedElement<VM> {
    pub(super) fn collect_slider_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Slider {
            value,
            min,
            max,
            step,
            show_ticks,
            show_value_label,
            tick_count,
            value_formatter,
            on_change,
            ..
        } = &self.kind
        else {
            return false;
        };

        let slider_style = visual
            .styles
            .slider_style
            .as_ref()
            .expect("slider style should be resolved for slider widgets");
        let resolved_value = common::slider_resolve_value(value.resolve(), *min, *max, *step);
        let display_value = context
            .active_slider_value
            .filter(|(widget_id, _)| *widget_id == self.id)
            .map(|(_, raw_value)| common::slider_resolve_value(raw_value, *min, *max, *step))
            .unwrap_or(resolved_value);
        let value_label = if *show_value_label {
            Some(
                value_formatter
                    .as_ref()
                    .map(|formatter| formatter.format(display_value))
                    .unwrap_or_else(|| format!("{display_value:.2}")),
            )
        } else {
            None
        };
        let geometry = push_slider_primitives(
            visual.frame,
            display_value,
            *min,
            *max,
            *step,
            *show_ticks,
            *show_value_label,
            common::slider_tick_count(*min, *max, *step, *tick_count),
            value_label.as_deref(),
            slider_style,
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.primitive_clip_mask,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            context.media,
        );
        if !visual.disabled {
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Slider {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    on_change: on_change.clone(),
                    value: display_value,
                    min: *min,
                    max: *max,
                    step: *step,
                    track_rect: geometry.track_rect,
                    thumb_rect: geometry.thumb_rect,
                },
            });
        }
        true
    }

    pub(super) fn collect_select_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Select {
            selected_label,
            placeholder,
            options,
            open,
            on_open_change,
            ..
        } = &self.kind
        else {
            return false;
        };

        let active = open
            .as_ref()
            .map(Value::resolve)
            .or_else(|| context.select_open_states.get(&self.id).copied())
            .unwrap_or(false);
        let menu_progress = context.animations.resolve_f32(
            crate::animation::AnimationKey::Widget {
                id: self.id.raw(),
                property: WidgetProperty::SelectMenuOpen,
            },
            if active { 1.0 } else { 0.0 },
            Some(default_select_menu_transition()),
            context.now,
        );
        let select_style = visual
            .styles
            .select_style
            .as_ref()
            .expect("select style should be resolved for select widgets");
        let padding = Insets::symmetric(select_style.padding_x, select_style.padding_y);
        push_select_primitives(
            visual.frame,
            selected_label.resolve(),
            placeholder,
            select_style,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            padding,
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.primitive_clip_mask,
        );
        if menu_progress > f32::EPSILON && !visual.disabled {
            push_select_menu_primitives(
                self.id,
                visual.frame,
                context.viewport,
                options,
                on_open_change.as_ref(),
                select_style,
                context,
                computed,
                visual.opacity,
                menu_progress,
            );
        }
        if !visual.disabled {
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::SelectTrigger {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    on_open_change: on_open_change.clone(),
                    is_open: active,
                },
            });
        }
        true
    }

    pub(super) fn collect_text_editor_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::TextEditor {
            controller,
            placeholder,
            on_change,
            on_change_set,
            multiline,
            show_scrollbar,
            auto_wrap,
            ..
        } = &self.kind
        else {
            return false;
        };

        let input_style = visual
            .styles
            .input_style
            .as_ref()
            .expect("input style should be resolved for input widgets");
        let show_scrollbar = show_scrollbar.resolve();
        let auto_wrap = auto_wrap.resolve();
        let padding = Insets::symmetric(input_style.padding_x, input_style.padding_y);
        let inner = visual.frame.inset(padding);
        let content_viewport = text_input_content_viewport(
            visual.frame,
            padding,
            *multiline,
            show_scrollbar,
            context.theme,
            context.units,
        );
        let focused = context.focused_input == Some(self.id);
        let controller_revision = (!focused).then(|| controller.revision());
        let cached_layout = (!focused)
            .then(|| {
                context
                    .text_layout_overrides
                    .and_then(|overrides| overrides.get(&self.id))
                    .filter(|override_layout| Some(override_layout.revision) == controller_revision)
            })
            .flatten();
        let resolved_value = if focused {
            context
                .focused_text_value
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(controller.text()))
        } else if let Some(override_layout) = cached_layout {
            Cow::Borrowed(override_layout.text)
        } else {
            Cow::Owned(controller.text())
        };
        let display_value = if resolved_value.is_empty() {
            Cow::Owned(placeholder.resolve())
        } else {
            Cow::Borrowed(resolved_value.as_ref())
        };
        let mut text = text_with_typography("", &input_style.text_style);
        text.color = Some(Value::Static(input_style.text));
        let precomputed_layout = if resolved_value.is_empty() {
            None
        } else if focused {
            context.focused_text_layout
        } else {
            cached_layout.map(|override_layout| override_layout.layout)
        };
        let scroll_offset = context
            .scroll_offsets
            .get(&self.id)
            .copied()
            .unwrap_or(Point::ZERO);
        let text_render = push_text_input_primitives(
            display_value.as_ref(),
            &text,
            visual.frame,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            context.caret_visible && context.focused_input == Some(self.id),
            *multiline,
            auto_wrap,
            show_scrollbar,
            padding,
            scroll_offset,
            context
                .focused_text_state
                .filter(|_| context.focused_input == Some(self.id)),
            input_style.placeholder,
            input_style.selection,
            input_style.caret,
            visual.opacity,
            self.id,
            precomputed_layout,
            visual.primitive_clip,
            visual.primitive_clip_mask,
        );
        if *multiline {
            let content_bounds = Rect::new(
                content_viewport.x,
                content_viewport.y,
                text_render.content_width,
                text_render.content_height.max(content_viewport.height),
            );
            let overflow_x = if auto_wrap {
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
            let clamped_scroll = Point {
                x: if overflow_x == Overflow::Scroll {
                    scroll_offset.x.clamp(0.0, max_scroll.x)
                } else {
                    Dp::ZERO
                },
                y: scroll_offset.y.clamp(0.0, max_scroll.y),
            };
            let scrollbar_geometry = compute_scrollbar_geometry(
                inner,
                content_bounds,
                clamped_scroll,
                &scrollbar_layout,
                context.theme,
                context.units,
            );
            let visible_frame = visual
                .frame
                .intersect(visual.primitive_clip.unwrap_or(visual.frame))
                .unwrap_or(Rect::new(visual.frame.x, visual.frame.y, 0.0, 0.0));
            computed.scroll_regions.push(ScrollRegion {
                id: self.id,
                content_viewport,
                visible_frame,
                content_bounds,
                scroll_offset: clamped_scroll,
                overflow_x,
                overflow_y,
                horizontal_track: scrollbar_geometry.horizontal_track,
                horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                vertical_track: scrollbar_geometry.vertical_track,
                vertical_thumb: scrollbar_geometry.vertical_thumb,
            });
            if show_scrollbar {
                push_scrollbar_primitives(
                    &mut computed.scene,
                    context.theme,
                    inner,
                    visual.opacity,
                    &scrollbar_layout,
                    scrollbar_geometry,
                    self.id,
                    context.hovered_scrollbar,
                    context.active_scrollbar,
                );
            }
        }
        if context.focused_input == Some(self.id) {
            computed.ime_cursor_area = text_render.ime_cursor_area;
        }
        if !visual.disabled {
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::TextInput {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    controller: controller.clone(),
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                    multiline: *multiline,
                    auto_wrap,
                    show_scrollbar,
                    frame: visual.frame,
                    padding,
                    text_style: text,
                },
            });
        }
        true
    }
}
