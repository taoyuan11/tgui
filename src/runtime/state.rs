use crate::foundation::binding::{DependencyGraph, TextChange, TextChangeSet};
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::{FontWeight, TextLayoutInfo};
use crate::ui::unit::{Dp, UnitContext};
#[cfg(feature = "audio")]
use crate::ui::widget::LifecycleWidgetKind;
use crate::ui::widget::{
    CanvasDragEvent, CanvasItemId, CanvasMouseButton, CanvasMouseEvent, CanvasPointerEvent,
    CanvasWheelEvent, ComputedScene, LifecycleEventState, MediaEventPhase, MediaEventState, Point,
    Rect, ResolvedSceneLayout, SceneChunkParts, ScrollbarHandle, Text, VisualContextSnapshot,
    WidgetId,
};
use cosmic_text::Editor;
use ropey::Rope;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub(super) const SMOOTH_SCROLL_EPSILON: f32 = 0.1;
pub(super) const SMOOTH_SCROLL_LERP: f32 = 0.28;

pub(super) struct CachedScene<VM> {
    pub(super) viewport: Rect,
    pub(super) units: UnitContext,
    pub(super) focused_widget: Option<WidgetId>,
    pub(super) focus_visible: bool,
    pub(super) pressed_widget: Option<WidgetId>,
    pub(super) selected_text: Option<WidgetId>,
    pub(super) caret_visible: bool,
    pub(super) theme_epoch: u64,
    pub(super) animation_epoch: u64,
    pub(super) layout_animation_epoch: u64,
    pub(super) scroll_epoch: u64,
    pub(super) hover_epoch: u64,
    pub(super) text_input_epoch: u64,
    pub(super) hovered_scrollbar: Option<ScrollbarHandle>,
    pub(super) active_scrollbar: Option<ScrollbarHandle>,
    pub(super) computed_valid: bool,
    pub(super) layout: Option<ResolvedSceneLayout<VM>>,
    pub(super) computed: ComputedScene<VM>,
    pub(super) lifecycle_states: HashMap<WidgetId, LifecycleEventState<VM>>,
    pub(super) scene_chunks: HashMap<WidgetId, ComputedScene<VM>>,
    pub(super) scene_chunk_parts: HashMap<WidgetId, SceneChunkParts<VM>>,
    pub(super) visual_contexts: HashMap<WidgetId, VisualContextSnapshot>,
    pub(super) dependencies: DependencyGraph,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct TextInputSessionConfig {
    pub(super) font_family: Option<String>,
    pub(super) font_weight: FontWeight,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) width_bits: u32,
    pub(super) multiline: bool,
    pub(super) auto_wrap: bool,
}

#[derive(Clone, Debug)]
pub(super) struct TextInputBufferState {
    pub(super) external_value: String,
    pub(super) external_revision: u64,
    pub(super) current_text: String,
    pub(super) display_text: String,
    pub(super) rope: Rope,
    pub(super) editor: Editor<'static>,
    pub(super) config: Option<TextInputSessionConfig>,
    pub(super) layout_snapshot: Option<TextLayoutInfo>,
    pub(super) pending_changes: Vec<TextChange>,
    pub(super) pending_start_revision: Option<u64>,
}

impl TextInputBufferState {
    pub(super) fn new(editor: Editor<'static>, resolved_value: String, revision: u64) -> Self {
        Self {
            external_value: resolved_value.clone(),
            external_revision: revision,
            current_text: resolved_value.clone(),
            display_text: resolved_value.clone(),
            rope: Rope::from_str(&resolved_value),
            editor,
            config: None,
            layout_snapshot: None,
            pending_changes: Vec::new(),
            pending_start_revision: None,
        }
    }

    pub(super) fn current_text(&self) -> &str {
        &self.current_text
    }

    pub(super) fn has_unresolved_local_edits(&self) -> bool {
        self.current_text != self.external_value
    }

    pub(super) fn push_pending_change(&mut self, change: TextChange) {
        if self.pending_start_revision.is_none() {
            self.pending_start_revision = Some(self.external_revision);
        }
        self.pending_changes.push(change);
    }

