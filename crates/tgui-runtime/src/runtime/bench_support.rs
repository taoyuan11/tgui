use super::scene_runtime::{frame_path_probe, scroll_fast_path_probe};
use super::{BoundRuntimeHandler, WindowBindings};
use crate::animation::AnimationCoordinator;
use crate::application::{ApplicationConfig, MsaaMode, ResourceBudget, ThemeSelection, WindowRole};
use crate::dialog::async_dialog_channel;
use crate::foundation::binding::{
    InvalidationSignal, State, Toast, ToastId, ToastPlacement, ToastQueue, ViewModelContext,
};
use crate::foundation::color::Color;
use crate::foundation::task::async_task_channel;
use crate::foundation::view_model::{Command, CommandEffect, ValueCommand, ViewModel};
use crate::notification::async_notification_channel;
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use crate::platform::backend::window::Window;
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::error::RequestError;
use crate::platform::event::{
    ButtonSource, ElementState, KeyEvent, MouseButton, PointerSource, WindowEvent,
};
use crate::platform::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
use crate::platform::window::WindowAttributes;
use crate::rendering::renderer::HeadlessBenchRenderer;
use crate::text::font::FontCatalog;
use crate::ui::layout::{Axis, Insets, Overflow};
use crate::ui::theme::{ThemeMode, ThemeSet};
use crate::ui::unit::{dp, Dp};
use crate::ui::widget::{
    Button, DataGrid, DataGridColumn, DataGridColumnPin, DataGridRow, DataGridSelectionChange,
    DataGridSelectionMode, Element, Flex, HitInteraction, ItemLayout, List, ListItem,
    ListSelectionChange, ListSelectionMode, Point, Rect, Slider, SliderOrientation, Stack, Text,
    TextPrimitive, ToastHost, Tree, TreeNode, TreeSelectionChange, TreeSelectionMode, WidgetId,
    WidgetKey, WidgetTree,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::monitor::MonitorHandle;

#[derive(Default)]
pub struct RuntimeScrollBenchmarkVm;

impl ViewModel for RuntimeScrollBenchmarkVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeScrollFrameStats {
    pub gpu_fast_path_hits: usize,
    pub subtree_patch_hits: usize,
    pub full_recollects: usize,
    pub scene_update: Duration,
    pub renderer_liveness: Duration,
    pub renderer_prepare_upload: Duration,
    pub renderer_encode: Duration,
    pub queue_submit: Duration,
    pub gpu_wait: Duration,
}

pub struct RuntimeScrollBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeScrollBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    scroll_id: WidgetId,
    pub adapter_name: String,
    pub backend: String,
    pub gpu_scroll_supported: bool,
}

impl RuntimeScrollBenchmarkContext {
    pub fn new(tree: WidgetTree<RuntimeScrollBenchmarkVm>, viewport: Rect) -> Result<Self, String> {
        let width = viewport.width.get().max(1.0).ceil() as u32;
        let height = viewport.height.get().max(1.0).ceil() as u32;
        let renderer = HeadlessBenchRenderer::new(PhysicalSize::new(width, height))
            .map_err(|error| error.to_string())?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let gpu_scroll_supported = renderer.push_constants_supported();
        let invalidation = InvalidationSignal::new();
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        let (notification_dispatcher, notification_receiver) = async_notification_channel();
        let (task_dispatcher, task_receiver) = async_task_channel();
        let config = ApplicationConfig {
            app_id: None,
            title: "runtime-scroll-benchmark".to_string(),
            size: LogicalSize::new(f64::from(width), f64::from(height)),
            min_size: None,
            max_size: None,
            clear_color: Color::WHITE,
            clear_color_overridden: true,
            close_children_with_main: true,
            decorations: true,
            viewport_insets: Insets::ZERO,
            msaa: MsaaMode::Off,
            fonts: FontCatalog::default(),
            theme: ThemeSelection::Mode(ThemeMode::Light),
            theme_set: ThemeSet::default(),
            style_sheet: crate::ui::widget::StyleSheet::default(),
            reduced_motion: false,
            window_icon: None,
            resource_budget: ResourceBudget::DEFAULT,
        };
        let mut handler = BoundRuntimeHandler::new(
            "runtime-scroll-benchmark".to_string(),
            1,
            WindowRole::Main,
            config,
            Arc::new(Mutex::new(RuntimeScrollBenchmarkVm)),
            WindowBindings::default(),
            Some(tree),
            None,
            Vec::new(),
            invalidation,
            AnimationCoordinator::default(),
            dialog_dispatcher,
            Some(dialog_receiver),
            notification_dispatcher,
            Some(notification_receiver),
            task_dispatcher,
            Some(task_receiver),
        );
        handler.gpu_scroll_supported = gpu_scroll_supported;
        let scroll_id = handler
            .computed_scene()
            .scroll_regions
            .iter()
            .find(|region| region.can_scroll_x() || region.can_scroll_y())
            .map(|region| region.id)
            .ok_or_else(|| "runtime benchmark tree has no scrollable region".to_string())?;
        let mut context = Self {
            handler,
            renderer,
            scroll_id,
            adapter_name,
            backend,
            gpu_scroll_supported,
        };
        context.render_current_scene()?;
        Ok(context)
    }

    pub fn render_scroll_frame(
        &mut self,
        offset: Point,
        force_full_recollect: bool,
    ) -> Result<RuntimeScrollFrameStats, String> {
        scroll_fast_path_probe::reset();
        self.handler.set_scroll_offset(self.scroll_id, offset);
        if force_full_recollect {
            self.handler.invalidate_computed_scene();
        }
        let profile = self.render_current_scene()?;
        let gpu_fast_path_hits = scroll_fast_path_probe::gpu_hits() as usize;
        let subtree_patch_hits = scroll_fast_path_probe::patch_hits() as usize;
        Ok(RuntimeScrollFrameStats {
            gpu_fast_path_hits,
            subtree_patch_hits,
            full_recollects: usize::from(gpu_fast_path_hits + subtree_patch_hits == 0),
            scene_update: profile.scene_update,
            renderer_liveness: profile.render.liveness,
            renderer_prepare_upload: profile.render.prepare_upload,
            renderer_encode: profile.render.encode,
            queue_submit: profile.render.submit,
            gpu_wait: profile.render.gpu_wait,
        })
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn render_current_scene(&mut self) -> Result<RuntimeScrollProfile, String> {
        let font_manager = Arc::clone(&self.handler.font_manager);
        let scene_started = Instant::now();
        let computed = self.handler.computed_scene_mut();
        let scene_update = scene_started.elapsed();
        self.renderer
            .render_and_wait(
                &mut computed.scene,
                font_manager.as_ref(),
                &computed.scroll_regions,
                &computed.transform_records,
            )
            .map_err(|error| error.to_string())?;
        Ok(RuntimeScrollProfile {
            scene_update,
            render: self.renderer.last_render_profile(),
        })
    }
}

struct RuntimeScrollProfile {
    scene_update: Duration,
    render: crate::rendering::renderer::BenchRenderProfile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeInteractionFrameStats {
    pub event_changed: bool,
    pub rendered: bool,
    pub command_dispatches_delta: usize,
    pub hover_epoch_delta: u64,
    pub invalidation_revision_delta: u64,
    pub cache_hits: usize,
    pub scene_recollects: usize,
    pub layout_reuses: usize,
    pub layout_builds: usize,
    pub retained_scene_patches: usize,
    pub reactive_property_slot_writes: usize,
    pub layout_patch_actions: usize,
    pub full_rebuild_actions: usize,
    pub event_handling: Duration,
    pub state_mutation: Duration,
    pub sync_bindings: Duration,
    pub scene_update: Duration,
    pub toast_measure: Duration,
    pub toast_collect: Duration,
    pub toast_compose: Duration,
    pub toast_measured_cards: usize,
    pub toast_collected_cards: usize,
    pub toast_layout_passes: usize,
    pub toast_base_scene_replay_hits: usize,
    pub toast_base_scene_replay_fallbacks: usize,
    pub renderer_liveness: Duration,
    pub renderer_prepare_upload: Duration,
    pub renderer_encode: Duration,
    pub queue_submit: Duration,
    pub gpu_wait: Duration,
    pub total: Duration,
}

impl RuntimeInteractionFrameStats {
    pub fn is_scene_only_recollect(self) -> bool {
        self.scene_recollects > 0 && self.layout_reuses > 0 && self.layout_builds == 0
    }

    pub fn is_retained_scene_update(self) -> bool {
        self.layout_builds == 0
            && self.layout_patch_actions == 0
            && self.full_rebuild_actions == 0
            && (self.retained_scene_patches > 0
                || self.reactive_property_slot_writes > 0
                || self.is_scene_only_recollect())
    }

    pub fn scene_and_layout_total(self) -> Duration {
        self.sync_bindings + self.scene_update
    }

