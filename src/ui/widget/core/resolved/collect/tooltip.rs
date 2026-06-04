//! Tooltip 在 collect 阶段的浮层渲染。
//!
//! Tooltip 由 runtime 先解析”当前唯一激活的 tooltip”，collect 阶段只负责：
//! - hover trigger 的 delay 门控与 wakeup 注册；
//! - 按最终 solved placement 画背景、文字、阴影和三角指针；
//! - 对 focus / long-press tooltip 接入外部点击与 Esc 关闭。

use std::sync::Arc;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::color::Color;
use crate::text::font::TextFontRequest;
use crate::ui::layout::Insets;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    ComputedScene, MeshPrimitive, MeshVertex, Rect, RenderPrimitive, TextPrimitive,
};
use crate::ui::widget::overlay::{
    collect::emit_overlay, solve_placement, Anchor, AnchorKey, Overlay, OverlayContent, OverlayId,
    OverlayLayer, OverlayPrimitive,
};

use super::super::scene::{CollectContext, TooltipTrigger};
use super::super::types::ResolvedElement;
use super::super::CARET_WIDTH;
use super::CollectVisualState;
use crate::ui::widget::{OverlayPlacementOptions, OverlaySide, OverlaySolvedPlacement};

impl<VM> ResolvedElement<VM> {
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

        let text = tooltip.text.resolve();
        if text.is_empty() {
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

        let theme_mode = crate::ui::widget::style::infer_theme_mode(context.theme);
        let style = Box::new(tooltip.resolved_style(theme_mode));
        let background = style.background;
        let foreground = style.foreground;
        let shadow_blur = style.shadow.blur.get() * visibility;
        let animated_offset = (style.offset - Dp::from((1.0 - visibility) * 4.0)).max(Dp::ZERO);

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
        let overlay_w = if tooltip.placement.side.is_vertical() {
            content_w
        } else {
            content_w + pointer
        };
        let overlay_h = if tooltip.placement.side.is_vertical() {
            content_h + pointer
        } else {
            content_h
        };
        let placement_options = OverlayPlacementOptions {
            placement: tooltip.placement,
            offset: animated_offset,
            cross_offset: Dp::ZERO,
            flip: tooltip.flip_policy,
            viewport_padding: insets_max(style.padding),
            clamp_to_viewport: true,
            match_anchor_width: false,
        };
        let solved = solve_placement(
            Anchor::Rect(visual.frame),
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
        if let Some(pointer_mesh) = tooltip_pointer_mesh(
            background,
            &style,
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

fn bubble_origin(side: OverlaySide, pointer: Dp) -> (Dp, Dp) {
    match side {
        OverlaySide::Top => (Dp::ZERO, Dp::ZERO),
        OverlaySide::Bottom => (Dp::ZERO, pointer),
        OverlaySide::Left => (Dp::ZERO, Dp::ZERO),
        OverlaySide::Right => (pointer, Dp::ZERO),
    }
}

fn tooltip_pointer_mesh(
    color: Color,
    style: &crate::ui::widget::TooltipStyle,
    solved: &OverlaySolvedPlacement,
    bubble_origin: (Dp, Dp),
    content_w: Dp,
    content_h: Dp,
    overlay_w: Dp,
    overlay_h: Dp,
) -> Option<MeshPrimitive> {
    let pointer = style.pointer_size.max(Dp::ZERO);
    if pointer.get() <= 0.0 {
        return None;
    }
    let side = solved.resolved_placement.side;
    let base_span = (pointer.get() * 1.8).max(pointer.get());
    let inset = style.pointer_inset.get().max(0.0);
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
