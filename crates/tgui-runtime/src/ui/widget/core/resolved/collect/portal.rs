use taffy::prelude::TaffyTree;

use crate::foundation::binding::{with_dependency_collection, DependencyGraph};
use crate::log::Log;
use crate::runtime::overlay::{
    Anchor, AnchorKey, Overlay, OverlayContent, OverlayId, OverlayLayer, PlacementOptions,
};
use crate::runtime::portal::ExternalPortalRequest;
use crate::ui::unit::Dp;
use crate::ui::widget::core::compute_taffy_layout_with_measure;
use crate::ui::widget::{
    AccessibilityFragment, AccessibilityFragmentNode, ComputedScene, Element, FocusScopeState,
    HitRegion, LifecycleEventState, PortalAnchor, PortalTarget, Rect, ScrollRegion, WidgetId,
};

use super::super::LayoutNode;

use super::super::scene::{CollectContext, PortalAccessibilityGeometryRecord, VisualContext};
use super::super::types::{ResolvedElement, ResolvedWidgetKind};
use super::CollectVisualState;

const PORTAL_OVERLAY_TAG: u64 = 0x504F5254414C5F31; // "PORTAL_1"

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn emit_portal_if_open(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
        lifecycle_states: &mut std::collections::HashMap<WidgetId, LifecycleEventState<VM>>,
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
                let Some((content_scene, content_size, content_lifecycle_states)) =
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
                    Some(self.id),
                );
                let solved = crate::runtime::overlay::collect::emit_overlay(
                    computed,
                    context.viewport,
                    portal,
                    content_size,
                    OverlayContent::Scene(Box::new(content_scene)),
                );
                if !solved.was_hidden {
                    lifecycle_states.extend(content_lifecycle_states);
                }
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
                lifecycle_states.extend(collect_portal_source_lifecycle_states(
                    content.as_ref(),
                    context,
                ));
                computed
                    .external_portal_requests
                    .push(ExternalPortalRequest {
                        source_window_instance_id: None,
                        source_publication_generation: 0,
                        source_open: open.clone(),
                        focus_scope_instance_id: None,
                        source_widget_id: self.id,
                        overlay_id: OverlayId::new(self.id.raw() ^ PORTAL_OVERLAY_TAG),
                        target_window_key: target_window_key.clone(),
                        anchor,
                        options: options.clone(),
                        layer: *layer,
                        content: std::sync::Arc::new(content.as_ref().clone()),
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
) -> Option<(
    ComputedScene<VM>,
    (Dp, Dp),
    std::collections::HashMap<WidgetId, LifecycleEventState<VM>>,
)> {
    let (result, dependencies): (Option<(ComputedScene<VM>, _, _)>, DependencyGraph) =
        with_dependency_collection(|| {
            super::super::tree::with_widget_stack(|| {
                let mut root = content.clone();
                super::prepare_nested_scene_root(&mut root, context, context.viewport);
                let resolved = std::sync::Arc::new(root.resolve(context.theme));
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
                compute_taffy_layout_with_measure(
                    &mut taffy,
                    layout_root.node,
                    context.viewport,
                    context.font_manager,
                    context.theme,
                    context.media,
                    context.units,
                )
                .ok()?;
                let layout = taffy.layout(layout_root.node).ok()?;
                let size = (Dp::new(layout.size.width), Dp::new(layout.size.height));
                let local_bounds = Rect::new(Dp::ZERO, Dp::ZERO, size.0, size.1);

                let mut lifecycle_states = std::collections::HashMap::new();
                let mut chunks = std::collections::HashMap::new();
                let mut chunk_parts = std::collections::HashMap::new();
                let mut visual_contexts = std::collections::HashMap::new();
                let mut accessibility_geometry = Vec::new();
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
                    frame_clock: context.frame_clock,
                    focus: Default::default(),
                    tooltip_hover_started_at: context.tooltip_hover_started_at,
                    next_tooltip_wakeup: context.next_tooltip_wakeup,
                    next_toast_wakeup: context.next_toast_wakeup,
                    active_tooltip: context.active_tooltip,
                    active_hover_popover: context.active_hover_popover,
                    gpu_scroll_enabled: false,
                    gpu_scroll_container: None,
                    transform_stack: context.transform_stack.clone(),
                    portal_accessibility_geometry: Some(&mut accessibility_geometry),
                    portal_accessibility_path: smallvec::SmallVec::new(),
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
                drop(local_context);
                let mut computed = chunks.get(&root_id).cloned().unwrap_or_default();
                if let Some(accessibility_fragment) = collect_accessibility_fragment(
                    std::sync::Arc::clone(&resolved),
                    &layout_root,
                    &accessibility_geometry,
                    &computed.hit_regions,
                    &computed.scroll_regions,
                ) {
                    computed
                        .accessibility_fragments
                        .push(accessibility_fragment);
                }
                computed.finalize_portals(context.viewport);
                Some((computed, size, lifecycle_states))
            })
        });
    let (mut computed, size, lifecycle_states) = result?;
    computed.dependencies = dependencies;
    // Cross-window Portal transport is intentionally one hop. Nested external requests need a
    // provenance/lease model (and cycle handling) before they can be forwarded safely; dropping
    // only that metadata preserves the outer Portal's visuals, hits, and accessibility fragment.
    computed.external_portal_requests.clear();
    Some((computed, size, lifecycle_states))
}

fn collect_portal_source_lifecycle_states<VM: 'static>(
    content: &Element<VM>,
    context: &CollectContext<'_, '_>,
) -> std::collections::HashMap<WidgetId, LifecycleEventState<VM>> {
    let mut root = content.clone();
    super::prepare_nested_scene_root(&mut root, context, context.viewport);
    let mut states = Vec::new();
    root.resolve(context.theme)
        .collect_lifecycle_event_states(&mut states);
    states
        .into_iter()
        .map(|state| (state.widget_id, state))
        .collect()
}

