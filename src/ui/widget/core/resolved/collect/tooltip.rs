//! Tooltip 在 collect 阶段的浮层渲染。
//!
//! Tooltip 由 runtime 先解析”当前唯一激活的 tooltip”，collect 阶段只负责：
//! - hover trigger 的 delay 门控与 wakeup 注册；
//! - 按最终 solved placement 画背景、文字、阴影和三角指针；
//! - 对 focus / long-press tooltip 接入外部点击与 Esc 关闭。

use std::sync::Arc;
use taffy::prelude::{AvailableSpace, TaffyTree};
use taffy::Size as TaffySize;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::binding::{with_dependency_collection, DependencyGraph};
use crate::foundation::color::Color;
use crate::text::font::TextFontRequest;
use crate::ui::layout::{Insets, Length, Value};
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    ComputedScene, MeshPrimitive, MeshVertex, Rect, RenderPrimitive, TextPrimitive,
};
use crate::ui::widget::container::Stack;
use crate::ui::widget::core::measure_node;
use crate::ui::widget::overlay::{
    collect::emit_overlay, solve_placement, Anchor, AnchorKey, Overlay, OverlayContent, OverlayId,
    OverlayLayer, OverlayPrimitive,
};
use crate::ui::widget::tooltip::TooltipContent;
use crate::ui::widget::{Element, TooltipStyle};

