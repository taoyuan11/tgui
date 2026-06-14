use taffy::prelude::{AvailableSpace, TaffyTree};
use taffy::Size as TaffySize;

use crate::foundation::binding::{with_dependency_collection, DependencyGraph};
use crate::log::Log;
use crate::runtime::overlay::{
    Anchor, AnchorKey, Overlay, OverlayContent, OverlayId, OverlayLayer, PlacementOptions,
};
use crate::runtime::portal::ExternalPortalRequest;
use crate::ui::unit::Dp;
use crate::ui::widget::core::measure_node;
use crate::ui::widget::{
    ComputedScene, Element, FocusScopeState, PortalAnchor, PortalTarget, Rect,
};

use super::super::scene::{CollectContext, VisualContext};
use super::super::types::{ResolvedElement, ResolvedWidgetKind};
use super::CollectVisualState;

const PORTAL_OVERLAY_TAG: u64 = 0x504F5254414C5F31; // "PORTAL_1"

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn emit_portal_if_open(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) {
        let ResolvedWidgetKind::Portal {
            content,
            open,
            target,
            anchor,
            options,
            layer,
            on_open_change,
            return_focus_to,
            close_on_outside_click,
            close_on_escape,
            focus_scope,
        } = &self.kind
        else {
            return;
        };

        if !open.resolve() {
            return;
        }

        match target {
            PortalTarget::CurrentWindow => {
                let anchor = anchor.clone().unwrap_or(PortalAnchor::SelfWidget);
                let Some(anchor) = resolve_portal_anchor(
                    self.id,
                    anchor,
                    visual.frame,
                    context.viewport,
                    true,
                    computed,
                ) else {
                    return;
                };
                let Some((content_scene, content_size)) =
                    collect_portal_content_scene(content.as_ref(), context)
                else {
                    return;
                };
                computed
                    .dependencies
                    .merge_from(&content_scene.dependencies);
                let portal = build_overlay(
                    self.id,
                    OverlayId::new(self.id.raw() ^ PORTAL_OVERLAY_TAG),
                    anchor,
                    options.clone(),
                    *layer,
                    on_open_change.clone(),
                    *return_focus_to,
                    *close_on_outside_click,
                    *close_on_escape,
                    focus_scope.clone(),
                );
                let _ = crate::runtime::overlay::collect::emit_overlay(
                    computed,
                    context.viewport,
                    portal,
                    content_size,
                    OverlayContent::Scene(Box::new(content_scene)),
                );
            }
            PortalTarget::WindowKey(target_window_key) => {
                let anchor = anchor.clone().unwrap_or(PortalAnchor::Viewport);
                if matches!(anchor, PortalAnchor::SelfWidget) {
                    Log::with_tag("tgui-portal").debug(format_args!(
                        "cross-window Portal {:?} requested SelfWidget anchor; skipping",
                        self.id
                    ));
                    return;
                }
                computed
                    .external_portal_requests
                    .push(ExternalPortalRequest {
                        source_widget_id: self.id,
                        overlay_id: OverlayId::new(self.id.raw() ^ PORTAL_OVERLAY_TAG),
                        target_window_key: target_window_key.clone(),
                        anchor,
                        options: options.clone(),
                        layer: *layer,
                        content: content.clone(),
                        on_open_change: on_open_change.clone(),
                        return_focus_to: *return_focus_to,
                        close_on_outside_click: *close_on_outside_click,
                        close_on_escape: *close_on_escape,
                        focus_scope: focus_scope.clone(),
                    });
            }
        }
    }
}

pub(crate) fn collect_portal_content_scene<VM: 'static>(
    content: &Element<VM>,
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let (result, dependencies): (Option<(ComputedScene<VM>, _)>, DependencyGraph) =
        with_dependency_collection(|| {
            super::super::tree::with_widget_stack(|| {
                let mut root = content.clone();
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
                    gpu_scroll_enabled: false,
                    gpu_scroll_container: None,
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
    computed.dependencies = dependencies;
    Some((computed, size))
}

pub(crate) fn resolve_external_portal_anchor<VM>(
    request: &ExternalPortalRequest<VM>,
    viewport: Rect,
    computed: &mut ComputedScene<VM>,
) -> Option<Anchor> {
    resolve_portal_anchor(
        request.source_widget_id,
        request.anchor.clone(),
        Rect::new(Dp::ZERO, Dp::ZERO, Dp::ZERO, Dp::ZERO),
        viewport,
        false,
        computed,
    )
}

pub(crate) fn build_external_portal_overlay<VM>(
    request: &ExternalPortalRequest<VM>,
    anchor: Anchor,
) -> Overlay<VM> {
    build_overlay(
        request.source_widget_id,
        request.overlay_id,
        anchor,
        request.options.clone(),
        request.layer,
        request.on_open_change.clone(),
        request.return_focus_to,
        request.close_on_outside_click,
        request.close_on_escape,
        request.focus_scope.clone(),
    )
}

fn resolve_portal_anchor<VM>(
    widget_id: crate::ui::widget::WidgetId,
    anchor: PortalAnchor,
    self_frame: Rect,
    viewport: Rect,
    allow_self_widget: bool,
    computed: &mut ComputedScene<VM>,
) -> Option<Anchor> {
    match anchor {
        PortalAnchor::SelfWidget if allow_self_widget => {
            computed.register_widget_overlay_anchor(widget_id, self_frame);
            Some(Anchor::Key(AnchorKey::widget(widget_id)))
        }
        PortalAnchor::SelfWidget => None,
        PortalAnchor::Viewport => Some(Anchor::Rect(viewport)),
        PortalAnchor::Rect(rect) => Some(Anchor::Rect(rect)),
        PortalAnchor::Point(point) => Some(Anchor::Point(point)),
        PortalAnchor::Key(key) => Some(Anchor::Key(key)),
    }
}

fn build_overlay<VM>(
    widget_id: crate::ui::widget::WidgetId,
    overlay_id: OverlayId,
    anchor: Anchor,
    options: PlacementOptions,
    layer: OverlayLayer,
    on_open_change: Option<crate::foundation::view_model::ValueCommand<VM, bool>>,
    return_focus_to: Option<crate::ui::widget::WidgetId>,
    close_on_outside_click: bool,
    close_on_escape: bool,
    focus_scope: Option<crate::ui::widget::FocusScopeOptions>,
) -> Overlay<VM> {
    let mut overlay = Overlay::<VM>::new(overlay_id, anchor)
        .source_widget(widget_id)
        .layer(layer);
    overlay.options = options;
    if let Some(command) = on_open_change {
        overlay = overlay
            .on_close(command)
            .close_on_outside_click(close_on_outside_click)
            .close_on_escape(close_on_escape);
        if let Some(target) = return_focus_to {
            overlay = overlay.return_focus_to(target);
        }
    }
    if let Some(options) = focus_scope {
        overlay = overlay.focus_scope(FocusScopeState {
            scope_id: widget_id,
            path: vec![widget_id],
            active: options.is_active(),
            options,
        });
    }
    overlay
}