    pub(super) fn take_pending_change_set(&mut self) -> Option<TextChangeSet> {
        let start_revision = self.pending_start_revision.take()?;
        Some(TextChangeSet {
            start_revision,
            end_revision: self.external_revision,
            changes: std::mem::take(&mut self.pending_changes),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HoverTargetId {
    Widget(WidgetId),
    SelectOption {
        widget_id: WidgetId,
        option_index: usize,
    },
    CanvasItem {
        widget_id: WidgetId,
        item_id: CanvasItemId,
    },
}

#[derive(Clone)]
pub(super) struct CanvasPointerContext {
    pub(super) item_id: CanvasItemId,
    pub(super) canvas_origin: Point,
    pub(super) item_origin: Point,
    pub(super) inverse_transform: [f32; 6],
    pub(super) text_hits: Arc<[crate::ui::widget::CanvasTextHitRegion]>,
}

impl CanvasPointerContext {
    pub(super) fn local_position(&self, position: Point) -> Point {
        let local = Point::new(
            position.x - self.item_origin.x,
            position.y - self.item_origin.y,
        );
        let [a, b, c, d, e, f] = self.inverse_transform;
        Point::new(
            a * local.x.get() + c * local.y.get() + e,
            b * local.x.get() + d * local.y.get() + f,
        )
    }

    pub(super) fn text_hit(&self, position: Point) -> Option<crate::ui::widget::CanvasTextHit> {
        self.text_hits
            .iter()
            .find(|entry| crate::ui::widget::HitGeometry::Quad(entry.quad).contains(position))
            .map(|entry| entry.hit)
    }

    pub(super) fn mouse_event(
        &self,
        position: Point,
        button: Option<CanvasMouseButton>,
    ) -> CanvasMouseEvent {
        CanvasMouseEvent {
            item_id: self.item_id,
            button,
            canvas_position: Point::new(
                position.x - self.canvas_origin.x,
                position.y - self.canvas_origin.y,
            ),
            scene_position: position,
            local_position: self.local_position(position),
            text_hit: self.text_hit(position),
        }
    }

    pub(super) fn pointer_event(&self, position: Point) -> CanvasPointerEvent {
        self.mouse_event(position, None)
    }

    pub(super) fn wheel_event(&self, position: Point, delta: Point) -> CanvasWheelEvent {
        let mouse = self.mouse_event(position, None);
        CanvasWheelEvent {
            item_id: mouse.item_id,
            delta,
            canvas_position: mouse.canvas_position,
            scene_position: mouse.scene_position,
            local_position: mouse.local_position,
            text_hit: None,
        }
    }

    pub(super) fn drag_event(
        &self,
        start_position: Point,
        position: Point,
        button: CanvasMouseButton,
    ) -> CanvasDragEvent {
        let start = self.mouse_event(start_position, Some(button));
        let current = self.mouse_event(position, Some(button));
        CanvasDragEvent {
            item_id: self.item_id,
            button,
            start_canvas_position: start.canvas_position,
            start_scene_position: start.scene_position,
            start_local_position: start.local_position,
            start_text_hit: start.text_hit,
            canvas_position: current.canvas_position,
            scene_position: current.scene_position,
            local_position: current.local_position,
            text_hit: current.text_hit,
            delta: Point::new(
                current.scene_position.x - start.scene_position.x,
                current.scene_position.y - start.scene_position.y,
            ),
        }
    }
}

#[derive(Clone)]
pub(super) enum ClickHandler<VM> {
    Command(Command<VM>),
    Toggle(ValueCommand<VM, bool>, bool),
    SelectOption {
        widget_id: WidgetId,
        command: Option<Command<VM>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
    },
    Canvas(
        ValueCommand<VM, CanvasMouseEvent>,
        CanvasPointerContext,
        Option<CanvasMouseButton>,
    ),
}

pub(super) struct PendingClick<VM> {
    pub(super) target_id: HoverTargetId,
    pub(super) deadline: Instant,
    pub(super) command: Option<ClickHandler<VM>>,
}

#[derive(Clone)]
pub(super) struct ActiveKeyRepeat {
    pub(super) event: crate::platform::event::KeyEvent,
    pub(super) next_fire_at: Instant,
}

pub(super) struct ActiveCanvasDrag<VM> {
    pub(super) button: CanvasMouseButton,
    pub(super) context: CanvasPointerContext,
    pub(super) start_position: Point,
    pub(super) started: bool,
    pub(super) on_mouse_up: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub(super) on_drag_start: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub(super) on_drag: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub(super) on_drag_end: Option<ValueCommand<VM, CanvasDragEvent>>,
}

#[derive(Clone)]
pub(super) struct SliderDrag<VM> {
    pub(super) widget_id: WidgetId,
    pub(super) on_change: Option<ValueCommand<VM, f32>>,
    pub(super) min: f32,
    pub(super) max: f32,
    pub(super) step: f32,
    pub(super) track_rect: Rect,
    pub(super) current_value: f32,
}

pub(super) struct FocusedWidget<VM> {
    pub(super) widget_id: WidgetId,
    pub(super) on_blur: Option<Command<VM>>,
}

#[derive(Clone)]
pub(super) enum HoverTransitionHandler<VM> {
    Command(Command<VM>),
    Canvas(ValueCommand<VM, CanvasPointerEvent>, CanvasPointerContext),
}

#[derive(Clone)]
pub(super) enum HoverMoveHandler<VM> {
    Point(ValueCommand<VM, Point>),
    Canvas(ValueCommand<VM, CanvasPointerEvent>, CanvasPointerContext),
}

pub(super) struct HoveredWidget<VM> {
    pub(super) target_id: HoverTargetId,
    pub(super) cursor_style: Option<crate::ui::widget::CursorStyle>,
    pub(super) on_mouse_enter: Option<HoverTransitionHandler<VM>>,
    pub(super) on_mouse_leave: Option<HoverTransitionHandler<VM>>,
    pub(super) on_mouse_move: Option<HoverMoveHandler<VM>>,
}

#[derive(Clone, Copy)]
pub(super) struct ScrollbarDrag {
    pub(super) handle: ScrollbarHandle,
    pub(super) start_cursor: Point,
    pub(super) start_scroll_offset: Point,
    pub(super) track: Rect,
    pub(super) thumb: Rect,
    pub(super) max_offset: Dp,
}

#[derive(Clone, Copy)]
pub(super) struct SmoothScrollState {
    pub(super) target: Point,
}

#[derive(Clone)]
pub(super) struct TextSelectionDrag {
    pub(super) widget_id: WidgetId,
    pub(super) frame: Rect,
    pub(super) padding: crate::ui::layout::Insets,
    pub(super) text_style: Text,
    pub(super) text: String,
    pub(super) multiline: bool,
    pub(super) auto_wrap: bool,
    pub(super) show_scrollbar: bool,
}

pub(super) enum PendingMediaEvent<VM> {
    Command(Command<VM>),
    Error(ValueCommand<VM, String>, String),
}

pub(super) enum PendingLifecycleEvent<VM> {
    Command(Command<VM>),
}

#[derive(Clone, Default)]
pub(super) struct DispatchedMediaState {
    pub(super) phase: Option<MediaEventPhase>,
}

#[derive(Clone)]
pub(super) struct DispatchedLifecycleState<VM> {
    pub(super) snapshot: crate::ui::widget::LifecycleSnapshot,
    pub(super) handlers: crate::ui::widget::LifecycleEventHandlers<VM>,
}

#[cfg(feature = "audio")]
#[derive(Clone, PartialEq, Eq)]
pub(super) struct AudioLifecycleState {
    pub(super) controller: crate::audio::AudioController,
    pub(super) autoplay: bool,
    pub(super) looping: bool,
}

#[cfg(feature = "audio")]
pub(super) fn audio_lifecycle_state(
    snapshot: &crate::ui::widget::LifecycleSnapshot,
) -> Option<AudioLifecycleState> {
    let LifecycleWidgetKind::Audio { audio } = &snapshot.kind else {
        return None;
    };
    Some(AudioLifecycleState {
        controller: audio.controller.clone(),
        autoplay: audio.autoplay.resolve(),
        looping: audio.looping.resolve(),
    })
}

pub(super) fn collect_pending_media_event<VM>(
    state: &MediaEventState<VM>,
    previous: Option<&DispatchedMediaState>,
    pending: &mut Vec<PendingMediaEvent<VM>>,
) {
    if previous.and_then(|value| value.phase.as_ref()) != state.media_phase.as_ref() {
        match state.media_phase.as_ref() {
            Some(MediaEventPhase::Loading) => {
                if let Some(command) = state.handlers.on_loading.clone() {
                    pending.push(PendingMediaEvent::Command(command));
                }
            }
            Some(MediaEventPhase::Success) => {
                if let Some(command) = state.handlers.on_success.clone() {
                    pending.push(PendingMediaEvent::Command(command));
                }
            }
            Some(MediaEventPhase::Error(error)) => {
                if let Some(command) = state.handlers.on_error.clone() {
                    pending.push(PendingMediaEvent::Error(command, error.clone()));
                }
            }
            _ => {}
        }
    }
}

pub(super) fn collect_pending_lifecycle_events<VM>(
    state: &LifecycleEventState<VM>,
    previous: Option<&DispatchedLifecycleState<VM>>,
    pending: &mut Vec<PendingLifecycleEvent<VM>>,
) {
    if previous.is_none() {
        if let Some(command) = state.handlers.on_mount.clone() {
            pending.push(PendingLifecycleEvent::Command(command));
        }
        return;
    }

    if state.snapshot
        != previous
            .expect("previous lifecycle state should exist")
            .snapshot
    {
        if let Some(command) = state.handlers.on_update.clone() {
            pending.push(PendingLifecycleEvent::Command(command));
        }
    }
}

#[derive(Default)]
pub(super) struct ClipboardService {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    inner: Option<arboard::Clipboard>,
}

impl ClipboardService {
    pub(super) fn get_text(&mut self) -> Option<String> {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = self.inner.as_mut() {
                return clipboard.get_text().ok();
            }
        }

        None
    }

    pub(super) fn set_text(&mut self, text: String) {
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            if let Some(clipboard) = self.inner.as_mut() {
                let _ = clipboard.set_text(text);
            }
        }
    }
}