    pub fn renderer_total(self) -> Duration {
        self.renderer_liveness
            + self.renderer_prepare_upload
            + self.renderer_encode
            + self.queue_submit
            + self.gpu_wait
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeDataGridHoverTarget {
    FirstRowStartPinned,
    FirstRowUnpinned,
    FirstRowEndPinned,
    SecondRowStartPinned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeRowHoverKind {
    List,
    Tree,
}

pub struct RuntimeButtonHoverBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    button_ids: [WidgetId; 2],
    hover_points: [Point; 2],
    pub adapter_name: String,
    pub backend: String,
}

impl RuntimeButtonHoverBenchmarkContext {
    pub fn new(buttons: usize, viewport: Rect) -> Result<Self, String> {
        Self::new_with_reduced_motion(buttons, viewport, false)
    }

    pub fn new_with_reduced_motion(
        buttons: usize,
        viewport: Rect,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        if buttons < 2 {
            return Err("button-hover benchmark requires at least two buttons".to_string());
        }
        let (tree, button_ids) = button_hover_benchmark_tree(buttons, viewport);
        let (mut handler, renderer) = interaction_handler(tree, viewport, reduced_motion)?;
        let hover_points = [
            button_hover_point(&mut handler, button_ids[0])?,
            button_hover_point(&mut handler, button_ids[1])?,
        ];
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let mut context = Self {
            handler,
            renderer,
            button_ids: [button_ids[0], button_ids[1]],
            hover_points,
            adapter_name,
            backend,
        };
        let _ = context.render_scene()?;
        Ok(context)
    }

    pub fn move_hover(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(second_button, true)
    }

    pub fn move_hover_scene_recollect_control(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(second_button, false)
    }

    fn move_hover_with_mode(
        &mut self,
        second_button: bool,
        allow_retained_hover_patch: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let point = self.hover_points[usize::from(second_button)];
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        let had_hover_before = !self.handler.hovered_widgets.is_empty();
        let focused_before = self.handler.focused_widget_id();
        let pressed_before = self.handler.pressed_widget;
        begin_interaction_probe();
        let total_started = Instant::now();
        let event_started = Instant::now();
        let _ = self
            .handler
            .handle_bound_window_event(&RuntimeBenchEventLoop, pointer_moved_event(point));
        let event_handling = event_started.elapsed();
        let event_changed = hover_epoch_before != self.handler.hover_epoch
            || revision_before != self.handler.invalidation.revision()
            || focused_before != self.handler.focused_widget_id()
            || pressed_before != self.handler.pressed_widget;
        if allow_retained_hover_patch
            && event_changed
            && had_hover_before
            && self.handler.button_hover_patch_pending.is_none()
        {
            let cached = self.handler.cached_scene.as_ref();
            let hovered_button = self.handler.hovered_widgets.iter().find_map(|hovered| {
                if let super::HoverTargetId::Widget(id) = hovered.target_id {
                    Some(id)
                } else {
                    None
                }
            });
            let layout = cached.and_then(|cached| cached.layout.as_ref());
            return Err(format!(
                "button-hover event produced no retained candidate: invalidation_match={} root_rebuild_match={} runtime_idle={} computed_valid={} layout_valid={} gpu_scroll_deferred={} structure_eligible={} lifecycle_states={} media_bindings={} media_events={} contains_virtual={} passive_path={} hovered_widgets={} hovered_button={hovered_button:?} simple_root={} chunk_eligible={} has_chunk_parts={} has_visual_context={}",
                self.handler.invalidation.revision() == self.handler.last_invalidation_revision,
                self.handler.invalidation.root_rebuild_revision()
                    == self.handler.last_root_rebuild_revision,
                self.handler.button_hover_runtime_is_idle(),
                cached.is_some_and(|cached| cached.computed_valid),
                cached.is_some_and(|cached| cached.layout_valid),
                cached.is_some_and(|cached| cached.gpu_scroll_deferred),
                cached.is_some_and(|cached| cached
                    .computed
                    .is_simple_for_button_hover_recompose()),
                cached.map_or(0, |cached| cached.lifecycle_states.len()),
                cached.map_or(0, |cached| cached.media_texture_bindings.len()),
                self.handler.media_event_states.len(),
                cached
                    .and_then(|cached| cached.layout.as_ref())
                    .is_some_and(|layout| layout.contains_virtual()),
                BoundRuntimeHandler::<RuntimeInteractionBenchmarkVm>::button_hover_path_is_passive(
                    &self.handler.hovered_widgets,
                ),
                self.handler.hovered_widgets.len(),
                hovered_button.is_some_and(|id| layout.is_some_and(|layout|
                    BoundRuntimeHandler::<RuntimeInteractionBenchmarkVm>::is_simple_button_hover_root(layout, id)
                )),
                hovered_button.is_some_and(|id| cached
                    .and_then(|cached| cached.scene_chunks.get(&id))
                    .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose())),
                hovered_button.is_some_and(|id| cached
                    .is_some_and(|cached| cached.scene_chunk_parts.contains_key(&id))),
                hovered_button.is_some_and(|id| cached
                    .is_some_and(|cached| cached.visual_contexts.contains_key(&id))),
            ));
        }
        if !allow_retained_hover_patch {
            self.handler.button_hover_patch_pending = None;
        }
        let profile = event_changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        let stats = interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        );
        if allow_retained_hover_patch
            && event_changed
            && had_hover_before
            && stats.retained_scene_patches == 0
        {
            return Err(format!(
                "button-hover retained candidate fell back: actions={actions:?} stats={stats:?}"
            ));
        }
        Ok(stats)
    }

    pub fn prime_focus(&mut self, second_button: bool) -> Result<(), String> {
        let button_id = self.button_ids[usize::from(second_button)];
        let _ = self.move_hover_scene_recollect_control(second_button)?;
        let down = self.measure_button_event(ElementState::Pressed, second_button, false)?;
        if down.layout_builds != 0 {
            return Err("button focus prime unexpectedly rebuilt layout".to_string());
        }
        let up = self.measure_button_event(ElementState::Released, second_button, false)?;
        if up.layout_builds != 0 {
            return Err("button focus release unexpectedly rebuilt layout".to_string());
        }
        if self.handler.focused_widget_id() != Some(button_id)
            || self.handler.focus_visible
            || self.handler.pressed_widget.is_some()
        {
            return Err(format!(
                "button focus prime did not settle: focused={:?} focus_visible={} pressed={:?}",
                self.handler.focused_widget_id(),
                self.handler.focus_visible,
                self.handler.pressed_widget,
            ));
        }
        Ok(())
    }

    pub fn pointer_down(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.measure_button_event(ElementState::Pressed, second_button, true)
    }

    pub fn pointer_down_scene_recollect_control(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.measure_button_event(ElementState::Pressed, second_button, false)
    }

    pub fn pointer_up(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.measure_button_event(ElementState::Released, second_button, true)
    }

    pub fn pointer_up_scene_recollect_control(
        &mut self,
        second_button: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.measure_button_event(ElementState::Released, second_button, false)
    }

    fn measure_button_event(
        &mut self,
        state: ElementState,
        second_button: bool,
        allow_retained_pressed_patch: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let index = usize::from(second_button);
        let point = self.hover_points[index];
        let button_id = self.button_ids[index];
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        let focused_before = self.handler.focused_widget_id();
        let pressed_before = self.handler.pressed_widget;
        let dispatches_before = self
            .handler
            .with_view_model(|view_model| view_model.focus_activation_dispatches);
        if !allow_retained_pressed_patch && state == ElementState::Released {
            // Release normally consumes its retained candidate inside `handle_hover`, before
            // this benchmark regains control. Simulate a release carrying a fresh pointer
            // position so the production safety guard selects the full scene-recollect control.
            self.handler.cursor_position = None;
        }
        begin_interaction_probe();
        let total_started = Instant::now();
        let event_started = Instant::now();
        let _ = self
            .handler
            .handle_bound_window_event(&RuntimeBenchEventLoop, pointer_button_event(point, state));
        let event_handling = event_started.elapsed();
        let dispatches_after = self
            .handler
            .with_view_model(|view_model| view_model.focus_activation_dispatches);
        let event_changed = hover_epoch_before != self.handler.hover_epoch
            || revision_before != self.handler.invalidation.revision()
            || focused_before != self.handler.focused_widget_id()
            || pressed_before != self.handler.pressed_widget;
        if allow_retained_pressed_patch
            && focused_before == Some(button_id)
            && self.handler.button_pressed_patch_pending.is_none()
            && self
                .handler
                .cached_scene
                .as_ref()
                .is_none_or(|cached| cached.pressed_widget != self.handler.pressed_widget)
        {
            let cached = self.handler.cached_scene.as_ref();
            let layout = cached.and_then(|cached| cached.layout.as_ref());
            return Err(format!(
                "button {state:?} produced no pressed candidate: invalidation_match={} root_rebuild_match={} runtime_idle={} computed_valid={} layout_valid={} gpu_scroll_deferred={} cached_pressed={:?} source_pressed={pressed_before:?} next_pressed={:?} cached_hover_epoch={:?} hover_epoch={} focus_match={} structure_eligible={} lifecycle_states={} media_bindings={} media_events={} external_portals={} contains_virtual={} hovered_button={:?} simple_root={} chunk_eligible={} has_visual_context={}",
                self.handler.invalidation.revision() == self.handler.last_invalidation_revision,
                self.handler.invalidation.root_rebuild_revision()
                    == self.handler.last_root_rebuild_revision,
                self.handler.button_visual_runtime_is_idle_ignoring_pressed(),
                cached.is_some_and(|cached| cached.computed_valid),
                cached.is_some_and(|cached| cached.layout_valid),
                cached.is_some_and(|cached| cached.gpu_scroll_deferred),
                cached.and_then(|cached| cached.pressed_widget),
                self.handler.pressed_widget,
                cached.map(|cached| cached.hover_epoch),
                self.handler.hover_epoch,
                cached.is_some_and(|cached| cached.focused_widget == self.handler.focused_widget_id()
                    && cached.focus_visible == self.handler.focus_visible),
                cached.is_some_and(|cached| cached
                    .computed
                    .is_simple_for_button_hover_recompose()),
                cached.map_or(0, |cached| cached.lifecycle_states.len()),
                cached.map_or(0, |cached| cached.media_texture_bindings.len()),
                self.handler.media_event_states.len(),
                self.handler.external_portal_requests.len(),
                layout.is_some_and(|layout| layout.contains_virtual()),
                layout.and_then(|layout| self.handler.hovered_simple_button(layout)),
                layout.is_some_and(|layout|
                    BoundRuntimeHandler::<RuntimeInteractionBenchmarkVm>::is_simple_button_pressed_root(layout, button_id)
                ),
                cached
                    .and_then(|cached| cached.scene_chunks.get(&button_id))
                    .is_some_and(|chunk| chunk.is_simple_for_button_hover_recompose()),
                cached.is_some_and(|cached| cached.visual_contexts.contains_key(&button_id)),
            ));
        }
        if !allow_retained_pressed_patch {
            self.handler.button_pressed_patch_pending = None;
        }
        let profile = event_changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        let mut stats = interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        );
        stats.command_dispatches_delta = dispatches_after.saturating_sub(dispatches_before);
        let expected_pressed = match state {
            ElementState::Pressed => Some(button_id),
            ElementState::Released => None,
        };
        if self.handler.focused_widget_id() != Some(button_id)
            || self.handler.pressed_widget != expected_pressed
        {
            return Err(format!(
                "button {state:?} ended in the wrong state: focused={:?} pressed={:?} expected_button={button_id:?}",
                self.handler.focused_widget_id(),
                self.handler.pressed_widget,
            ));
        }
        if allow_retained_pressed_patch
            && focused_before == Some(button_id)
            && stats.retained_scene_patches == 0
        {
            return Err(format!(
                "button {state:?} retained candidate fell back: actions={actions:?} stats={stats:?}"
            ));
        }
        Ok(stats)
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn render_scene(&mut self) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, true)
    }
}

pub struct RuntimeRowHoverBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    hover_points: [Point; 2],
    viewport: Rect,
    pub kind: RuntimeRowHoverKind,
    pub adapter_name: String,
    pub backend: String,
}

impl RuntimeRowHoverBenchmarkContext {
    pub fn new(kind: RuntimeRowHoverKind, rows: usize, viewport: Rect) -> Result<Self, String> {
        Self::new_with_reduced_motion(kind, rows, viewport, false)
    }