use super::super::scene::{CollectContext, TooltipTrigger, VisualContext};
use super::super::types::ResolvedElement;
use super::super::CARET_WIDTH;
use super::CollectVisualState;
use crate::ui::widget::{OverlayPlacementOptions, OverlaySide, OverlaySolvedPlacement};

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn emit_tooltip_if_visible(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) {
        let Some(tooltip) = &self.tooltip else {
            return;
        };
        let active_tooltip = context.active_tooltip;
        let is_active_tooltip = active_tooltip
            .map(|tooltip_state| tooltip_state.widget_id == self.id)
            .unwrap_or(false);
        let another_tooltip_is_active = active_tooltip
            .map(|tooltip_state| tooltip_state.widget_id != self.id)
            .unwrap_or(false);

        let mut target_visible = is_active_tooltip;
        if let Some(active_tooltip) =
            active_tooltip.filter(|tooltip_state| tooltip_state.widget_id == self.id)
        {
            if active_tooltip.trigger == TooltipTrigger::Hover {
                if let Some(started_at) = context.tooltip_hover_started_at.get(&self.id).copied() {
                    let elapsed = context.now.saturating_duration_since(started_at);
                    if elapsed < tooltip.delay {
                        let wakeup = started_at + tooltip.delay;
                        let prev = context.next_tooltip_wakeup.get();
                        let merged = match prev {
                            Some(p) => Some(p.min(wakeup)),
                            None => Some(wakeup),
                        };
                        context.next_tooltip_wakeup.set(merged);
                        target_visible = false;
                    }
                } else if !tooltip.delay.is_zero() {
                    let wakeup = context.now + tooltip.delay;
                    let prev = context.next_tooltip_wakeup.get();
                    let merged = match prev {
                        Some(p) => Some(p.min(wakeup)),
                        None => Some(wakeup),
                    };
                    context.next_tooltip_wakeup.set(merged);
                    target_visible = false;
                }
            }
        }

        if matches!(&tooltip.content, TooltipContent::Text(text) if text.resolve().is_empty()) {
            let _ = context.animations.resolve_f32(
                AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::TooltipVisibility,
                },
                0.0,
                Some(default_tooltip_transition()),
                context.now,
            );
            return;
        }
        let visibility = context.animations.resolve_f32(
            AnimationKey::Widget {
                id: self.id.raw(),
                property: WidgetProperty::TooltipVisibility,
            },
            if target_visible { 1.0 } else { 0.0 },
            Some(default_tooltip_transition()),
            context.now,
        );
        if visibility <= f32::EPSILON {
            return;
        }
        if another_tooltip_is_active && !is_active_tooltip {
            return;
        }

        let style = Box::new(self.resolved_tooltip_style(context, visual));
        let background = style.background;
        let animated_offset = (style.offset - Dp::from((1.0 - visibility) * 4.0)).max(Dp::ZERO);

        computed.register_widget_overlay_anchor(self.id, visual.frame);
        let mut overlay = Overlay::<VM>::new(
            OverlayId::new(self.id.raw()),
            Anchor::Key(AnchorKey::widget(self.id)),
        )
        .source_widget(self.id)
        .placement(tooltip.placement)
        .offset(animated_offset)
        .flip_policy(tooltip.flip_policy)
        .viewport_padding(insets_max(style.padding))
        .layer(OverlayLayer::Tooltip);

        if let Some(active_tooltip) =
            active_tooltip.filter(|tooltip_state| tooltip_state.widget_id == self.id)
        {
            if matches!(
                active_tooltip.trigger,
                TooltipTrigger::Focus | TooltipTrigger::LongPress
            ) {
                overlay = overlay
                    .close_on_escape(true)
                    .close_on_outside_click(true)
                    .return_focus_to(self.id);
            } else {
                overlay = overlay.close_on_escape(true);
            }
        }

        match &tooltip.content {
            TooltipContent::Text(text) => emit_text_tooltip(
                text.resolve(),
                visual.frame,
                tooltip.placement,
                tooltip.flip_policy,
                &style,
                background,
                visibility,
                animated_offset,
                overlay,
                context,
                computed,
            ),
            TooltipContent::Element(content) => emit_element_tooltip(
                content.as_ref(),
                visual.frame,
                tooltip.placement,
                tooltip.flip_policy,
                &style,
                background,
                visibility,
                animated_offset,
                overlay,
                context,
                computed,
            ),
        }
    }

    fn resolved_tooltip_style(
        &self,
        context: &CollectContext<'_, '_>,
        visual: &CollectVisualState,
    ) -> TooltipStyle {
        let tooltip = self
            .tooltip
            .as_ref()
            .expect("tooltip style should only resolve for tooltip elements");
        let mut style = tooltip.resolved_style(&context.style_context);
        context
            .style_sheet
            .apply_tooltip(&mut style, &context.style_context, &self.visual);
        context.style_sheet.apply_tooltip_state(
            &mut style,
            &context.style_context,
            &self.visual,
            visual.widget_state,
        );
        style
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_text_tooltip<VM>(
    text: String,
    anchor_frame: Rect,
    placement: crate::ui::widget::OverlayPlacement,
    flip_policy: crate::ui::widget::OverlayFlipPolicy,
    style: &TooltipStyle,
    background: Color,
    visibility: f32,
    animated_offset: Dp,
    overlay: Overlay<VM>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let foreground = style.foreground;
    let shadow_blur = style.shadow.blur.get() * visibility;

    let units = context.units;
    let font_manager = context.font_manager;
    let font_size = units.resolve_sp(style.text_style.size).max(1.0);
    let line_height = style
        .text_style
        .line_height
        .map(|h| units.resolve_sp(h))
        .unwrap_or(font_size * 1.4);
    let letter_spacing = style
        .text_style
        .letter_spacing
        .map(|s| units.resolve_sp(s))
        .unwrap_or(0.0);
    let request = TextFontRequest {
        preferred_font: style.text_style.font_family.as_deref(),
        weight: style.text_style.weight,
    };

    let max_text_w = (style.max_width - style.padding.left - style.padding.right).max(Dp::ZERO);
    let layout = font_manager.measure_text_layout_wrapped(
        &text,
        request.clone(),
        font_size,
        line_height,
        letter_spacing,
        max_text_w.get(),
    );
    let resolved_font = font_manager.resolve_text(&text, request);

    let text_w = Dp::from(layout.width).max(Dp::from(CARET_WIDTH));
    let text_h = Dp::from(layout.height).max(Dp::from(line_height));
    let content_w = text_w + style.padding.left + style.padding.right;
    let content_h = text_h + style.padding.top + style.padding.bottom;
    let pointer = style.pointer_size.max(Dp::ZERO);
    let overlay_w = if placement.side.is_vertical() {
        content_w
    } else {
        content_w + pointer
    };
    let overlay_h = if placement.side.is_vertical() {
        content_h + pointer
    } else {
        content_h
    };
    let placement_options = OverlayPlacementOptions {
        placement,
        offset: animated_offset,
        cross_offset: Dp::ZERO,
        flip: flip_policy,
        viewport_padding: insets_max(style.padding),
        clamp_to_viewport: true,
        match_anchor_width: false,
    };
    let solved = solve_placement(
        Anchor::Rect(anchor_frame),
        (overlay_w, overlay_h),
        context.viewport,
        &placement_options,
    );
    let bubble_origin = bubble_origin(solved.resolved_placement.side, pointer);
    let bg_rect = Rect::new(bubble_origin.0, bubble_origin.1, content_w, content_h);

    let bg = RenderPrimitive {
        rect: bg_rect,
        color: background,
        corner_radius: style.radius.get(),
        stroke_width: style.border_width.get(),
        clip_rect: None,
        clip_mask: None,
    };

    let text_frame = Rect::new(
        bubble_origin.0 + style.padding.left,
        bubble_origin.1 + style.padding.top,
        text_w,
        text_h,
    );
    let text_prim = TextPrimitive {
        content: Arc::from(text),
        rich_spans: None,
        frame: text_frame,
        quad: None,
        color: foreground,
        force_color: false,
        font_family: Some(Arc::from(resolved_font.primary_font)),
        font_size,
        font_weight: style.text_style.weight,
        line_height,
        letter_spacing,
        wrap: crate::ui::widget::CanvasTextWrap::Word,
        overflow: crate::ui::widget::CanvasTextOverflow::Clip,
        horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
        vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
        clip_rect: None,
        clip_mask: None,
    };

    let mut primitives = vec![
        OverlayPrimitive::Shape(bg),
        OverlayPrimitive::Text(text_prim),
    ];
    if shadow_blur > f32::EPSILON {
        primitives.insert(
            0,
            OverlayPrimitive::BackdropBlur(crate::ui::widget::BackdropBlurPrimitive {
                rect: bg_rect,
                corner_radius: style.radius.get(),
                blur_radius: shadow_blur,
                clip_rect: None,
                clip_mask: None,
            }),
        );
    }
    if let Some(pointer_mesh) = overlay_pointer_mesh(
        background,
        style.pointer_size,
        style.pointer_inset,
        &solved,
        bubble_origin,
        content_w,
        content_h,
        overlay_w,
        overlay_h,
    ) {
        primitives.push(OverlayPrimitive::Mesh(pointer_mesh));
    }
    let _ = emit_overlay(
        computed,
        context.viewport,
        overlay,
        (overlay_w, overlay_h),
        OverlayContent::Primitives(primitives),
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_element_tooltip<VM: 'static>(
    content: &Element<VM>,
    anchor_frame: Rect,
    placement: crate::ui::widget::OverlayPlacement,
    flip_policy: crate::ui::widget::OverlayFlipPolicy,
    style: &TooltipStyle,
    background: Color,
    _visibility: f32,
    animated_offset: Dp,
    overlay: Overlay<VM>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let Some((content_scene, content_size)) = build_tooltip_scene(content, style, context) else {
        return;
    };

    let pointer = style.pointer_size.max(Dp::ZERO);
    let overlay_w = if placement.side.is_vertical() {
        content_size.0
    } else {
        content_size.0 + pointer
    };
    let overlay_h = if placement.side.is_vertical() {
        content_size.1 + pointer
    } else {
        content_size.1
    };
    let placement_options = OverlayPlacementOptions {
        placement,
        offset: animated_offset,
        cross_offset: Dp::ZERO,
        flip: flip_policy,
        viewport_padding: insets_max(style.padding),
        clamp_to_viewport: true,
        match_anchor_width: false,
    };
    let solved = solve_placement(
        Anchor::Rect(anchor_frame),
        (overlay_w, overlay_h),
        context.viewport,
        &placement_options,
    );
    let bubble_origin = bubble_origin(solved.resolved_placement.side, pointer);

    let mut primitives = Vec::new();
    if let Some(pointer_mesh) = overlay_pointer_mesh(
        background,
        style.pointer_size,
        style.pointer_inset,
        &solved,
        bubble_origin,
        content_size.0,
        content_size.1,
        overlay_w,
        overlay_h,
    ) {
        primitives.push(OverlayPrimitive::Mesh(pointer_mesh));
    }

    computed
        .dependencies
        .merge_from(&content_scene.dependencies);

    let _ = emit_overlay(
        computed,
        context.viewport,
        overlay,
        (overlay_w, overlay_h),
        OverlayContent::SceneWithPrimitives {
            scene: Box::new(content_scene),
            scene_offset: crate::ui::widget::Point::new(bubble_origin.0, bubble_origin.1),
            primitives,
        },
    );
}

fn build_tooltip_scene<VM: 'static>(
    content: &Element<VM>,
    style: &TooltipStyle,
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let (result, dependencies): (Option<(ComputedScene<VM>, _)>, DependencyGraph) =
        with_dependency_collection(|| {
            super::super::tree::with_widget_stack(|| {
                let mut root: Element<VM> = Stack::new()
                    .padding(style.padding)
                    .child(content.clone())
                    .into();
                root.background = Some(Value::Static(style.background));
                root.visual.border_color = Some(Value::Static(style.border));
                root.visual.border_width = Some(Value::Static(style.border_width));
                root.visual.border_radius = Some(Value::Static(style.radius));
                root.visual.shadow = Some(Value::Static(style.shadow.clone()));
                root.layout.max_width = Some(Value::Static(Length::Px(style.max_width)));

                super::prepare_nested_scene_root(&mut root, context, context.viewport);
                let resolved = root.resolve(context.theme);
                let mut taffy = TaffyTree::new();
                let layout_root = resolved
                    .build_layout_tree(
                        &mut taffy,
                        context.animations,
                        context.theme,
                        context.units,
                        None,
                        context.viewport,
                        false,
                        context.now,
                    )
                    .ok()?;
                taffy
                    .compute_layout_with_measure(
                        layout_root.node,
                        TaffySize {
                            width: AvailableSpace::Definite(context.viewport.width.get()),
                            height: AvailableSpace::Definite(context.viewport.height.get()),
                        },
                        |known_dimensions, _, _, node_context, _| {
                            measure_node(
                                node_context,
                                known_dimensions,
                                context.font_manager,
                                context.theme,
                                context.media,
                                context.units,
                            )
                        },
                    )
                    .ok()?;
                let layout = taffy.layout(layout_root.node).ok()?;
                let size = (Dp::new(layout.size.width), Dp::new(layout.size.height));
                let local_bounds = Rect::new(Dp::ZERO, Dp::ZERO, size.0, size.1);

                let mut lifecycle_states = std::collections::HashMap::new();
                let mut chunks = std::collections::HashMap::new();
                let mut chunk_parts = std::collections::HashMap::new();
                let mut visual_contexts = std::collections::HashMap::new();
                let mut local_context = CollectContext {
                    taffy: &taffy,
                    font_manager: context.font_manager,
                    theme: context.theme,
                    style_context: context.style_context,
                    style_sheet: context.style_sheet,
                    media: context.media,
                    focused_input: context.focused_input,
                    focused_text_state: context.focused_text_state,
                    focused_text_value: context.focused_text_value,
                    focused_text_layout: context.focused_text_layout,
                    text_layout_overrides: context.text_layout_overrides,
                    active_slider_value: context.active_slider_value,
                    caret_visible: context.caret_visible,
                    selected_text: context.selected_text,
                    selected_text_state: context.selected_text_state,
                    hovered_scrollbar: context.hovered_scrollbar,
                    active_scrollbar: context.active_scrollbar,
                    widget_states: context.widget_states,
                    select_open_states: context.select_open_states,
                    menu_open_states: context.menu_open_states,
                    menubar_active_states: context.menubar_active_states,
                    context_menu_anchor_states: context.context_menu_anchor_states,
                    scroll_offsets: context.scroll_offsets,
                    virtual_states: context.virtual_states,
                    viewport: context.viewport,
                    units: context.units,
                    animations: context.animations,
                    reduced_motion: context.reduced_motion,
                    now: context.now,
                    focus: Default::default(),
                    tooltip_hover_started_at: context.tooltip_hover_started_at,
                    next_tooltip_wakeup: context.next_tooltip_wakeup,
                    next_toast_wakeup: context.next_toast_wakeup,
                    active_tooltip: context.active_tooltip,
                    active_hover_popover: context.active_hover_popover,
                };
                let root_id = resolved.collect_subtree_cache(
                    &layout_root,
                    VisualContext {
                        origin: crate::ui::widget::Point::ZERO,
                        opacity: 1.0,
                        clip_rect: local_bounds,
                        overflow_clip_rect: None,
                        clip_mask: None,
                    },
                    &mut local_context,
                    &mut lifecycle_states,
                    &mut chunks,
                    &mut chunk_parts,
                    &mut visual_contexts,
                );
                let mut computed = chunks.get(&root_id).cloned().unwrap_or_default();
                computed.finalize_portals(context.viewport);
                Some((computed, size))
            })
        });

    let (mut computed, size) = result?;
    computed.dependencies = dependencies.clone();
    Some((computed, size))
}

fn default_tooltip_transition() -> Transition {
    Transition::ease_in_out(std::time::Duration::from_millis(140))
}

fn insets_max(insets: Insets) -> Dp {
    let mut m = insets.left;
    if insets.top > m {
        m = insets.top;
    }
    if insets.right > m {
        m = insets.right;
    }
    if insets.bottom > m {
        m = insets.bottom;
    }
    m
}

pub(super) fn bubble_origin(side: OverlaySide, pointer: Dp) -> (Dp, Dp) {
    match side {
        OverlaySide::Top => (Dp::ZERO, Dp::ZERO),
        OverlaySide::Bottom => (Dp::ZERO, pointer),
        OverlaySide::Left => (Dp::ZERO, Dp::ZERO),
        OverlaySide::Right => (pointer, Dp::ZERO),
    }
}

pub(super) fn overlay_pointer_mesh(
    color: Color,
    pointer_size: Dp,
    pointer_inset: Dp,
    solved: &OverlaySolvedPlacement,
    bubble_origin: (Dp, Dp),
    content_w: Dp,
    content_h: Dp,
    overlay_w: Dp,
    overlay_h: Dp,
) -> Option<MeshPrimitive> {
    let pointer = pointer_size.max(Dp::ZERO);
    if pointer.get() <= 0.0 {
        return None;
    }
    let side = solved.resolved_placement.side;
    let base_span = (pointer.get() * 1.8).max(pointer.get());
    let inset = pointer_inset.get().max(0.0);
    let (tip, base_a, base_b) = match side {
        OverlaySide::Top => {
            let center_x = pointer_anchor_x(
                solved,
                bubble_origin.0.get(),
                content_w.get(),
                inset,
                base_span,
            );
            (
                [center_x, overlay_h.get()],
                [center_x - base_span / 2.0, overlay_h.get() - pointer.get()],
                [center_x + base_span / 2.0, overlay_h.get() - pointer.get()],
            )
        }
        OverlaySide::Bottom => {
            let center_x = pointer_anchor_x(
                solved,
                bubble_origin.0.get(),
                content_w.get(),
                inset,
                base_span,
            );
            (
                [center_x, 0.0],
                [center_x - base_span / 2.0, pointer.get()],
                [center_x + base_span / 2.0, pointer.get()],
            )
        }
        OverlaySide::Left => {
            let center_y = pointer_anchor_y(
                solved,
                bubble_origin.1.get(),
                content_h.get(),
                inset,
                base_span,
            );
            (
                [overlay_w.get(), center_y],
                [overlay_w.get() - pointer.get(), center_y - base_span / 2.0],
                [overlay_w.get() - pointer.get(), center_y + base_span / 2.0],
            )
        }
        OverlaySide::Right => {
            let center_y = pointer_anchor_y(
                solved,
                bubble_origin.1.get(),
                content_h.get(),
                inset,
                base_span,
            );
            (
                [0.0, center_y],
                [pointer.get(), center_y - base_span / 2.0],
                [pointer.get(), center_y + base_span / 2.0],
            )
        }
    };

    let brush_meta = [0.0, 1.0, 0.0, 0.0];
    let stop_color = color.to_linear_rgba_f32();
    let mut stop_colors = [[0.0; 4]; 8];
    stop_colors[0] = stop_color;
    stop_colors[1] = stop_color;
    let vertices = [tip, base_a, base_b]
        .into_iter()
        .map(|position| MeshVertex {
            position,
            local_position: position,
            brush_meta,
            gradient_data0: [0.0; 4],
            gradient_data1: [0.0; 4],
            stop_offsets0: [0.0; 4],
            stop_offsets1: [0.0; 4],
            stop_colors,
        })
        .collect::<Vec<_>>();
    let triangles = [[
        crate::ui::widget::Point::new(tip[0], tip[1]),
        crate::ui::widget::Point::new(base_a[0], base_a[1]),
        crate::ui::widget::Point::new(base_b[0], base_b[1]),
    ]];
    Some(MeshPrimitive {
        vertices: Arc::from(vertices),
        triangles: Arc::from(triangles),
        clip_rect: None,
        clip_mask: None,
    })
}

fn pointer_anchor_x(
    solved: &OverlaySolvedPlacement,
    bubble_x: f32,
    bubble_w: f32,
    inset: f32,
    base_span: f32,
) -> f32 {
    let anchor_center =
        (solved.anchor_rect.x - solved.rect.x).get() + solved.anchor_rect.width.get() * 0.5;
    clamp_pointer_anchor(anchor_center, bubble_x, bubble_w, inset, base_span)
}

fn pointer_anchor_y(
    solved: &OverlaySolvedPlacement,
    bubble_y: f32,
    bubble_h: f32,
    inset: f32,
    base_span: f32,
) -> f32 {
    let anchor_center =
        (solved.anchor_rect.y - solved.rect.y).get() + solved.anchor_rect.height.get() * 0.5;
    clamp_pointer_anchor(anchor_center, bubble_y, bubble_h, inset, base_span)
}

fn clamp_pointer_anchor(
    anchor_center: f32,
    bubble_origin: f32,
    bubble_extent: f32,
    inset: f32,
    base_span: f32,
) -> f32 {
    let min = bubble_origin + inset + base_span * 0.5;
    let max = bubble_origin + bubble_extent - inset - base_span * 0.5;
    if min <= max {
        anchor_center.clamp(min, max)
    } else {
        bubble_origin + bubble_extent * 0.5
    }
}
