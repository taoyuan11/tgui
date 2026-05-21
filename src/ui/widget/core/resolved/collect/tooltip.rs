//! Tooltip 在 collect 阶段的浮层渲染。
//!
//! 每个 widget 在自己的 collect 完成后，由 `collect_subtree_cache_tracked` 末尾统一调用
//! `emit_tooltip`：若 widget 当前 hover 且挂了 `Tooltip` 描述符，则根据 hover 已持续时间
//! 决定是否触发渲染——若尚未达到 `tooltip.delay`，仅向 runtime 注册一次唤醒；
//! 达到后才把 tooltip 写入 overlay 层。
//!
//! Tooltip 不挂 close handler——hover 离开后下一帧 collect 不再 emit，浮层自然消失。

use crate::text::font::TextFontRequest;
use crate::ui::layout::Insets;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{ComputedScene, RenderPrimitive, Rect, TextPrimitive};
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer,
    OverlayPrimitive,
};

use super::super::scene::CollectContext;
use super::super::types::ResolvedElement;
use super::super::CARET_WIDTH;
use super::CollectVisualState;

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
        if !visual.widget_state.hovered {
            return;
        }

        // 时间门控：若 hover 持续未达 delay，仅安排一次唤醒，不写 primitive。
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
                return;
            }
        } else if !tooltip.delay.is_zero() {
            // runtime 还没记录起始时间（首帧 / 罕见竞态）：保守等一帧。
            let wakeup = context.now + tooltip.delay;
            let prev = context.next_tooltip_wakeup.get();
            let merged = match prev {
                Some(p) => Some(p.min(wakeup)),
                None => Some(wakeup),
            };
            context.next_tooltip_wakeup.set(merged);
            return;
        }

        let text = tooltip.text.resolve();
        if text.is_empty() {
            return;
        }

        let theme_mode = crate::ui::widget::style::infer_theme_mode(context.theme);
        let style = tooltip.resolved_style(theme_mode);

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

        // 局部坐标 primitives（emit_overlay 会平移到窗口坐标）。
        let bg_rect = Rect::new(Dp::ZERO, Dp::ZERO, content_w, content_h);
        let bg = RenderPrimitive {
            rect: bg_rect,
            color: style.background,
            corner_radius: style.radius.get(),
            stroke_width: style.border_width.get(),
            clip_rect: None,
            clip_mask: None,
        };

        let text_frame = Rect::new(style.padding.left, style.padding.top, text_w, text_h);
        let text_prim = TextPrimitive {
            content: text,
            rich_spans: None,
            frame: text_frame,
            quad: None,
            color: style.foreground,
            force_color: false,
            font_family: Some(resolved_font.primary_font),
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

        let overlay = Overlay::<VM>::new(OverlayId::new(self.id.raw()), Anchor::Rect(visual.frame))
            .placement(tooltip.placement)
            .offset(style.offset)
            .flip_policy(tooltip.flip_policy)
            .viewport_padding(insets_max(style.padding))
            .layer(OverlayLayer::Tooltip);

        let _ = emit_overlay(
            computed,
            context.viewport,
            overlay,
            (content_w, content_h),
            OverlayContent::Primitives(vec![
                OverlayPrimitive::Shape(bg),
                OverlayPrimitive::Text(text_prim),
            ]),
        );
    }
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