    pub fn new_with_reduced_motion(
        kind: RuntimeRowHoverKind,
        rows: usize,
        viewport: Rect,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        let tree = row_hover_benchmark_tree(kind, rows, viewport);
        let (mut handler, renderer) = interaction_handler(tree, viewport, reduced_motion)?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let first = row_hover_point(&mut handler, kind, 0)?;
        let second = row_hover_point(&mut handler, kind, 1)?;
        if first.0 == second.0 {
            return Err(format!(
                "{kind:?} adjacent rows unexpectedly share one widget id"
            ));
        }
        let expected_kind = match kind {
            RuntimeRowHoverKind::List => crate::ui::widget::RetainedHoverRowKind::List,
            RuntimeRowHoverKind::Tree => crate::ui::widget::RetainedHoverRowKind::Tree,
        };
        let eligibility = handler
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(first.0))
            .and_then(|row| row.retained_hover_row_kind());
        if eligibility != Some(expected_kind) {
            return Err(format!(
                "{kind:?} benchmark row is not retained-hover eligible: {eligibility:?}"
            ));
        }
        let mut context = Self {
            handler,
            renderer,
            hover_points: [first.1, second.1],
            viewport,
            kind,
            adapter_name,
            backend,
        };
        let _ = context.render_scene()?;
        Ok(context)
    }

    pub fn move_hover(&mut self, second_row: bool) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(second_row, true)
    }

    pub fn move_hover_scene_recollect_control(
        &mut self,
        second_row: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(second_row, false)
    }

    fn move_hover_with_mode(
        &mut self,
        second_row: bool,
        allow_retained_hover_patch: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let point = self.hover_points[usize::from(second_row)];
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        begin_interaction_probe();
        let total_started = Instant::now();
        self.handler.cursor_position = Some(point);
        let event_started = Instant::now();
        let event_changed = self.handler.handle_hover(self.viewport);
        let event_handling = event_started.elapsed();
        if !allow_retained_hover_patch {
            self.handler.row_hover_patch_pending = None;
        }
        let profile = event_changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        Ok(interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        ))
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn render_scene(&mut self) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeRowSelectionKind {
    List,
    Tree,
    DataGrid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeRowSelectionMode {
    None,
    Single,
    Multiple,
}

pub struct RuntimeRowSelectionBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    row_ids: [WidgetId; 2],
    row_points: [Point; 2],
    viewport: Rect,
    pub kind: RuntimeRowSelectionKind,
    pub selection_enabled: bool,
    pub adapter_name: String,
    pub backend: String,
}

pub struct RuntimeTreeCheckedBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
}

pub struct RuntimeSliderValueBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    slider_count: usize,
}

pub struct RuntimeTextContentBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    text_count: usize,
}

#[derive(Debug, PartialEq)]
struct RuntimeTextContentSceneFingerprint {
    texts: Vec<TextPrimitive>,
}

impl RuntimeTextContentBenchmarkContext {
    pub fn new(text_count: usize, viewport: Rect) -> Result<Self, String> {
        if text_count == 0 {
            return Err("text-content benchmark requires at least one Text".to_string());
        }

        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        let tree =
            text_content_benchmark_tree(text_count, viewport, view_model.text_content.clone());
        let mut handler = interaction_runtime_handler_with_vm(
            view_model,
            tree,
            viewport,
            true,
            invalidation,
            animations,
        );
        let actual_text_count = handler.computed_scene().scene.texts.len();
        if actual_text_count != text_count {
            return Err(format!(
                "text-content benchmark expected {text_count} retained Text primitives, got {actual_text_count}"
            ));
        }
        Ok(Self {
            handler,
            text_count,
        })
    }

    pub fn set_content(&mut self, content: &str) -> RuntimeInteractionFrameStats {
        self.set_content_with_mode(content, false)
    }

    pub fn set_content_legacy_full_visual(
        &mut self,
        content: &str,
    ) -> RuntimeInteractionFrameStats {
        self.set_content_with_mode(content, true)
    }

    fn set_content_with_mode(
        &mut self,
        content: &str,
        legacy_full_visual: bool,
    ) -> RuntimeInteractionFrameStats {
        let revision_before = self.handler.invalidation.revision();
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler.with_view_model(|view_model| {
            view_model.text_content.set(content.to_string());
        });
        let state_mutation = mutation_started.elapsed();
        let (sync_bindings, scene_update) =
            crate::ui::widget::with_legacy_text_content_reactive_resolve(
                legacy_full_visual,
                || {
                    let sync_started = Instant::now();
                    self.handler.request_redraw_if_dirty(Instant::now());
                    let sync_bindings = sync_started.elapsed();
                    let scene_started = Instant::now();
                    let _ = self.handler.computed_scene();
                    (sync_bindings, scene_started.elapsed())
                },
            );
        let (paths, actions) = finish_interaction_probe();
        interaction_stats(
            true,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            Some(InteractionRenderProfile {
                sync_bindings,
                scene_update,
                render: Default::default(),
            }),
        )
    }

    pub fn text_count(&self) -> usize {
        self.text_count
    }

    pub fn content(&self) -> String {
        self.handler
            .with_view_model(|view_model| view_model.text_content.get())
    }

    pub fn assert_scene_equivalent(&mut self, other: &mut Self) -> Result<(), String> {
        let left = runtime_text_content_scene_fingerprint(&mut self.handler);
        let right = runtime_text_content_scene_fingerprint(&mut other.handler);
        if left != right {
            return Err(format!(
                "direct and legacy TextContent scene fingerprints differ:\ndirect={left:#?}\nlegacy={right:#?}"
            ));
        }
        Ok(())
    }

    pub fn assert_full_recollect_equivalent(&mut self) -> Result<(), String> {
        let retained = runtime_text_content_scene_fingerprint(&mut self.handler);
        self.handler.invalidate_computed_scene();
        let full = runtime_text_content_scene_fingerprint(&mut self.handler);
        if retained != full {
            return Err(format!(
                "retained TextContent scene fingerprint differs from full recollect:\nretained={retained:#?}\nfull={full:#?}"
            ));
        }
        Ok(())
    }
}

fn runtime_text_content_scene_fingerprint(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
) -> RuntimeTextContentSceneFingerprint {
    let texts = handler
        .computed_scene()
        .scene
        .texts
        .iter()
        .cloned()
        .collect();
    RuntimeTextContentSceneFingerprint { texts }
}

#[derive(Debug, PartialEq)]
struct RuntimeSliderSceneFingerprint {
    shapes: Vec<(Rect, Color, u32, u32, Option<Rect>)>,
    textures: Vec<(Rect, Option<Rect>, u32, u32, Option<Rect>)>,
    hits: Vec<(WidgetId, u32, u32, u32, u32, bool, Rect, Rect)>,
}

impl RuntimeSliderValueBenchmarkContext {
    pub fn new(slider_count: usize, viewport: Rect, with_shadow: bool) -> Result<Self, String> {
        if slider_count == 0 {
            return Err("slider-value benchmark requires at least one slider".to_string());
        }

        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        let tree = slider_value_benchmark_tree(
            slider_count,
            viewport,
            view_model.slider_value.clone(),
            with_shadow,
        );
        let mut handler = interaction_runtime_handler_with_vm(
            view_model,
            tree,
            viewport,
            true,
            invalidation,
            animations,
        );
        let _ = handler.computed_scene();
        Ok(Self {
            handler,
            slider_count,
        })
    }

    pub fn set_value(&mut self, value: f32) -> RuntimeInteractionFrameStats {
        self.set_value_with_mode(value, false)
    }

    pub fn set_value_full_recollect_control(&mut self, value: f32) -> RuntimeInteractionFrameStats {
        self.set_value_with_mode(value, true)
    }

    fn set_value_with_mode(
        &mut self,
        value: f32,
        force_full_recollect: bool,
    ) -> RuntimeInteractionFrameStats {
        let revision_before = self.handler.invalidation.revision();
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler
            .with_view_model(|view_model| view_model.slider_value.set(value));
        let state_mutation = mutation_started.elapsed();
        let sync_started = Instant::now();
        self.handler.request_redraw_if_dirty(Instant::now());
        let sync_bindings = sync_started.elapsed();
        if force_full_recollect {
            self.handler.invalidate_computed_scene();
        }
        let scene_started = Instant::now();
        let _ = self.handler.computed_scene();
        let scene_update = scene_started.elapsed();
        let (paths, actions) = finish_interaction_probe();
        interaction_stats(
            true,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            Some(InteractionRenderProfile {
                sync_bindings,
                scene_update,
                render: Default::default(),
            }),
        )
    }

    pub fn value(&self) -> f32 {
        self.handler
            .with_view_model(|view_model| view_model.slider_value.get())
    }

    pub fn slider_count(&self) -> usize {
        self.slider_count
    }

    pub fn texture_count(&mut self) -> usize {
        self.handler.computed_scene().scene.textures.len()
    }

    pub fn assert_full_recollect_equivalent(&mut self) -> Result<(), String> {
        let retained = runtime_slider_scene_fingerprint(&mut self.handler);
        self.handler.invalidate_computed_scene();
        let full = runtime_slider_scene_fingerprint(&mut self.handler);
        if retained != full {
            return Err(format!(
                "retained Slider scene/hit fingerprint differs from full recollect:\nretained={retained:#?}\nfull={full:#?}"
            ));
        }
        Ok(())
    }
}

fn runtime_slider_scene_fingerprint(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
) -> RuntimeSliderSceneFingerprint {
    let computed = handler.computed_scene();
    let shapes = computed
        .scene
        .shapes
        .iter()
        .map(|shape| {
            (
                shape.rect,
                shape.color,
                shape.corner_radius.to_bits(),
                shape.stroke_width.to_bits(),
                shape.clip_rect,
            )
        })
        .collect();
    let textures = computed
        .scene
        .textures
        .iter()
        .map(|texture| {
            (
                texture.frame,
                texture.uv_rect,
                texture.corner_radius.to_bits(),
                texture.opacity.to_bits(),
                texture.clip_rect,
            )
        })
        .collect();
    let hits = computed
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::Slider {
                id,
                value,
                min,
                max,
                step,
                orientation,
                track_rect,
                thumb_rect,
                ..
            } => Some((
                *id,
                value.to_bits(),
                min.to_bits(),
                max.to_bits(),
                step.to_bits(),
                matches!(orientation, SliderOrientation::Horizontal),
                *track_rect,
                *thumb_rect,
            )),
            _ => None,
        })
        .collect();
    RuntimeSliderSceneFingerprint {
        shapes,
        textures,
        hits,
    }
}

impl RuntimeTreeCheckedBenchmarkContext {
    pub fn new(
        rows: usize,
        viewport: Rect,
        initial_checked_keys: Vec<WidgetKey>,
    ) -> Result<Self, String> {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        view_model.checked_keys.set(initial_checked_keys);
        let tree = row_selection_benchmark_tree(
            RuntimeRowSelectionKind::Tree,
            rows,
            viewport,
            view_model.selected_keys.clone(),
            RuntimeRowSelectionMode::None,
            view_model.checked_keys.clone(),
            true,
        );
        let mut handler = interaction_runtime_handler_with_vm(
            view_model,
            tree,
            viewport,
            true,
            invalidation,
            animations,
        );
        let _ = handler.computed_scene();
        Ok(Self { handler })
    }

