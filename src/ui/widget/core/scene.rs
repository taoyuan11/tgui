use std::cell::Cell;
use std::collections::HashMap;
use std::time::Instant;

use taffy::prelude::TaffyTree;

use crate::animation::AnimationEngine;
use crate::foundation::binding::DependencyGraph;
use crate::media::MediaManager;
use crate::text::font::{FontManager, TextLayoutInfo};
use crate::ui::theme::Theme;
use crate::ui::unit::UnitContext;
use crate::ui::widget::common::{
    ClipMask, ComputedScene, FocusScopeState, FocusTargetMeta, LifecycleEventState, MeasureContext,
    Point, Rect, ScrollbarHandle, TextEditState, WidgetId, WidgetStateMap,
};
use crate::ui::widget::VirtualCacheState;

#[derive(Clone, Default)]
pub(crate) struct FocusCollectState {
    pub(crate) scope_path: Vec<WidgetId>,
    pub(crate) next_order: usize,
    pub(crate) disabled_depth: usize,
}

pub(crate) struct CollectContext<'a, 'b> {
    pub(crate) taffy: &'a TaffyTree<MeasureContext>,
    pub(crate) font_manager: &'a FontManager,
    pub(crate) theme: &'a Theme,
    pub(crate) media: &'a MediaManager,
    pub(crate) focused_input: Option<WidgetId>,
    pub(crate) focused_text_state: Option<&'a TextEditState>,
    pub(crate) focused_text_value: Option<&'a str>,
    pub(crate) focused_text_layout: Option<&'a TextLayoutInfo>,
    pub(crate) text_layout_overrides:
        Option<&'a HashMap<WidgetId, super::TextInputLayoutOverride<'a>>>,
    pub(crate) active_slider_value: Option<(WidgetId, f32)>,
    pub(crate) caret_visible: bool,
    pub(crate) selected_text: Option<WidgetId>,
    pub(crate) selected_text_state: Option<&'a TextEditState>,
    pub(crate) hovered_scrollbar: Option<ScrollbarHandle>,
    pub(crate) active_scrollbar: Option<ScrollbarHandle>,
    pub(crate) widget_states: &'a WidgetStateMap,
    pub(crate) select_open_states: &'a HashMap<WidgetId, bool>,
    pub(crate) scroll_offsets: &'a HashMap<WidgetId, Point>,
    pub(crate) virtual_states: &'a HashMap<WidgetId, VirtualCacheState>,
    pub(crate) viewport: Rect,
    pub(crate) units: UnitContext,
    pub(crate) animations: &'b mut AnimationEngine,
    pub(crate) reduced_motion: bool,
    pub(crate) now: Instant,
    pub(crate) focus: FocusCollectState,
    /// runtime 维护：widget 进入 hover 的时间戳。emit_tooltip 据此判断 hover 是否已持续到 `delay`。
    pub(crate) tooltip_hover_started_at: &'a HashMap<WidgetId, Instant>,
    /// emit_tooltip 写入：尚未达到 delay 时记录下次该唤醒事件循环的时刻，
    /// runtime 会聚合到 `next_deadline` 并在到点后 invalidate scene 触发重 collect。
    pub(crate) next_tooltip_wakeup: &'a Cell<Option<Instant>>,
    /// emit_toast 写入：当前场景最早的 toast 过期时间。
    pub(crate) next_toast_wakeup: &'a Cell<Option<Instant>>,
    pub(crate) active_tooltip: Option<ActiveTooltipState>,
    pub(crate) active_hover_popover: Option<WidgetId>,
}

impl<'a, 'b> CollectContext<'a, 'b> {
    pub(crate) fn focus_scope_path(&self) -> Vec<WidgetId> {
        self.focus.scope_path.clone()
    }

