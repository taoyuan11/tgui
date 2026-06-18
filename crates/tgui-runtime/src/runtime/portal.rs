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
    pub(crate) source_widget_id: WidgetId,
    pub(crate) overlay_id: OverlayId,
    pub(crate) target_window_key: String,
    pub(crate) anchor: PortalAnchor,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) content: Box<Element<VM>>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) focus_scope: Option<FocusScopeOptions>,
}

impl<VM> Clone for ExternalPortalRequest<VM> {
    fn clone(&self) -> Self {
        Self {
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
        self.external_portal_requests = requests;
        if self.external_portal_revision != revision {
            self.external_portal_revision = revision;
            self.invalidate_computed_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }

    pub(in crate::runtime) fn external_portal_requests_from_computed(
        &mut self,
    ) -> Vec<ExternalPortalRequest<VM>> {
        self.computed_scene()
            .external_portal_requests
            .iter()
            .cloned()
            .collect()
    }

    pub(in crate::runtime) fn append_external_portals_to_computed(
        &mut self,
        computed: &mut ComputedScene<VM>,
        now: Instant,
    ) {
        if self.external_portal_requests.is_empty() {
            return;
        }

        let requests = self.external_portal_requests.clone();
        let viewport = self.viewport_rect();
        let units = self.unit_context();
        let theme = self.animated_theme(now);
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
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
            widget_states: &widget_states,
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
            focus: FocusCollectState::default(),
            tooltip_hover_started_at: &self.tooltip_hover_started_at,
            next_tooltip_wakeup: &next_tooltip_wakeup,
            next_toast_wakeup: &next_toast_wakeup,
            active_tooltip: None,
            active_hover_popover: None,
            gpu_scroll_enabled: false,
            gpu_scroll_container: None,
            transform_stack: smallvec::SmallVec::new(),
        };

        for request in requests {
            let Some(anchor) = resolve_external_portal_anchor(&request, viewport, computed) else {
                continue;
            };
            let Some((content_scene, content_size)) =
                collect_portal_content_scene(request.content.as_ref(), &mut context)
            else {
                continue;
            };
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
            "{:?}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}|{}",
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
        if previous_targets.is_empty() {
            return Vec::new();
        }
        self.by_source.remove(source_window_key);
        self.source_fingerprints.remove(source_window_key);
        self.bump_targets(previous_targets.into_iter().collect())
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