    pub fn set_checked_keys(
        &mut self,
        checked_keys: Vec<WidgetKey>,
    ) -> RuntimeInteractionFrameStats {
        let revision_before = self.handler.invalidation.revision();
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler
            .with_view_model(|view_model| view_model.checked_keys.set(checked_keys));
        let state_mutation = mutation_started.elapsed();
        let sync_started = Instant::now();
        self.handler.request_redraw_if_dirty(Instant::now());
        let sync_bindings = sync_started.elapsed();
        let scene_started = Instant::now();
        let _ = self.handler.computed_scene();
        let scene_update = scene_started.elapsed();
        let (paths, actions) = finish_interaction_probe();
        interaction_stats(
            true,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            Some(InteractionRenderProfile {
                sync_bindings,
                scene_update,
                render: Default::default(),
            }),
        )
    }

    pub fn checked_keys(&self) -> Vec<WidgetKey> {
        self.handler
            .with_view_model(|view_model| view_model.checked_keys.get())
    }
}

impl RuntimeRowSelectionBenchmarkContext {
    pub fn new(
        kind: RuntimeRowSelectionKind,
        rows: usize,
        viewport: Rect,
        selection_enabled: bool,
    ) -> Result<Self, String> {
        Self::new_with_initial_selection(
            kind,
            rows,
            viewport,
            if selection_enabled {
                RuntimeRowSelectionMode::Single
            } else {
                RuntimeRowSelectionMode::None
            },
            Vec::new(),
            false,
        )
    }

    pub fn new_with_reduced_motion(
        kind: RuntimeRowSelectionKind,
        rows: usize,
        viewport: Rect,
        selection_enabled: bool,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        Self::new_with_initial_selection(
            kind,
            rows,
            viewport,
            if selection_enabled {
                RuntimeRowSelectionMode::Single
            } else {
                RuntimeRowSelectionMode::None
            },
            Vec::new(),
            reduced_motion,
        )
    }

    pub fn new_with_initial_selection(
        kind: RuntimeRowSelectionKind,
        rows: usize,
        viewport: Rect,
        selection_mode: RuntimeRowSelectionMode,
        initial_selected_keys: Vec<WidgetKey>,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        Self::new_with_initial_tree_state(
            kind,
            rows,
            viewport,
            selection_mode,
            initial_selected_keys,
            Vec::new(),
            false,
            reduced_motion,
        )
    }

    pub fn new_tree_with_initial_checked(
        rows: usize,
        viewport: Rect,
        initial_checked_keys: Vec<WidgetKey>,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        Self::new_with_initial_tree_state(
            RuntimeRowSelectionKind::Tree,
            rows,
            viewport,
            RuntimeRowSelectionMode::None,
            Vec::new(),
            initial_checked_keys,
            true,
            reduced_motion,
        )
    }

    fn new_with_initial_tree_state(
        kind: RuntimeRowSelectionKind,
        rows: usize,
        viewport: Rect,
        selection_mode: RuntimeRowSelectionMode,
        initial_selected_keys: Vec<WidgetKey>,
        initial_checked_keys: Vec<WidgetKey>,
        tree_checkable: bool,
        reduced_motion: bool,
    ) -> Result<Self, String> {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        view_model.selected_keys.set(initial_selected_keys);
        view_model.checked_keys.set(initial_checked_keys);
        let tree = row_selection_benchmark_tree(
            kind,
            rows,
            viewport,
            view_model.selected_keys.clone(),
            selection_mode,
            view_model.checked_keys.clone(),
            tree_checkable,
        );
        let (mut handler, renderer) = interaction_handler_with_vm(
            view_model,
            tree,
            viewport,
            reduced_motion,
            invalidation,
            animations,
        )?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let row_targets = match kind {
            RuntimeRowSelectionKind::List => [
                row_hover_point(&mut handler, RuntimeRowHoverKind::List, 0)?,
                row_hover_point(&mut handler, RuntimeRowHoverKind::List, 1)?,
            ],
            RuntimeRowSelectionKind::Tree => [
                row_hover_point(&mut handler, RuntimeRowHoverKind::Tree, 0)?,
                row_hover_point(&mut handler, RuntimeRowHoverKind::Tree, 1)?,
            ],
            RuntimeRowSelectionKind::DataGrid => [
                data_grid_cell_point(&mut handler, 0, "name")?,
                data_grid_cell_point(&mut handler, 1, "name")?,
            ],
        };
        let row_ids = row_targets.map(|target| target.0);
        let row_points = row_targets.map(|target| target.1);
        let mut context = Self {
            handler,
            renderer,
            row_ids,
            row_points,
            viewport,
            kind,
            selection_enabled: selection_mode != RuntimeRowSelectionMode::None,
            adapter_name,
            backend,
        };
        let _ = context.render_scene()?;
        context.prepare_pointer(false)?;
        Ok(context)
    }

    pub fn prepare_pointer(&mut self, second_row: bool) -> Result<(), String> {
        if self.handler.pressed_widget.is_some() {
            return Err("cannot prepare pointer while a row remains pressed".to_string());
        }
        self.handler.pending_click = None;
        self.handler.cursor_position = Some(self.row_points[usize::from(second_row)]);
        if self.handler.handle_hover(self.viewport) {
            let _ = self.render_scene()?;
        }
        Ok(())
    }

    pub fn pointer_down(
        &mut self,
        second_row: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.prepare_pointer(second_row)?;
        self.measure_event(pointer_button_event(
            self.row_points[usize::from(second_row)],
            ElementState::Pressed,
        ))
    }

    pub fn pointer_up(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        let point = self
            .handler
            .cursor_position
            .ok_or_else(|| "pointer-up benchmark has no cursor position".to_string())?;
        self.measure_event(pointer_button_event(point, ElementState::Released))
    }

    pub fn keyboard_move(
        &mut self,
        next_row: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let physical_key = if next_row {
            PhysicalKey::Code(KeyCode::ArrowDown)
        } else {
            PhysicalKey::Code(KeyCode::ArrowUp)
        };
        self.measure_event(WindowEvent::KeyboardInput {
            device_id: None,
            event: KeyEvent {
                physical_key,
                logical_key: Key::Character(" ".into()),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
            },
        })
    }

    pub fn signal_only_select(
        &mut self,
        second_row: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler.with_view_model(|view_model| {
            view_model
                .selected_keys
                .set(vec![WidgetKey::from(if second_row {
                    "row-1"
                } else {
                    "row-0"
                })]);
        });
        let state_mutation = mutation_started.elapsed();
        let changed = revision_before != self.handler.invalidation.revision();
        let profile = changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        Ok(interaction_stats(
            changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        ))
    }

    pub fn signal_only_set_selected_keys(
        &mut self,
        selected_keys: Vec<WidgetKey>,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler
            .with_view_model(|view_model| view_model.selected_keys.set(selected_keys));
        let state_mutation = mutation_started.elapsed();
        let changed = revision_before != self.handler.invalidation.revision();
        let profile = changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        Ok(interaction_stats(
            changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        ))
    }

    pub fn signal_only_set_checked_keys(
        &mut self,
        checked_keys: Vec<WidgetKey>,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        begin_interaction_probe();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.handler
            .with_view_model(|view_model| view_model.checked_keys.set(checked_keys));
        let state_mutation = mutation_started.elapsed();
        let changed = revision_before != self.handler.invalidation.revision();
        let profile = changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        Ok(interaction_stats(
            changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        ))
    }

    pub fn selected_keys(&self) -> Vec<WidgetKey> {
        self.handler
            .with_view_model(|view_model| view_model.selected_keys.get())
    }

    pub fn checked_keys(&self) -> Vec<WidgetKey> {
        self.handler
            .with_view_model(|view_model| view_model.checked_keys.get())
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    pub fn scene_debug_summary(&self) -> String {
        let Some(cached) = self.handler.cached_scene.as_ref() else {
            return "cache=none".to_string();
        };
        let row_chunks = self
            .row_ids
            .iter()
            .map(|row_id| {
                let chunk = cached.scene_chunks.get(row_id);
                let shapes = chunk
                    .into_iter()
                    .flat_map(|chunk| chunk.scene.shapes.iter())
                    .map(|shape| {
                        (
                            shape.rect,
                            shape.color,
                            shape.corner_radius,
                            shape.stroke_width,
                            shape.clip_rect,
                        )
                    })
                    .collect::<Vec<_>>();
                let commands = chunk
                    .map(|chunk| {
                        chunk
                            .scene
                            .commands
                            .iter()
                            .map(|command| std::mem::discriminant(command))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let hits = cached
                    .computed
                    .hit_regions
                    .iter()
                    .filter(|region| match &region.interaction {
                        HitInteraction::ListItem { id, .. }
                        | HitInteraction::TreeNode { id, .. } => id == row_id,
                        HitInteraction::DataGridCell { state, .. } => state.row_id == *row_id,
                        _ => false,
                    })
                    .map(|region| {
                        (
                            region.rect,
                            region.clip_rect,
                            region.interaction.target_id(),
                        )
                    })
                    .collect::<Vec<_>>();
                (row_id, shapes, commands, hits)
            })
            .collect::<Vec<_>>();
        let hovered = self
            .handler
            .hovered_widgets
            .iter()
            .map(|hovered| match hovered.target_id {
                super::HoverTargetId::Widget(id) => format!("widget:{id:?}"),
                super::HoverTargetId::SplitterHandle { widget_id, .. } => {
                    format!("splitter:{widget_id:?}")
                }
                super::HoverTargetId::SelectOption { widget_id, .. } => {
                    format!("select:{widget_id:?}")
                }
                super::HoverTargetId::CanvasItem { widget_id, .. } => {
                    format!("canvas:{widget_id:?}")
                }
            })
            .collect::<Vec<_>>();
        let virtual_layout = cached
            .layout
            .as_ref()
            .into_iter()
            .flat_map(|layout| layout.all_widget_ids())
            .filter_map(|id| {
                let widget = cached.layout.as_ref()?.resolved_widget(id)?;
                let crate::ui::widget::ResolvedWidgetKind::Virtual {
                    runtime_state,
                    window_plan,
                    children,
                    ..
                } = &widget.kind
                else {
                    return None;
                };
                Some((
                    id,
                    runtime_state
                        .viewport_hint
                        .as_ref()
                        .map(|hint| (hint.width, hint.height)),
                    (
                        runtime_state.fallback_viewport_hint.width,
                        runtime_state.fallback_viewport_hint.height,
                    ),
                    (
                        window_plan.viewport_hint.width,
                        window_plan.viewport_hint.height,
                    ),
                    window_plan.visible_range.clone(),
                    window_plan.placements.len(),
                    children.len(),
                    window_plan.bootstrap,
                ))
            })
            .collect::<Vec<_>>();
        let virtual_cache = self
            .handler
            .virtual_states
            .iter()
            .map(|(id, state)| {
                (
                    *id,
                    state
                        .viewport_hint
                        .as_ref()
                        .map(|hint| (hint.width, hint.height)),
                    state.widget_ids_by_key.len(),
                )
            })
            .collect::<Vec<_>>();
        format!(
            "layout_valid={} computed_valid={} cached_focus={:?} runtime_focus={:?} focus_visible={} cached_pressed={:?} runtime_pressed={:?} cached_hover_epoch={} runtime_hover_epoch={} hovered={hovered:?} root_shapes={} root_commands={} root_hits={} virtual_layout={virtual_layout:?} virtual_cache={virtual_cache:?} row_chunks={row_chunks:?}",
            cached.layout_valid,
            cached.computed_valid,
            cached.focused_widget,
            self.handler.focused_widget_id(),
            self.handler.focus_visible,
            cached.pressed_widget,
            self.handler.pressed_widget,
            cached.hover_epoch,
            self.handler.hover_epoch,
            cached.computed.scene.shapes.len(),
            cached.computed.scene.commands.len(),
            cached.computed.hit_regions.len(),
        )
    }

    pub fn render_full_layout_control(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        begin_interaction_probe();
        let total_started = Instant::now();
        self.handler
            .invalidate_scene_with_reason("row_selection_benchmark_full_layout_control");
        let profile = self.render_scene();
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            false,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }

    fn measure_event(
        &mut self,
        event: WindowEvent,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        let focused_before = self.handler.focused_widget_id();
        let pressed_before = self.handler.pressed_widget;
        let dispatches_before = self
            .handler
            .with_view_model(|view_model| view_model.selection_dispatches);
        begin_interaction_probe();
        let total_started = Instant::now();
        let event_started = Instant::now();
        let _ = self
            .handler
            .handle_bound_window_event(&RuntimeBenchEventLoop, event);
        let event_handling = event_started.elapsed();
        let dispatches_after = self
            .handler
            .with_view_model(|view_model| view_model.selection_dispatches);
        let event_changed = focused_before != self.handler.focused_widget_id()
            || pressed_before != self.handler.pressed_widget
            || revision_before != self.handler.invalidation.revision()
            || hover_epoch_before != self.handler.hover_epoch;
        let profile = event_changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        let mut stats = interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        );
        stats.command_dispatches_delta = dispatches_after.saturating_sub(dispatches_before);
        Ok(stats)
    }

    fn render_scene(&mut self) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, true)
    }
}