    pub(crate) fn register_focus_scope(
        &mut self,
        computed: &mut ComputedScene<impl Sized>,
        scope_id: WidgetId,
        options: crate::ui::widget::FocusScopeOptions,
    ) -> Vec<WidgetId> {
        let mut path = self.focus.scope_path.clone();
        path.push(scope_id);
        let active = options.is_active();
        computed.register_focus_scope(FocusScopeState {
            scope_id,
            path: path.clone(),
            options,
            active,
        });
        path
    }

    pub(crate) fn next_focus_order(&mut self) -> usize {
        let order = self.focus.next_order;
        self.focus.next_order += 1;
        order
    }

    pub(crate) fn build_focus_meta<VM>(
        &mut self,
        widget_id: WidgetId,
        focus_state: &super::FocusState,
        interactions: &crate::ui::widget::InteractionHandlers<VM>,
        fallback_focusable: bool,
    ) -> Option<FocusTargetMeta<VM>> {
        let focusable = focus_state.focusable.unwrap_or(fallback_focusable);
        if !focusable || self.focus.disabled_depth > 0 {
            return None;
        }
        Some(FocusTargetMeta {
            widget_id,
            tab_index: focus_state.tab_index,
            order: self.next_focus_order(),
            scope_path: self.focus.scope_path.clone(),
            on_focus: interactions.on_focus.clone(),
            on_blur: interactions.on_blur.clone(),
        })
    }
}

pub(crate) struct TextInputLayoutOverride<'a> {
    pub(crate) revision: u64,
    pub(crate) text: &'a str,
    pub(crate) layout: &'a TextLayoutInfo,
}

#[derive(Clone, Copy)]
pub(crate) struct VisualContext {
    pub(crate) origin: Point,
    pub(crate) opacity: f32,
    pub(crate) clip_rect: Rect,
    pub(crate) clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy)]
pub(crate) struct VisualContextSnapshot {
    pub(crate) origin: Point,
    pub(crate) opacity: f32,
    pub(crate) clip_rect: Rect,
    pub(crate) clip_mask: Option<ClipMask>,
}

impl From<VisualContext> for VisualContextSnapshot {
    fn from(value: VisualContext) -> Self {
        Self {
            origin: value.origin,
            opacity: value.opacity,
            clip_rect: value.clip_rect,
            clip_mask: value.clip_mask,
        }
    }
}

impl From<VisualContextSnapshot> for VisualContext {
    fn from(value: VisualContextSnapshot) -> Self {
        Self {
            origin: value.origin,
            opacity: value.opacity,
            clip_rect: value.clip_rect,
            clip_mask: value.clip_mask,
        }
    }
}

#[derive(Clone)]
pub(crate) struct CollectedSceneCache<VM> {
    pub(crate) computed: ComputedScene<VM>,
    pub(crate) lifecycle_states: HashMap<WidgetId, LifecycleEventState<VM>>,
    pub(crate) chunks: HashMap<WidgetId, ComputedScene<VM>>,
    pub(crate) chunk_parts: HashMap<WidgetId, SceneChunkParts<VM>>,
    pub(crate) visual_contexts: HashMap<WidgetId, VisualContextSnapshot>,
    pub(crate) dependencies: DependencyGraph,
    /// 最近的 tooltip 唤醒时刻；runtime 据此把事件循环 WaitUntil 至此时间，
    /// 到点后 invalidate scene 再次 collect。若无 tooltip 在等待期内则为 `None`。
    pub(crate) next_tooltip_wakeup: Option<Instant>,
    /// 最近的 toast 唤醒时刻；runtime 到点后 invalidate scene 触发过期清理。
    pub(crate) next_toast_wakeup: Option<Instant>,
}

#[derive(Clone)]
pub(crate) struct SceneChunkParts<VM> {
    pub(crate) before_children: ComputedScene<VM>,
    pub(crate) after_children: ComputedScene<VM>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TooltipTrigger {
    Hover,
    Focus,
    LongPress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveTooltipState {
    pub(crate) widget_id: WidgetId,
    pub(crate) trigger: TooltipTrigger,
}
