use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use crate::foundation::view_model::ValueCommand;
use crate::runtime::overlay::{OverlayContent, OverlayId, OverlayLayer, PlacementOptions};
use crate::ui::widget::{
    build_external_portal_overlay, collect_portal_content_scene, resolve_external_portal_anchor,
    CollectContext, ComputedScene, Element, FocusCollectState, FocusScopeOptions, MeasureContext,
    PortalAnchor, WidgetId,
};
use taffy::prelude::TaffyTree;

use super::BoundRuntimeHandler;

pub(crate) struct ExternalPortalRequest<VM> {
    pub(crate) source_window_instance_id: Option<u64>,
    pub(crate) source_publication_generation: u64,
    pub(crate) source_open: crate::ui::layout::Value<bool>,
    pub(crate) focus_scope_instance_id: Option<WidgetId>,
    pub(crate) source_widget_id: WidgetId,
    pub(crate) overlay_id: OverlayId,
    pub(crate) target_window_key: String,
    pub(crate) anchor: PortalAnchor,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) content: std::sync::Arc<Element<VM>>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) focus_scope: Option<FocusScopeOptions>,
}

impl<VM> Clone for ExternalPortalRequest<VM> {
    fn clone(&self) -> Self {
        Self {
            source_window_instance_id: self.source_window_instance_id,
            source_publication_generation: self.source_publication_generation,
            source_open: self.source_open.clone(),
            focus_scope_instance_id: self.focus_scope_instance_id,
            source_widget_id: self.source_widget_id,
            overlay_id: self.overlay_id,
            target_window_key: self.target_window_key.clone(),
            anchor: self.anchor.clone(),
            options: self.options.clone(),
            layer: self.layer,
            content: self.content.clone(),
            on_open_change: self.on_open_change.clone(),
            return_focus_to: self.return_focus_to,
            close_on_outside_click: self.close_on_outside_click,
            close_on_escape: self.close_on_escape,
            focus_scope: self.focus_scope.clone(),
        }
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn set_external_portal_requests(
        &mut self,
        requests: Vec<ExternalPortalRequest<VM>>,
        revision: u64,
    ) {
        if self.external_portal_revision == revision {
            return;
        }
        self.external_portal_requests = requests;
        self.external_portal_revision = revision;
        self.invalidate_computed_scene();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    pub(in crate::runtime) fn external_portal_requests_from_computed(
        &mut self,
    ) -> Vec<ExternalPortalRequest<VM>> {
        let source_window_instance_id = self.window_instance_id;
        let source_publication_generation = self.portal_publication_generation;
        let mut requests = self
            .computed_scene()
            .external_portal_requests
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut live_focus_scopes = std::collections::HashSet::new();
        for request in &mut requests {
            request.source_window_instance_id = Some(source_window_instance_id);
            request.source_publication_generation = source_publication_generation;
            if request.focus_scope.is_some() && request.source_open.resolve_untracked() {
                live_focus_scopes.insert(request.overlay_id);
                request.focus_scope_instance_id = Some(
                    *self
                        .external_portal_focus_scopes
                        .entry(request.overlay_id)
                        .or_insert_with(WidgetId::next),
                );
            } else {
                request.focus_scope_instance_id = None;
            }
        }
        self.external_portal_focus_scopes
            .retain(|overlay_id, _| live_focus_scopes.contains(overlay_id));
        requests
    }

    pub(in crate::runtime) fn append_external_portals_to_computed(
        &mut self,
        computed: &mut ComputedScene<VM>,
        widget_states: &crate::ui::widget::WidgetStateMap,
        now: Instant,
    ) {
        if self.external_portal_requests.is_empty() {
            return;
        }

        // 请求中持有完整的 portal `Element` 子树。目标窗口每次重收集都 clone 整组请求会
        // 深拷贝这些树；临时移出后按引用收集，结束时原样放回，既避开 clone，也保留
        // registry 给出的稳定顺序和请求身份。
        let requests = std::mem::take(&mut self.external_portal_requests);
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let theme = self.animated_theme(now);
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let focused_input = self.focused_text_input_id_cached(computed);
        let focused_text_state = focused_input
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let selected_text_state = self
            .selected_text
            .and_then(|id| self.text_edit_state(id))
            .cloned();
        let (focused_text_value, focused_text_layout) = Self::focused_text_overrides(
            &self.text_input_buffers,
            focused_input,
            focused_text_state.as_ref(),
        );
        let text_layout_overrides = Self::stable_text_layout_overrides(&self.text_input_buffers);
        let caret_visible = self.caret_visible_at(now, focused_input);
        let next_tooltip_wakeup = Cell::new(None);
        let next_toast_wakeup = Cell::new(None);
        let taffy: TaffyTree<MeasureContext> = TaffyTree::new();
        let style_context = crate::ui::theme::StyleContext::from_theme(&theme)
            .with_reduced_motion(self.reduced_motion)
            .with_text_scale(units.font_scale());

        let mut context = CollectContext {
            taffy: &taffy,
            font_manager: &self.font_manager,
            theme: &theme,
            style_context,
            style_sheet: &self.config.style_sheet,
            media: &self.media_manager,
            focused_input,
            focused_text_state: focused_text_state.as_ref(),
            focused_text_value,
            focused_text_layout,
            text_layout_overrides: Some(&text_layout_overrides),
            active_slider_value: None,
            caret_visible,
            selected_text: self.selected_text,
            selected_text_state: selected_text_state.as_ref(),
            hovered_scrollbar: self.hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states: &self.select_open_states,
            menu_open_states: &self.menu_open_states,
            menubar_active_states: &self.menubar_active_states,
            context_menu_anchor_states: &self.context_menu_anchor_states,
            scroll_offsets: &self.scroll_states,
            virtual_states: &self.virtual_states,
            viewport,
            units,
            animations: &mut self.animation_engine,
            reduced_motion: self.reduced_motion,
            now,
            frame_clock: self.frame_clock.snapshot(),
            focus: FocusCollectState::default(),
            tooltip_hover_started_at: &self.tooltip_hover_started_at,
            next_tooltip_wakeup: &next_tooltip_wakeup,
            next_toast_wakeup: &next_toast_wakeup,
            active_tooltip: None,
            active_hover_popover: None,
            gpu_scroll_enabled: false,
            gpu_scroll_container: None,
            transform_stack: smallvec::SmallVec::new(),
            portal_accessibility_geometry: None,
            portal_accessibility_path: smallvec::SmallVec::new(),
        };

        for request in &requests {
            if !request.source_open.resolve() {
                continue;
            }
            let Some(anchor) = resolve_external_portal_anchor(request, viewport, computed) else {
                continue;
            };
            let Some((mut content_scene, content_size)) =
                collect_portal_content_scene(request.content.as_ref(), &mut context)
            else {
                continue;
            };
            if let Some(source_window_instance_id) = request.source_window_instance_id {
                for fragment in &mut content_scene.accessibility_fragments {
                    fragment.source_window_instance_id = Some(source_window_instance_id);
                    fragment.source_publication_generation =
                        Some(request.source_publication_generation);
                    fragment.source_open = Some(request.source_open.clone());
                }
            } else {
                // A malformed/legacy request must never make visible Portal content disappear.
                // Without a real source instance its accessibility identity is unsafe, so only
                // omit that metadata and preserve rendering, hits, and close handlers.
                content_scene.accessibility_fragments.clear();
            }
            if request.focus_scope.is_some() && request.focus_scope_instance_id.is_none() {
                content_scene.accessibility_fragments.clear();
            }
            computed
                .dependencies
                .merge_from(&content_scene.dependencies);
            let overlay = build_external_portal_overlay(&request, anchor);
            let _ = crate::runtime::overlay::collect::emit_overlay(
                computed,
                viewport,
                overlay,
                content_size,
                OverlayContent::Scene(Box::new(content_scene)),
            );
        }
        self.external_portal_requests = requests;

        computed.finalize_additional_portals(viewport, std::iter::empty());

        if let Some(deadline) = next_tooltip_wakeup.get() {
            self.next_tooltip_wakeup_deadline = Some(
                self.next_tooltip_wakeup_deadline
                    .map(|current| current.min(deadline))
                    .unwrap_or(deadline),
            );
        }
        if let Some(deadline) = next_toast_wakeup.get() {
            self.next_toast_wakeup_deadline = Some(
                self.next_toast_wakeup_deadline
                    .map(|current| current.min(deadline))
                    .unwrap_or(deadline),
            );
        }
    }
}

impl<VM> ExternalPortalRequest<VM> {
    pub(crate) fn fingerprint(&self) -> String {
        format!(
            "{:?}|{}|{:?}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}|{:?}|{}",
            self.source_window_instance_id,
            self.source_publication_generation,
            self.source_widget_id,
            self.overlay_id,
            self.target_window_key,
            self.anchor,
            self.options,
            self.layer,
            self.return_focus_to,
            self.close_on_outside_click,
            self.close_on_escape,
            self.focus_scope,
            self.focus_scope_instance_id,
            self.content.id.raw(),
        )
    }
}

pub(crate) struct PortalRegistry<VM> {
    by_source: BTreeMap<String, Vec<ExternalPortalRequest<VM>>>,
    source_fingerprints: HashMap<String, Vec<String>>,
    target_revisions: HashMap<String, u64>,
    revision: u64,
}

impl<VM> Default for PortalRegistry<VM> {
    fn default() -> Self {
        Self {
            by_source: BTreeMap::new(),
            source_fingerprints: HashMap::new(),
            target_revisions: HashMap::new(),
            revision: 0,
        }
    }
}

impl<VM> PortalRegistry<VM> {
    pub(crate) fn publish_source(
        &mut self,
        source_window_key: &str,
        requests: Vec<ExternalPortalRequest<VM>>,
    ) -> Vec<String> {
        let next_fingerprints = requests
            .iter()
            .map(ExternalPortalRequest::fingerprint)
            .collect::<Vec<_>>();
        let previous_fingerprints = self
            .source_fingerprints
            .get(source_window_key)
            .cloned()
            .unwrap_or_default();
        if previous_fingerprints == next_fingerprints {
            return Vec::new();
        }

        let previous_targets = self.targets_for_source(source_window_key);
        let next_targets = requests
            .iter()
            .map(|request| request.target_window_key.clone())
            .collect::<BTreeSet<_>>();
        self.by_source
            .insert(source_window_key.to_string(), requests);
        self.source_fingerprints
            .insert(source_window_key.to_string(), next_fingerprints);
        self.bump_targets(previous_targets.union(&next_targets).cloned().collect())
    }

    pub(crate) fn remove_source(&mut self, source_window_key: &str) -> Vec<String> {
        let previous_targets = self.targets_for_source(source_window_key);
        self.by_source.remove(source_window_key);
        self.source_fingerprints.remove(source_window_key);
        if previous_targets.is_empty() {
            return Vec::new();
        }
        self.bump_targets(previous_targets.into_iter().collect())
    }

    #[cfg(test)]
    pub(crate) fn has_source_registration(&self, source_window_key: &str) -> bool {
        self.by_source.contains_key(source_window_key)
            || self.source_fingerprints.contains_key(source_window_key)
    }

    pub(crate) fn requests_for_target(
        &self,
        target_window_key: &str,
    ) -> Vec<ExternalPortalRequest<VM>> {
        self.by_source
            .values()
            .flat_map(|requests| requests.iter())
            .filter(|request| request.target_window_key == target_window_key)
            .cloned()
            .collect()
    }

    pub(crate) fn target_revision(&self, target_window_key: &str) -> u64 {
        self.target_revisions
            .get(target_window_key)
            .copied()
            .unwrap_or(0)
    }

    fn targets_for_source(&self, source_window_key: &str) -> BTreeSet<String> {
        self.by_source
            .get(source_window_key)
            .map(|requests| {
                requests
                    .iter()
                    .map(|request| request.target_window_key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn bump_targets(&mut self, targets: Vec<String>) -> Vec<String> {
        if targets.is_empty() {
            return Vec::new();
        }
        self.revision = self.revision.wrapping_add(1);
        for target in &targets {
            self.target_revisions.insert(target.clone(), self.revision);
        }
        targets
    }
}