pub struct RuntimeFocusBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    focus_order: Vec<WidgetId>,
    focused_index: Option<usize>,
    pub adapter_name: String,
    pub backend: String,
}

impl RuntimeFocusBenchmarkContext {
    pub fn new(buttons: usize, viewport: Rect) -> Result<Self, String> {
        Self::new_with_command_effect(buttons, viewport, CommandEffect::Conservative)
    }

    pub fn new_with_command_effect(
        buttons: usize,
        viewport: Rect,
        command_effect: CommandEffect,
    ) -> Result<Self, String> {
        if buttons == 0 {
            return Err("focus benchmark requires at least one button".to_string());
        }

        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        let (tree, expected_focus_order) = focus_benchmark_tree(buttons, viewport, command_effect);
        let (mut handler, renderer) = interaction_handler_with_vm(
            view_model,
            tree,
            viewport,
            false,
            invalidation,
            animations,
        )?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let focus_order = handler
            .focusable_widgets_in_tab_order()
            .into_iter()
            .map(|candidate| candidate.widget_id)
            .collect::<Vec<_>>();
        if focus_order.len() != buttons {
            return Err(format!(
                "focus benchmark expected {buttons} candidates, collected {}",
                focus_order.len()
            ));
        }
        if focus_order.iter().copied().collect::<HashSet<_>>().len() != buttons {
            return Err("focus benchmark candidates did not have unique widget ids".to_string());
        }
        if focus_order != expected_focus_order {
            return Err(
                "focus benchmark Tab order did not match the dense button insertion order"
                    .to_string(),
            );
        }
        if handler.focused_widget_id().is_some() {
            return Err("focus benchmark unexpectedly started with a focused widget".to_string());
        }

        let mut context = Self {
            handler,
            renderer,
            focus_order,
            focused_index: None,
            adapter_name,
            backend,
        };
        let _ = context.render_scene()?;
        Ok(context)
    }

    pub fn focusable_count(&self) -> usize {
        self.focus_order.len()
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub fn activation_dispatches(&self) -> usize {
        self.handler
            .with_view_model(|view_model| view_model.focus_activation_dispatches)
    }

    pub fn last_activated_index(&self) -> Option<usize> {
        self.handler
            .with_view_model(|view_model| view_model.last_focus_activation)
    }

    pub fn tab_forward(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        let expected_index = self
            .focused_index
            .map_or(0, |index| (index + 1) % self.focus_order.len());
        let expected_id = self.focus_order[expected_index];
        let stats = self.measure_event(focus_keyboard_event(KeyCode::Tab))?;
        if self.handler.focused_widget_id() != Some(expected_id) {
            return Err(format!(
                "Tab focused {:?}, expected candidate {expected_index} ({expected_id:?})",
                self.handler.focused_widget_id()
            ));
        }
        if stats.command_dispatches_delta != 0 {
            return Err("Tab unexpectedly dispatched an activation command".to_string());
        }
        self.focused_index = Some(expected_index);
        Ok(stats)
    }

    pub fn activate_enter(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        self.activate(KeyCode::Enter)
    }

    pub fn activate_space(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        self.activate(KeyCode::Space)
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn activate(&mut self, key: KeyCode) -> Result<RuntimeInteractionFrameStats, String> {
        let expected_index = self
            .focused_index
            .ok_or_else(|| "focus benchmark activation requires an initial Tab".to_string())?;
        let dispatches_before = self.activation_dispatches();
        let stats = self.measure_event(focus_keyboard_event(key))?;
        if stats.command_dispatches_delta != 1 {
            return Err(format!(
                "{key:?} dispatched {} commands instead of one",
                stats.command_dispatches_delta
            ));
        }
        if self.activation_dispatches() != dispatches_before + 1 {
            return Err(format!(
                "{key:?} activation counter did not advance exactly once"
            ));
        }
        if self.last_activated_index() != Some(expected_index) {
            return Err(format!(
                "{key:?} activated {:?}, expected focused candidate {expected_index}",
                self.last_activated_index()
            ));
        }
        Ok(stats)
    }

    fn measure_event(
        &mut self,
        event: WindowEvent,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        let focused_before = self.handler.focused_widget_id();
        let dispatches_before = self.activation_dispatches();
        begin_interaction_probe();
        let total_started = Instant::now();
        let event_started = Instant::now();
        let _ = self
            .handler
            .handle_bound_window_event(&RuntimeBenchEventLoop, event);
        let event_handling = event_started.elapsed();
        let dispatches_after = self.activation_dispatches();
        let event_changed = focused_before != self.handler.focused_widget_id()
            || revision_before != self.handler.invalidation.revision()
            || hover_epoch_before != self.handler.hover_epoch;
        let profile = event_changed.then(|| self.render_scene()).transpose()?;
        let (paths, actions) = finish_interaction_probe();
        let mut stats = interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        );
        stats.command_dispatches_delta = dispatches_after.saturating_sub(dispatches_before);
        Ok(stats)
    }

    fn render_scene(&mut self) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, true)
    }
}

#[derive(Debug)]
struct RuntimeBenchEventLoop;

impl ActiveEventLoop for RuntimeBenchEventLoop {
    fn create_proxy(&self) -> EventLoopProxy {
        panic!("runtime interaction benchmark does not install event-loop proxies")
    }

    fn create_window(
        &self,
        _attributes: WindowAttributes,
    ) -> Result<Box<dyn Window>, RequestError> {
        panic!("runtime interaction benchmark does not create windows")
    }

    fn set_control_flow(&self, _control_flow: ControlFlow) {}

    fn control_flow(&self) -> ControlFlow {
        ControlFlow::Wait
    }

    fn exit(&self) {}

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle> + '_> {
        Box::new(std::iter::empty())
    }
}

fn pointer_button_event(point: Point, state: ElementState) -> WindowEvent {
    WindowEvent::PointerButton {
        device_id: None,
        state,
        position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
        button: ButtonSource::Mouse(MouseButton::Left),
        primary: true,
    }
}

fn pointer_moved_event(point: Point) -> WindowEvent {
    WindowEvent::PointerMoved {
        device_id: None,
        position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
        primary: true,
        source: PointerSource::Mouse,
    }
}

fn focus_keyboard_event(key_code: KeyCode) -> WindowEvent {
    WindowEvent::KeyboardInput {
        device_id: None,
        event: KeyEvent {
            physical_key: PhysicalKey::Code(key_code),
            logical_key: if key_code == KeyCode::Tab {
                Key::Named(NamedKey::Tab)
            } else {
                Key::Character(" ".into())
            },
            text: None,
            location: KeyLocation::Standard,
            state: ElementState::Pressed,
            repeat: false,
        },
    }
}

impl RuntimeDataGridHoverTarget {
    fn index(self) -> usize {
        match self {
            Self::FirstRowStartPinned => 0,
            Self::FirstRowUnpinned => 1,
            Self::FirstRowEndPinned => 2,
            Self::SecondRowStartPinned => 3,
        }
    }
}

pub struct RuntimeInteractionBenchmarkVm {
    toasts: ToastQueue<Self>,
    selected_keys: State<Vec<WidgetKey>>,
    checked_keys: State<Vec<WidgetKey>>,
    slider_value: State<f32>,
    text_content: State<String>,
    selection_dispatches: usize,
    focus_activation_dispatches: usize,
    last_focus_activation: Option<usize>,
}

impl ViewModel for RuntimeInteractionBenchmarkVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            toasts: ToastQueue::new(context),
            selected_keys: context.state(Vec::new()),
            checked_keys: context.state(Vec::new()),
            slider_value: context.state(0.25),
            text_content: context.state(String::from("Frame 000000")),
            selection_dispatches: 0,
            focus_activation_dispatches: 0,
            last_focus_activation: None,
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

pub struct RuntimeDataGridBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    hover_points: [Point; 4],
    viewport: Rect,
    pub adapter_name: String,
    pub backend: String,
}

impl RuntimeDataGridBenchmarkContext {
    pub fn new(rows: usize, viewport: Rect) -> Result<Self, String> {
        let tree = data_grid_benchmark_tree(rows, viewport);
        let (mut handler, renderer) = interaction_handler(tree, viewport, false)?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let first_start = data_grid_cell_point(&mut handler, 0, "id")?;
        let first_middle = data_grid_cell_point(&mut handler, 0, "name")?;
        let first_end = data_grid_cell_point(&mut handler, 0, "status")?;
        let second_start = data_grid_cell_point(&mut handler, 1, "id")?;
        if first_start.0 != first_middle.0 || first_middle.0 != first_end.0 {
            return Err("pinned and unpinned cells did not share one logical row id".to_string());
        }
        if first_start.0 == second_start.0 {
            return Err("adjacent DataGrid rows unexpectedly share one row id".to_string());
        }
        let mut context = Self {
            handler,
            renderer,
            hover_points: [first_start.1, first_middle.1, first_end.1, second_start.1],
            viewport,
            adapter_name,
            backend,
        };
        let _ = context.render_scene(true)?;
        Ok(context)
    }

