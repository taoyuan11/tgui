use taffy::prelude::{AvailableSpace, TaffyTree};
use taffy::Size as TaffySize;

use crate::foundation::binding::{with_dependency_collection, DependencyGraph};
use crate::ui::layout::Length;
use crate::ui::widget::container::Stack;
use crate::ui::widget::core::measure_node;
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, AnchorKey, Overlay, OverlayContent, OverlayId, OverlayLayer,
};
use crate::ui::widget::style::PopoverStyle;
use crate::ui::widget::{ComputedScene, Element, Rect};

use super::super::scene::{CollectContext, VisualContext};
use super::super::types::ResolvedElement;
use super::CollectVisualState;

const POPOVER_OVERLAY_TAG: u64 = 0x504F504F56455231; // "POPOVER1"

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn emit_popover_overlay_if_visible(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) {
        let Some(popover) = &self.popover else {
            return;
        };
        if popover.disabled.resolve() {
            return;
        }

        let fixed_open = popover.open.resolve();
        let hover_preview =
            context.active_hover_popover == Some(self.id) && popover.trigger_mode.allows_hover();
        if !fixed_open && !hover_preview {
            return;
        }

        let theme_mode = crate::ui::widget::style::infer_theme_mode(context.theme);
        let style = popover.resolved_style(theme_mode);
        let anchor_width = popover
            .match_anchor_width
            .then_some(visual.frame.width.max(style.min_width).min(style.max_width));
        let Some((content_scene, content_size)) =
            build_popover_scene(popover.content.as_ref(), &style, anchor_width, context)
        else {
            return;
        };

        computed
            .dependencies
            .merge_from(&content_scene.dependencies);
        computed.register_widget_overlay_anchor(self.id, visual.frame);

        let overlay_id = OverlayId::new(self.id.raw() ^ POPOVER_OVERLAY_TAG);
        let mut overlay = Overlay::<VM>::new(overlay_id, Anchor::Key(AnchorKey::widget(self.id)))
            .source_widget(self.id)
            .placement(popover.placement)
            .offset(style.offset)
            .flip_policy(popover.flip_policy)
            .viewport_padding(insets_max(style.padding))
            .match_anchor_width(popover.match_anchor_width)
            .layer(OverlayLayer::Popover);
        if fixed_open {
            if let Some(command) = popover.on_open_change.clone() {
                overlay = overlay
                    .close_on_escape(popover.close_on_escape)
                    .close_on_outside_click(popover.close_on_outside_click)
                    .return_focus_to(self.id)
                    .on_close(command);
            }
        }

        let _ = emit_overlay(
            computed,
            context.viewport,
            overlay,
            content_size,
            OverlayContent::Scene(Box::new(content_scene)),
        );
    }
}

fn build_popover_scene<VM: 'static>(
    content: &Element<VM>,
    style: &PopoverStyle,
    anchor_width: Option<crate::ui::unit::Dp>,
    context: &mut CollectContext<'_, '_>,
) -> Option<(
    ComputedScene<VM>,
    (crate::ui::unit::Dp, crate::ui::unit::Dp),
)> {
    let (result, dependencies): (Option<(ComputedScene<VM>, _)>, DependencyGraph) =
        with_dependency_collection(|| {
            let mut root: Element<VM> = Stack::new()
                .padding(style.padding)
                .child(content.clone())
                .into();
            root.background = Some(style.background.clone());
            root.visual.background_brush = style.surface.background_brush.clone();
            root.visual.background_image = style.surface.background_image.clone();
            root.visual.background_blur = style.surface.background_blur.clone();
            root.visual.border_color = Some(style.border.clone());
            root.visual.border_width = Some(style.border_width.clone());
            root.visual.border_radius = Some(style.radius.clone());
            root.visual.shadow = Some(crate::ui::layout::Value::Static(style.shadow.clone()));
            root.visual.opacity = style.surface.opacity.clone();
            root.visual.offset = style.surface.offset.clone();
            root.layout.min_width = Some(crate::ui::layout::Value::Static(Length::Px(
                style.min_width,
            )));
            root.layout.max_width = Some(crate::ui::layout::Value::Static(Length::Px(
                style.max_width,
            )));
            if let Some(width) = anchor_width {
                root.layout.width = Some(crate::ui::layout::Value::Static(Length::Px(width)));
            }

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
            let size = (
                crate::ui::unit::Dp::new(layout.size.width),
                crate::ui::unit::Dp::new(layout.size.height),
            );
            let local_bounds = Rect::new(
                crate::ui::unit::Dp::ZERO,
                crate::ui::unit::Dp::ZERO,
                size.0,
                size.1,
            );

            let mut lifecycle_states = std::collections::HashMap::new();
            let mut chunks = std::collections::HashMap::new();
            let mut chunk_parts = std::collections::HashMap::new();
            let mut visual_contexts = std::collections::HashMap::new();
            let mut local_context = CollectContext {
                taffy: &taffy,
                font_manager: context.font_manager,
                theme: context.theme,
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
                scroll_offsets: context.scroll_offsets,
                viewport: context.viewport,
                units: context.units,
                animations: context.animations,
                now: context.now,
                focus: Default::default(),
                tooltip_hover_started_at: context.tooltip_hover_started_at,
                next_tooltip_wakeup: context.next_tooltip_wakeup,
                active_tooltip: context.active_tooltip,
                active_hover_popover: context.active_hover_popover,
            };
            let mut computed = resolved.collect_subtree_cache(
                &layout_root,
                VisualContext {
                    origin: crate::ui::widget::Point::ZERO,
                    opacity: 1.0,
                    clip_rect: local_bounds,
                    clip_mask: None,
                },
                &mut local_context,
                &mut lifecycle_states,
                &mut chunks,
                &mut chunk_parts,
                &mut visual_contexts,
            );
            computed.finalize_portals(context.viewport);
            Some((computed, size))
        });
    let (mut computed, size) = result?;

    computed.dependencies = dependencies.clone();
    Some((computed, size))
}

fn insets_max(insets: crate::ui::layout::Insets) -> crate::ui::unit::Dp {
    let mut value = insets.left;
    if insets.top > value {
        value = insets.top;
    }
    if insets.right > value {
        value = insets.right;
    }
    if insets.bottom > value {
        value = insets.bottom;
    }
    value
}