pub(super) fn collect_accessibility_fragment<VM>(
    resolved_root: std::sync::Arc<ResolvedElement<VM>>,
    layout_node: &LayoutNode,
    geometry: &[PortalAccessibilityGeometryRecord],
    hits: &[HitRegion<VM>],
    scroll_regions: &[ScrollRegion],
) -> Option<AccessibilityFragment<VM>> {
    let resolved = resolved_root.as_ref();
    let mut hits_by_widget =
        std::collections::HashMap::<WidgetId, smallvec::SmallVec<[HitRegion<VM>; 1]>>::new();
    for hit in hits {
        hits_by_widget
            .entry(hit.interaction.widget_id())
            .or_default()
            .push(hit.clone());
        if let Some(focus) = hit.focus.as_ref() {
            if focus.widget_id != hit.interaction.widget_id() {
                hits_by_widget
                    .entry(focus.widget_id)
                    .or_default()
                    .push(hit.clone());
            }
        }
    }
    let scroll_regions_by_widget = scroll_regions.iter().copied().fold(
        std::collections::HashMap::<WidgetId, smallvec::SmallVec<[ScrollRegion; 1]>>::new(),
        |mut by_widget, region| {
            by_widget.entry(region.id).or_default().push(region);
            by_widget
        },
    );
    let geometry_by_path = geometry
        .iter()
        .map(|record| (record.resolved_path.clone(), record))
        .collect::<std::collections::HashMap<_, _>>();
    let mut nodes = Vec::new();
    let mut path = smallvec::SmallVec::<[usize; 4]>::new();
    collect_accessibility_fragment_node(
        resolved,
        layout_node,
        &mut path,
        &geometry_by_path,
        &hits_by_widget,
        &scroll_regions_by_widget,
        &mut nodes,
    )?;
    let has_duplicate_widget_ids = resolved_tree_has_duplicate_widget_ids(resolved);
    Some(AccessibilityFragment {
        source_window_instance_id: None,
        source_publication_generation: None,
        source_open: None,
        owner_path: smallvec::SmallVec::new(),
        scope_path: Vec::new(),
        clip_rect: None,
        has_duplicate_widget_ids,
        resolved_root,
        nodes,
    })
}

fn resolved_tree_has_duplicate_widget_ids<VM>(root: &ResolvedElement<VM>) -> bool {
    fn visit<VM>(
        node: &ResolvedElement<VM>,
        seen: &mut std::collections::HashSet<WidgetId>,
    ) -> bool {
        if !seen.insert(node.id) {
            return true;
        }
        match &node.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => {
                children.iter().any(|child| visit(child, seen))
            }
            _ => false,
        }
    }

    visit(root, &mut std::collections::HashSet::new())
}

fn collect_accessibility_fragment_node<VM>(
    resolved: &ResolvedElement<VM>,
    layout_node: &LayoutNode,
    path: &mut smallvec::SmallVec<[usize; 4]>,
    geometry_by_path: &std::collections::HashMap<
        smallvec::SmallVec<[usize; 4]>,
        &PortalAccessibilityGeometryRecord,
    >,
    hits_by_widget: &std::collections::HashMap<WidgetId, smallvec::SmallVec<[HitRegion<VM>; 1]>>,
    scroll_regions_by_widget: &std::collections::HashMap<
        WidgetId,
        smallvec::SmallVec<[ScrollRegion; 1]>,
    >,
    nodes: &mut Vec<AccessibilityFragmentNode<VM>>,
) -> Option<usize> {
    let geometry = geometry_by_path.get(path)?;
    if geometry.widget_id != resolved.id {
        return None;
    }
    let resolved_children = match &resolved.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    };
    if resolved_children.len() != layout_node.children.len() {
        return None;
    }
    let node_index = nodes.len();
    nodes.push(AccessibilityFragmentNode {
        widget_id: resolved.id,
        resolved_path: path.clone(),
        bounds: geometry.frame,
        clip_rect: geometry.clip_rect,
        hits: hits_by_widget
            .get(&resolved.id)
            .cloned()
            .unwrap_or_default(),
        scroll_regions: scroll_regions_by_widget
            .get(&resolved.id)
            .cloned()
            .unwrap_or_default(),
        children: smallvec::SmallVec::new(),
        synthetic_semantics: None,
    });
    for (child_index, (child, child_layout)) in resolved_children
        .iter()
        .zip(layout_node.children.iter())
        .enumerate()
    {
        path.push(child_index);
        let result = collect_accessibility_fragment_node(
            child,
            child_layout,
            path,
            geometry_by_path,
            hits_by_widget,
            scroll_regions_by_widget,
            nodes,
        );
        path.pop();
        if let Some(child_node_index) = result {
            nodes[node_index]
                .children
                .push((child_index, child_node_index));
        }
    }
    Some(node_index)
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
        // Source-window WidgetIds are not valid focus targets in the host handler.
        None,
        request.close_on_outside_click,
        request.close_on_escape,
        request.focus_scope.clone(),
        request.focus_scope_instance_id,
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
    focus_scope_instance_id: Option<crate::ui::widget::WidgetId>,
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
    if let (Some(options), Some(scope_id)) = (focus_scope, focus_scope_instance_id) {
        overlay = overlay.focus_scope(FocusScopeState {
            scope_id,
            path: vec![scope_id],
            active: options.is_active(),
            options,
        });
    }
    overlay
}