    pub fn move_hover(
        &mut self,
        target: RuntimeDataGridHoverTarget,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(target, true)
    }

    pub fn move_hover_scene_recollect_control(
        &mut self,
        target: RuntimeDataGridHoverTarget,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.move_hover_with_mode(target, false)
    }

    fn move_hover_with_mode(
        &mut self,
        target: RuntimeDataGridHoverTarget,
        allow_retained_hover_patch: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let point = self.hover_points[target.index()];
        let revision_before = self.handler.invalidation.revision();
        let hover_epoch_before = self.handler.hover_epoch;
        begin_interaction_probe();
        let total_started = Instant::now();
        self.handler.cursor_position = Some(point);
        let event_started = Instant::now();
        let event_changed = self.handler.handle_hover(self.viewport);
        let event_handling = event_started.elapsed();
        if !allow_retained_hover_patch {
            self.handler.row_hover_patch_pending = None;
        }
        let profile = if event_changed {
            Some(self.render_scene(true))
        } else {
            None
        };
        let (paths, actions) = finish_interaction_probe();
        let profile = profile.transpose()?;
        Ok(interaction_stats(
            event_changed,
            profile.is_some(),
            self.handler.hover_epoch.wrapping_sub(hover_epoch_before),
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            event_handling,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            profile,
        ))
    }

    pub fn render_full_layout_control(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        let revision_before = self.handler.invalidation.revision();
        begin_interaction_probe();
        let total_started = Instant::now();
        self.handler
            .invalidate_scene_with_reason("data_grid_benchmark_full_layout_control");
        let profile = self.render_scene(true);
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            false,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn render_scene(&mut self, sync_bindings: bool) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, sync_bindings)
    }
}

pub struct RuntimeToastBenchmarkContext {
    handler: BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: HeadlessBenchRenderer,
    queue: ToastQueue<RuntimeInteractionBenchmarkVm>,
    toast_host_id: WidgetId,
    pub adapter_name: String,
    pub backend: String,
}

