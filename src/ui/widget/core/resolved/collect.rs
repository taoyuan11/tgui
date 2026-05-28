use super::resolved_freeze::lifecycle_snapshot;
use super::*;
use crate::ui::widget::FocusScopeState;

mod chrome;
mod controls;
mod layout_media;
mod menu;
mod modal;
mod popover;
mod toast;
mod tooltip;

struct CollectResolvedStyles {
    button_style: Option<ResolvedButtonStyle>,
    select_style: Option<ResolvedSelectStyle>,
    slider_style: Option<ResolvedSliderStyle>,
    progress_bar_style: Option<crate::ui::widget::style::ProgressBarStyle>,
    spinner_style: Option<crate::ui::widget::style::SpinnerStyle>,
    input_style: Option<ResolvedInputStyle>,
    checkbox_style: Option<ResolvedCheckboxStyle>,
    radio_style: Option<ResolvedRadioStyle>,
}

struct CollectVisualState {
    frame: Rect,
    background_frame: Rect,
    background_radius: Dp,
    primitive_clip: Option<Rect>,
    primitive_clip_mask: Option<ClipMask>,
    disabled: bool,
    widget_state: WidgetState,
    opacity: f32,
    border_width: Dp,
    border_radius: Dp,
    border_color: Color,
    background: Color,
    styles: CollectResolvedStyles,
}

struct CollectCaches<'a, VM> {
    lifecycle_states: &'a mut HashMap<WidgetId, LifecycleEventState<VM>>,
    chunks: &'a mut HashMap<WidgetId, ComputedScene<VM>>,
    chunk_parts: &'a mut HashMap<WidgetId, SceneChunkParts<VM>>,
    visual_contexts: &'a mut HashMap<WidgetId, VisualContextSnapshot>,
}

impl<VM: 'static> ResolvedElement<VM> {
    pub(in super::super) fn collect_subtree_cache(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> ComputedScene<VM> {
        super::super::tree::with_widget_stack_frame(|| {
            let owner = self.id.dependency_owner(DependencyPhase::Scene);
            track_dependency_scope(owner, || {
                self.collect_subtree_cache_tracked(
                    layout_node,
                    visual_context,
                    context,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        })
    }

    fn collect_subtree_cache_tracked(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> ComputedScene<VM> {
        let previous_scope_path = context.focus.scope_path.clone();
        if self.focus.scope.is_some() {
            let mut path = previous_scope_path.clone();
            path.push(self.id);
            context.focus.scope_path = path;
        }
        let mut caches = CollectCaches {
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
        };
        self.collect_runtime_lifecycle_state(caches.lifecycle_states);

        let mut computed = ComputedScene::default();
        if let Some(scope) = self.focus.scope {
            computed.register_focus_scope(FocusScopeState {
                scope_id: self.id,
                path: context.focus.scope_path.clone(),
                options: scope,
            });
        }
        let visual = self.resolve_collect_visual_state(layout_node, visual_context, context);
        self.push_surface_primitives_and_base_hit_regions(&mut computed, context, &visual);

        if !self.collect_layout_media_kind(
            layout_node,
            visual_context,
            context,
            &mut caches,
            &mut computed,
            &visual,
        ) {
            let handled = self.collect_control_kind(context, &mut computed, &visual);
            debug_assert!(
                handled,
                "unhandled widget kind in collect_subtree_cache_tracked"
            );
        }

        self.emit_tooltip_if_visible(context, &mut computed, &visual);
        self.emit_popover_overlay_if_visible(context, &mut computed, &visual);
        self.emit_menu_overlay_if_open(context, &mut computed, &visual);
        self.emit_modal_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_toast_overlay_if_visible(context, &mut computed, &visual);

        caches
            .chunk_parts
            .entry(self.id)
            .or_insert_with(|| SceneChunkParts {
                before_children: computed.clone(),
                after_children: ComputedScene::default(),
            });
        caches
            .visual_contexts
            .insert(self.id, visual_context.into());
        caches.chunks.insert(self.id, computed.clone());
        context.focus.scope_path = previous_scope_path;
        computed
    }

    fn collect_runtime_lifecycle_state(
        &self,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
    ) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            lifecycle_states.insert(
                self.id,
                LifecycleEventState {
                    widget_id: self.id,
                    snapshot: lifecycle_snapshot(self),
                    handlers: self.lifecycle_events.clone(),
                },
            );
        }
    }
}
