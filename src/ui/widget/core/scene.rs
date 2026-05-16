use std::collections::HashMap;

use taffy::prelude::TaffyTree;

use crate::animation::AnimationEngine;
use crate::foundation::binding::DependencyGraph;
use crate::media::MediaManager;
use crate::text::font::{FontManager, TextLayoutInfo};
use crate::ui::theme::Theme;
use crate::ui::unit::UnitContext;
use crate::ui::widget::common::{
    ClipMask, ComputedScene, LifecycleEventState, MeasureContext, Point, Rect, ScrollbarHandle,
    TextEditState, WidgetId, WidgetStateMap,
};

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
    pub(crate) viewport: Rect,
    pub(crate) units: UnitContext,
    pub(crate) animations: &'b mut AnimationEngine,
    pub(crate) now: std::time::Instant,
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
}

#[derive(Clone)]
pub(crate) struct SceneChunkParts<VM> {
    pub(crate) before_children: ComputedScene<VM>,
    pub(crate) after_children: ComputedScene<VM>,
}