impl RuntimeToastBenchmarkContext {
    pub fn new(viewport: Rect, reduced_motion: bool) -> Result<Self, String> {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
        let queue = view_model.toasts.clone();
        let tree = WidgetTree::new(
            Stack::new()
                .child(Text::new("Interaction frame budget"))
                .child(
                    ToastHost::new(queue.clone())
                        .placement(ToastPlacement::BottomEnd)
                        .max_visible(50),
                ),
        );
        let (handler, renderer) = interaction_handler_with_vm(
            view_model,
            tree,
            viewport,
            reduced_motion,
            invalidation,
            animations,
        )?;
        let adapter_name = renderer.adapter_name.clone();
        let backend = renderer.backend.clone();
        let mut context = Self {
            handler,
            renderer,
            queue,
            toast_host_id: WidgetId::from_raw(0),
            adapter_name,
            backend,
        };
        let _ = context.render_scene(true)?;
        context.toast_host_id = context
            .handler
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| {
                layout.all_widget_ids().find(|widget_id| {
                    layout.resolved_widget(*widget_id).is_some_and(|widget| {
                        matches!(
                            widget.kind,
                            crate::ui::widget::ResolvedWidgetKind::ToastHost { .. }
                        )
                    })
                })
            })
            .ok_or_else(|| "Toast benchmark tree did not retain its ToastHost".to_string())?;
        Ok(context)
    }

    /// Prime the benchmark-only prepared-card cache with the current queue contents.
    pub fn prime_prepared_card_cache(&mut self) -> Result<(), String> {
        let _ = self.render_prepared_card_recollect(true)?;
        Ok(())
    }

    /// Prime the benchmark-only canonical per-card scene cache.
    pub fn prime_toast_base_scene_cache(&mut self) -> Result<(), String> {
        let result = crate::ui::widget::with_toast_base_scene_replay(|| {
            self.render_prepared_card_recollect(true)
        });
        let _ = result?;
        Ok(())
    }

    /// Recollect the same ToastHost while reusing its resolved/Taffy card trees.
    pub fn render_prepared_card_cache_frame(
        &mut self,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.render_prepared_card_recollect(true)
    }

    /// Recollect the ToastHost by replaying canonical per-card scenes.
    pub fn render_toast_base_scene_cache_frame(
        &mut self,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        crate::ui::widget::with_toast_base_scene_replay(|| {
            self.render_prepared_card_recollect(true)
        })
    }

    /// A/B control that reuses the same prepared layout but runs the original scene collector.
    pub fn render_toast_base_scene_control_frame(
        &mut self,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        crate::ui::widget::without_toast_base_scene_replay(|| {
            self.render_prepared_card_recollect(true)
        })
    }

    /// A/B control for `render_prepared_card_cache_frame` that rebuilds every card tree.
    pub fn render_prepared_card_control_frame(
        &mut self,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        self.render_prepared_card_recollect(false)
    }

    pub fn prepare_empty(&mut self) -> Result<(), String> {
        let now = Instant::now();
        self.queue.clear_at(now - Duration::from_secs(2));
        let _ = self.queue.flush_expired_after(now, Duration::ZERO);
        self.queue.set_stack_expanded(false);
        let _ = self.render_scene(true)?;
        Ok(())
    }

    pub fn prepare_entering(
        &mut self,
        count: usize,
        enter_elapsed: Duration,
    ) -> Result<(), String> {
        self.prepare_empty()?;
        let created_at = Instant::now() - enter_elapsed;
        self.push_toasts(count, created_at);
        self.queue.set_stack_expanded(count > 3);
        let _ = self.render_scene(true)?;
        Ok(())
    }

    pub fn prepare_settled(&mut self, count: usize) -> Result<(), String> {
        self.prepare_empty()?;
        self.push_toasts(count, Instant::now() - Duration::from_secs(1));
        self.queue.set_stack_expanded(count > 3);
        let _ = self.render_scene(true)?;
        Ok(())
    }

    pub fn render_insert_frame(
        &mut self,
        count: usize,
        enter_elapsed: Duration,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        begin_interaction_probe();
        let revision_before = self.handler.invalidation.revision();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        self.push_toasts(count, Instant::now() - enter_elapsed);
        self.queue.set_stack_expanded(count > 3);
        let state_mutation = mutation_started.elapsed();
        let profile = self.render_scene(true);
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            true,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }

    pub fn render_insert_frame_legacy_double_layout(
        &mut self,
        count: usize,
        enter_elapsed: Duration,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        crate::ui::widget::toast_scene_bench_profile::with_legacy_double_layout(|| {
            self.render_insert_frame(count, enter_elapsed)
        })
    }

    pub fn render_dismiss_frame(
        &mut self,
        index: usize,
        exit_elapsed: Duration,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        let entries = self.queue.snapshot();
        let id = entries
            .get(index)
            .map(|entry| entry.id)
            .ok_or_else(|| format!("toast index {index} is not present"))?;
        begin_interaction_probe();
        let revision_before = self.handler.invalidation.revision();
        let total_started = Instant::now();
        let mutation_started = Instant::now();
        let changed = self.queue.dismiss_at(id, Instant::now() - exit_elapsed);
        let state_mutation = mutation_started.elapsed();
        if !changed {
            let _ = finish_interaction_probe();
            return Err(format!("toast {id:?} was already dismissed"));
        }
        let profile = self.render_scene(true);
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            true,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            state_mutation,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }

    pub fn render_dismiss_frame_legacy_double_layout(
        &mut self,
        index: usize,
        exit_elapsed: Duration,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        crate::ui::widget::toast_scene_bench_profile::with_legacy_double_layout(|| {
            self.render_dismiss_frame(index, exit_elapsed)
        })
    }

    pub fn render_full_layout_control(&mut self) -> Result<RuntimeInteractionFrameStats, String> {
        begin_interaction_probe();
        let revision_before = self.handler.invalidation.revision();
        let total_started = Instant::now();
        self.handler
            .invalidate_scene_with_reason("toast_benchmark_full_layout_control");
        let profile = self.render_scene(false);
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            false,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }

    pub fn render_full_layout_legacy_double_layout_control(
        &mut self,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        crate::ui::widget::toast_scene_bench_profile::with_legacy_double_layout(|| {
            self.render_full_layout_control()
        })
    }

    pub fn read_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    fn push_toasts(&self, count: usize, created_at: Instant) -> Vec<ToastId> {
        (0..count)
            .map(|index| {
                self.queue.push_at(
                    Toast::new(format!("Background task {index:02} completed"))
                        .title(format!("Update {index:02}"))
                        .duration(Duration::from_secs(30)),
                    created_at,
                )
            })
            .collect()
    }

    fn render_scene(&mut self, sync_bindings: bool) -> Result<InteractionRenderProfile, String> {
        render_interaction_scene(&mut self.handler, &mut self.renderer, sync_bindings)
    }

    fn render_prepared_card_recollect(
        &mut self,
        use_prepared_cache: bool,
    ) -> Result<RuntimeInteractionFrameStats, String> {
        begin_interaction_probe();
        let revision_before = self.handler.invalidation.revision();
        let total_started = Instant::now();
        let host_id = self.toast_host_id;
        let now = Instant::now();
        let patched = if use_prepared_cache {
            self.handler.invalidate_computed_scene_for_toast_motion();
            true
        } else {
            self.handler
                .patch_cached_scene_for_roots(&[host_id], now, false)
        };
        if !patched {
            let _ = finish_interaction_probe();
            return Err("Toast prepared-card benchmark patch fell back".to_string());
        }
        let profile = self.render_scene(false);
        let (paths, actions) = finish_interaction_probe();
        let profile = profile?;
        Ok(interaction_stats(
            false,
            true,
            0,
            self.handler
                .invalidation
                .revision()
                .wrapping_sub(revision_before),
            Duration::ZERO,
            Duration::ZERO,
            total_started.elapsed(),
            paths,
            &actions,
            Some(profile),
        ))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct InteractionRenderProfile {
    sync_bindings: Duration,
    scene_update: Duration,
    render: crate::rendering::renderer::BenchRenderProfile,
}

fn interaction_stats(
    event_changed: bool,
    rendered: bool,
    hover_epoch_delta: u64,
    invalidation_revision_delta: u64,
    event_handling: Duration,
    state_mutation: Duration,
    total: Duration,
    paths: frame_path_probe::Snapshot,
    actions: &[(&'static str, u64)],
    profile: Option<InteractionRenderProfile>,
) -> RuntimeInteractionFrameStats {
    let profile = profile.unwrap_or_default();
    let toast_profile = crate::ui::widget::toast_scene_bench_profile::snapshot();
    let action_count = |predicate: fn(&str) -> bool| {
        actions
            .iter()
            .filter(|(action, _)| predicate(action))
            .map(|(_, count)| *count as usize)
            .sum()
    };
    RuntimeInteractionFrameStats {
        event_changed,
        rendered,
        command_dispatches_delta: 0,
        hover_epoch_delta,
        invalidation_revision_delta,
        cache_hits: paths.cache_hits as usize,
        scene_recollects: paths.scene_recollects as usize,
        layout_reuses: paths.layout_reuses as usize,
        layout_builds: paths.layout_builds as usize,
        retained_scene_patches: action_count(|action| {
            action == "scene_subtree_patch"
                || action == "text_input_scene_patch"
                || action == "reactive_scene_patch"
                || action == "reactive_property_scene_patch"
                || action == "animation_reactive_property_slot_write"
                || action == "toast_prepared_card_scene_patch"
                || action == "row_hover_scene_patch"
                || action == "button_hover_scene_patch"
                || action == "button_pressed_scene_patch"
        }),
        reactive_property_slot_writes: action_count(|action| {
            action == "reactive_property_slot_write"
        }),
        layout_patch_actions: action_count(|action| {
            action.contains("layout") && action.contains("patch")
        }),
        full_rebuild_actions: action_count(|action| {
            action == "global_full_rebuild"
                || action == "layout_missing"
                || action.contains("full_rebuild")
        }),
        event_handling,
        state_mutation,
        sync_bindings: profile.sync_bindings,
        scene_update: profile.scene_update,
        toast_measure: toast_profile.measure,
        toast_collect: toast_profile.collect,
        toast_compose: toast_profile.compose,
        toast_measured_cards: toast_profile.measured_cards,
        toast_collected_cards: toast_profile.collected_cards,
        toast_layout_passes: toast_profile.layout_passes,
        toast_base_scene_replay_hits: toast_profile.base_scene_replay_hits,
        toast_base_scene_replay_fallbacks: toast_profile.base_scene_replay_fallbacks,
        renderer_liveness: profile.render.liveness,
        renderer_prepare_upload: profile.render.prepare_upload,
        renderer_encode: profile.render.encode,
        queue_submit: profile.render.submit,
        gpu_wait: profile.render.gpu_wait,
        total,
    }
}

fn begin_interaction_probe() {
    frame_path_probe::begin();
    super::action_stats::reset();
    crate::ui::widget::toast_scene_bench_profile::reset();
}

fn finish_interaction_probe() -> (frame_path_probe::Snapshot, Vec<(&'static str, u64)>) {
    let paths = frame_path_probe::finish();
    let actions = super::action_stats::snapshot();
    (paths, actions)
}

fn render_interaction_scene(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    renderer: &mut HeadlessBenchRenderer,
    sync_bindings: bool,
) -> Result<InteractionRenderProfile, String> {
    let sync_started = Instant::now();
    if sync_bindings {
        // Match the production redraw path: this consumes the invalidation revision, resolves
        // dependency ownership, and only then lets `computed_scene` choose retained scene-only
        // recollection or the full-layout fallback.
        handler.request_redraw_if_dirty(Instant::now());
    }
    let sync_bindings = sync_started.elapsed();
    let font_manager = Arc::clone(&handler.font_manager);
    let scene_started = Instant::now();
    let computed = handler.computed_scene_mut();
    let scene_update = scene_started.elapsed();
    renderer
        .render_and_wait(
            &mut computed.scene,
            font_manager.as_ref(),
            &computed.scroll_regions,
            &computed.transform_records,
        )
        .map_err(|error| error.to_string())?;
    Ok(InteractionRenderProfile {
        sync_bindings,
        scene_update,
        render: renderer.last_render_profile(),
    })
}

fn interaction_handler(
    tree: WidgetTree<RuntimeInteractionBenchmarkVm>,
    viewport: Rect,
    reduced_motion: bool,
) -> Result<
    (
        BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
        HeadlessBenchRenderer,
    ),
    String,
> {
    let invalidation = InvalidationSignal::new();
    let animations = AnimationCoordinator::default();
    let view_model_context = ViewModelContext::new(invalidation.clone(), animations.clone());
    let view_model = RuntimeInteractionBenchmarkVm::new(&view_model_context);
    interaction_handler_with_vm(
        view_model,
        tree,
        viewport,
        reduced_motion,
        invalidation,
        animations,
    )
}

fn interaction_handler_with_vm(
    view_model: RuntimeInteractionBenchmarkVm,
    tree: WidgetTree<RuntimeInteractionBenchmarkVm>,
    viewport: Rect,
    reduced_motion: bool,
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
) -> Result<
    (
        BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
        HeadlessBenchRenderer,
    ),
    String,
> {
    let width = viewport.width.get().max(1.0).ceil() as u32;
    let height = viewport.height.get().max(1.0).ceil() as u32;
    let renderer = HeadlessBenchRenderer::new(PhysicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    let handler = interaction_runtime_handler_with_vm(
        view_model,
        tree,
        viewport,
        reduced_motion,
        invalidation,
        animations,
    );
    Ok((handler, renderer))
}

fn interaction_runtime_handler_with_vm(
    view_model: RuntimeInteractionBenchmarkVm,
    tree: WidgetTree<RuntimeInteractionBenchmarkVm>,
    viewport: Rect,
    reduced_motion: bool,
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
) -> BoundRuntimeHandler<RuntimeInteractionBenchmarkVm> {
    let width = viewport.width.get().max(1.0).ceil() as u32;
    let height = viewport.height.get().max(1.0).ceil() as u32;
    let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
    let (notification_dispatcher, notification_receiver) = async_notification_channel();
    let (task_dispatcher, task_receiver) = async_task_channel();
    let config = ApplicationConfig {
        app_id: None,
        title: "runtime-interaction-benchmark".to_string(),
        size: LogicalSize::new(f64::from(width), f64::from(height)),
        min_size: None,
        max_size: None,
        clear_color: Color::WHITE,
        clear_color_overridden: true,
        close_children_with_main: true,
        decorations: true,
        viewport_insets: Insets::ZERO,
        msaa: MsaaMode::Off,
        fonts: FontCatalog::default(),
        theme: ThemeSelection::Mode(ThemeMode::Light),
        theme_set: ThemeSet::default(),
        style_sheet: crate::ui::widget::StyleSheet::default(),
        reduced_motion,
        window_icon: None,
        resource_budget: ResourceBudget::DEFAULT,
    };
    BoundRuntimeHandler::new(
        "runtime-interaction-benchmark".to_string(),
        1,
        WindowRole::Main,
        config,
        Arc::new(Mutex::new(view_model)),
        WindowBindings::default(),
        Some(tree),
        None,
        Vec::new(),
        invalidation,
        animations,
        dialog_dispatcher,
        Some(dialog_receiver),
        notification_dispatcher,
        Some(notification_receiver),
        task_dispatcher,
        Some(task_receiver),
    )
}

fn data_grid_benchmark_tree(
    rows: usize,
    viewport: Rect,
) -> WidgetTree<RuntimeInteractionBenchmarkVm> {
    let columns: Vec<DataGridColumn<usize, RuntimeInteractionBenchmarkVm>> = vec![
        DataGridColumn::new("id", "ID".to_string(), |context| {
            Text::new(format!("#{:05}", context.row)).into()
        })
        .width(dp(96.0))
        .pin(DataGridColumnPin::Start),
        DataGridColumn::new("name", "Name".to_string(), |context| {
            Text::new(format!("Production row {:05}", context.row)).into()
        })
        .width(dp(280.0)),
        DataGridColumn::new("metric", "Metric".to_string(), |context| {
            Text::new(format!("{} ms", context.row % 97)).into()
        })
        .width(dp(240.0)),
        DataGridColumn::new("owner", "Owner".to_string(), |context| {
            Text::new(format!("team-{}", context.row % 23)).into()
        })
        .width(dp(240.0)),
        DataGridColumn::new("status", "Status".to_string(), |_context| {
            Text::new("Ready").into()
        })
        .width(dp(112.0))
        .pin(DataGridColumnPin::End),
    ];
    let rows = (0..rows)
        .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    WidgetTree::new(
        DataGrid::<usize, RuntimeInteractionBenchmarkVm>::new(rows, columns)
            .size(viewport.width, viewport.height)
            .row_height(dp(34.0))
            .overscan(4),
    )
}

fn focus_benchmark_tree(
    buttons: usize,
    viewport: Rect,
    command_effect: CommandEffect,
) -> (WidgetTree<RuntimeInteractionBenchmarkVm>, Vec<WidgetId>) {
    let columns = (buttons as f64).sqrt().ceil().max(1.0) as usize;
    let rows = buttons.div_ceil(columns);
    let cell_width = (viewport.width.get() / columns as f32).max(1.0);
    let cell_height = (viewport.height.get() / rows as f32).max(1.0);
    // Keep every control inside the viewport. A virtual or clipped 10k-row list would only retain
    // its visible window and would therefore benchmark a few dozen focus candidates, not 10k.
    let mut surface = Stack::new().size(viewport.width, viewport.height);
    let mut expected_focus_order = Vec::with_capacity(buttons);

    for index in 0..buttons {
        let column = index % columns;
        let row = index / columns;
        let button: Element<RuntimeInteractionBenchmarkVm> = Button::new("")
            .key(format!("focus-button-{index}"))
            .position_absolute()
            .left(dp(column as f32 * cell_width))
            .top(dp(row as f32 * cell_height))
            .size(dp(cell_width), dp(cell_height))
            .on_click(
                Command::new(move |view_model: &mut RuntimeInteractionBenchmarkVm| {
                    view_model.focus_activation_dispatches =
                        view_model.focus_activation_dispatches.wrapping_add(1);
                    view_model.last_focus_activation = Some(index);
                })
                .effect(command_effect),
            )
            .into();
        expected_focus_order.push(button.id);
        surface = surface.child(button);
    }

    (
        WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(viewport.width, viewport.height)
                .child(surface),
        ),
        expected_focus_order,
    )
}

fn button_hover_benchmark_tree(
    buttons: usize,
    viewport: Rect,
) -> (WidgetTree<RuntimeInteractionBenchmarkVm>, Vec<WidgetId>) {
    let columns = (buttons as f64).sqrt().ceil().max(1.0) as usize;
    let rows = buttons.div_ceil(columns);
    let cell_width = 48.0_f32;
    let cell_height = 48.0_f32;
    let content_width = dp(columns as f32 * cell_width);
    let content_height = dp(rows as f32 * cell_height);
    // Keep real control geometry instead of shrinking 10k buttons below their intrinsic minimum
    // and accidentally overlapping hit regions. The clipped, non-virtual surface still retains
    // and scene-collects all 10k Button nodes while the headless render target remains 960×720.
    let mut surface = Stack::new().size(content_width, content_height);
    let mut button_ids = Vec::with_capacity(buttons);

    for index in 0..buttons {
        let column = index % columns;
        let row = index / columns;
        let button: Element<RuntimeInteractionBenchmarkVm> = Button::new("")
            .key(format!("hover-button-{index}"))
            .position_absolute()
            .left(dp(column as f32 * cell_width))
            .top(dp(row as f32 * cell_height))
            .size(dp(cell_width), dp(cell_height))
            .into();
        button_ids.push(button.id);
        surface = surface.child(button);
    }

    (
        WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(viewport.width, viewport.height)
                .child(
                    Stack::new()
                        .size(viewport.width, viewport.height)
                        .overflow_x(Overflow::Hidden)
                        .overflow_y(Overflow::Hidden)
                        .child(surface),
                ),
        ),
        button_ids,
    )
}

fn row_hover_benchmark_tree(
    kind: RuntimeRowHoverKind,
    rows: usize,
    viewport: Rect,
) -> WidgetTree<RuntimeInteractionBenchmarkVm> {
    let item_layout = ItemLayout::Fixed {
        item_extent: dp(34.0),
        spacing: Dp::ZERO,
        overscan: 4,
    };
    match kind {
        RuntimeRowHoverKind::List => {
            let items = (0..rows)
                .map(|index| ListItem::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            WidgetTree::new(
                List::<usize, RuntimeInteractionBenchmarkVm>::new(items, |context| {
                    Text::new(format!("Production list row {:05}", context.item)).into()
                })
                .item_layout(item_layout)
                .size(viewport.width, viewport.height),
            )
        }
        RuntimeRowHoverKind::Tree => {
            let nodes = (0..rows)
                .map(|index| TreeNode::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            WidgetTree::new(
                Tree::<usize, RuntimeInteractionBenchmarkVm>::new(nodes, |context| {
                    Text::new(format!("Production tree row {:05}", context.item)).into()
                })
                .item_layout(item_layout)
                .size(viewport.width, viewport.height),
            )
        }
    }
}

fn slider_value_benchmark_tree(
    slider_count: usize,
    viewport: Rect,
    slider_value: State<f32>,
    with_shadow: bool,
) -> WidgetTree<RuntimeInteractionBenchmarkVm> {
    let value = slider_value.signal();
    let rows = (0..slider_count)
        .step_by(2)
        .map(|row_start| {
            let sliders = (row_start..(row_start + 2).min(slider_count))
                .map(|_| {
                    Slider::new(value.clone(), 0.0, 1.0)
                        .step(0.01)
                        .height(dp(32.0))
                        .grow(1.0)
                        .style(move |style, context| {
                            style.thumb_shadow =
                                with_shadow.then(|| context.theme.elevation.sm.clone());
                        })
                        .into()
                })
                .collect::<Vec<Element<RuntimeInteractionBenchmarkVm>>>();
            Flex::horizontal()
                .gap(dp(12.0))
                .height(dp(32.0))
                .child(sliders)
                .into()
        })
        .collect::<Vec<Element<RuntimeInteractionBenchmarkVm>>>();
    WidgetTree::new(
        Flex::vertical()
            .gap(dp(4.0))
            .padding(Insets::all(dp(8.0)))
            .size(viewport.width, viewport.height)
            .child(rows),
    )
}

fn text_content_benchmark_tree(
    text_count: usize,
    viewport: Rect,
    text_content: State<String>,
) -> WidgetTree<RuntimeInteractionBenchmarkVm> {
    let content = text_content.signal();
    let texts = (0..text_count)
        .map(|_| Text::new(content.clone()).size(dp(180.0), dp(20.0)).into())
        .collect::<Vec<Element<RuntimeInteractionBenchmarkVm>>>();
    WidgetTree::new(
        Stack::new()
            .size(viewport.width, viewport.height)
            .child(texts),
    )
}

fn row_selection_benchmark_tree(
    kind: RuntimeRowSelectionKind,
    rows: usize,
    viewport: Rect,
    selected_keys: State<Vec<WidgetKey>>,
    selection_mode: RuntimeRowSelectionMode,
    checked_keys: State<Vec<WidgetKey>>,
    tree_checkable: bool,
) -> WidgetTree<RuntimeInteractionBenchmarkVm> {
    let item_layout = ItemLayout::Fixed {
        item_extent: dp(34.0),
        spacing: Dp::ZERO,
        overscan: 4,
    };
    match kind {
        RuntimeRowSelectionKind::List => {
            let items = (0..rows)
                .map(|index| ListItem::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            let mut list = List::<usize, RuntimeInteractionBenchmarkVm>::new(items, |context| {
                Text::new(format!("Production list row {:05}", context.item)).into()
            })
            .item_layout(item_layout)
            .size(viewport.width, viewport.height);
            if selection_mode != RuntimeRowSelectionMode::None {
                list = list
                    .selection_mode(match selection_mode {
                        RuntimeRowSelectionMode::None => ListSelectionMode::None,
                        RuntimeRowSelectionMode::Single => ListSelectionMode::Single,
                        RuntimeRowSelectionMode::Multiple => ListSelectionMode::Multiple,
                    })
                    .selected_keys(selected_keys.signal())
                    .on_selection_change(ValueCommand::new(
                        |view_model: &mut RuntimeInteractionBenchmarkVm,
                         change: ListSelectionChange| {
                            view_model.selection_dispatches += 1;
                            view_model.selected_keys.set(change.selected_keys);
                        },
                    ));
            } else {
                list = list.selection_mode(ListSelectionMode::None);
            }
            WidgetTree::new(list)
        }
        RuntimeRowSelectionKind::Tree => {
            let nodes = (0..rows)
                .map(|index| TreeNode::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            let mut tree = Tree::<usize, RuntimeInteractionBenchmarkVm>::new(nodes, |context| {
                Text::new(format!("Production tree row {:05}", context.item)).into()
            })
            .item_layout(item_layout)
            .size(viewport.width, viewport.height);
            if tree_checkable {
                tree = tree.checkable(true).checked_keys(checked_keys.signal());
            }
            if selection_mode != RuntimeRowSelectionMode::None {
                tree = tree
                    .selection_mode(match selection_mode {
                        RuntimeRowSelectionMode::None => TreeSelectionMode::None,
                        RuntimeRowSelectionMode::Single => TreeSelectionMode::Single,
                        RuntimeRowSelectionMode::Multiple => TreeSelectionMode::Multiple,
                    })
                    .selected_keys(selected_keys.signal())
                    .on_selection_change(ValueCommand::new(
                        |view_model: &mut RuntimeInteractionBenchmarkVm,
                         change: TreeSelectionChange| {
                            view_model.selection_dispatches += 1;
                            view_model.selected_keys.set(change.selected_keys);
                        },
                    ));
            } else {
                tree = tree.selection_mode(TreeSelectionMode::None);
            }
            WidgetTree::new(tree)
        }
        RuntimeRowSelectionKind::DataGrid => {
            let columns: Vec<DataGridColumn<usize, RuntimeInteractionBenchmarkVm>> = vec![
                DataGridColumn::new("id", "ID".to_string(), |context| {
                    Text::new(format!("#{:05}", context.row)).into()
                })
                .width(dp(96.0))
                .pin(DataGridColumnPin::Start),
                DataGridColumn::new("name", "Name".to_string(), |context| {
                    Text::new(format!("Production row {:05}", context.row)).into()
                })
                .width(dp(280.0)),
                DataGridColumn::new("metric", "Metric".to_string(), |context| {
                    Text::new(format!("{} ms", context.row % 97)).into()
                })
                .width(dp(240.0)),
                DataGridColumn::new("owner", "Owner".to_string(), |context| {
                    Text::new(format!("team-{}", context.row % 23)).into()
                })
                .width(dp(240.0)),
                DataGridColumn::new("status", "Status".to_string(), |_context| {
                    Text::new("Ready").into()
                })
                .width(dp(112.0))
                .pin(DataGridColumnPin::End),
            ];
            let rows = (0..rows)
                .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            let mut grid = DataGrid::<usize, RuntimeInteractionBenchmarkVm>::new(rows, columns)
                .size(viewport.width, viewport.height)
                .row_height(dp(34.0))
                .overscan(4);
            if selection_mode != RuntimeRowSelectionMode::None {
                grid = grid
                    .selection_mode(match selection_mode {
                        RuntimeRowSelectionMode::None => DataGridSelectionMode::None,
                        RuntimeRowSelectionMode::Single => DataGridSelectionMode::Single,
                        RuntimeRowSelectionMode::Multiple => DataGridSelectionMode::Multiple,
                    })
                    .selected_keys(selected_keys.signal())
                    .on_selection_change(ValueCommand::new(
                        |view_model: &mut RuntimeInteractionBenchmarkVm,
                         change: DataGridSelectionChange| {
                            view_model.selection_dispatches += 1;
                            view_model.selected_keys.set(change.selected_keys);
                        },
                    ));
            } else {
                grid = grid.selection_mode(DataGridSelectionMode::None);
            }
            WidgetTree::new(grid)
        }
    }
}

fn row_hover_point(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    kind: RuntimeRowHoverKind,
    row_index: usize,
) -> Result<(WidgetId, Point), String> {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| {
            let matches = match (&region.interaction, kind) {
                (HitInteraction::ListItem { state, .. }, RuntimeRowHoverKind::List) => {
                    state.item_index == row_index
                }
                (HitInteraction::TreeNode { state, .. }, RuntimeRowHoverKind::Tree) => {
                    state.node_index == row_index
                }
                _ => false,
            };
            if !matches {
                return None;
            }
            let visible = region
                .clip_rect
                .and_then(|clip| region.rect.intersect(clip))
                .unwrap_or(region.rect);
            (visible.width > Dp::ZERO && visible.height > Dp::ZERO).then_some((
                match region.interaction.target_id() {
                    crate::ui::widget::HitTargetId::Widget(id) => id,
                    _ => return None,
                },
                Point::new(
                    visible.x + visible.width * 0.75,
                    visible.y + visible.height * 0.5,
                ),
            ))
        })
        .ok_or_else(|| format!("{kind:?} row {row_index} has no visible hit region"))
}

fn button_hover_point(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    button_id: WidgetId,
) -> Result<Point, String> {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::Widget { id, .. } if *id == button_id => {
                let visible = region
                    .clip_rect
                    .and_then(|clip| region.rect.intersect(clip))
                    .unwrap_or(region.rect);
                (visible.width > Dp::ZERO && visible.height > Dp::ZERO).then_some(Point::new(
                    visible.x + visible.width * 0.5,
                    visible.y + visible.height * 0.5,
                ))
            }
            _ => None,
        })
        .ok_or_else(|| format!("button {button_id:?} has no visible hit region"))
}

fn data_grid_cell_point(
    handler: &mut BoundRuntimeHandler<RuntimeInteractionBenchmarkVm>,
    row_index: usize,
    column_key: &str,
) -> Result<(WidgetId, Point), String> {
    let column_key = WidgetKey::from(column_key);
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { state, .. }
                if state.row_index == row_index && state.column_key == column_key =>
            {
                let visible = region
                    .clip_rect
                    .and_then(|clip| region.rect.intersect(clip))
                    .unwrap_or(region.rect);
                (visible.width > Dp::ZERO && visible.height > Dp::ZERO).then_some((
                    state.row_id,
                    Point::new(
                        visible.x + visible.width * 0.5,
                        visible.y + visible.height * 0.5,
                    ),
                ))
            }
            _ => None,
        })
        .ok_or_else(|| {
            format!("DataGrid row {row_index} column {column_key:?} has no visible hit region")
        })
}
