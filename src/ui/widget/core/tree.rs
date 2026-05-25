use super::*;
use super::scene::ActiveTooltipState;
use crate::ui::widget::r#virtual::{
    apply_virtual_runtime_state_to_element, VirtualCacheState, VirtualViewportHint,
};
use std::time::Instant;

pub struct WidgetTree<VM> {
    pub(super) root: Element<VM>,
}

pub(super) fn with_widget_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        const WIDGET_STACK_SIZE: usize = 8 * 1024 * 1024;
        const WIDGET_STACK_RED_ZONE: usize = WIDGET_STACK_SIZE;
        return stacker::maybe_grow(WIDGET_STACK_RED_ZONE, WIDGET_STACK_SIZE, f);
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        f()
    }
}

impl<VM: 'static> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn compute_scene_with_units_and_widget_state_at(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
    ) -> ComputedScene<VM> {
        let layout = self.build_scene_layout_at(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            &HashMap::new(),
            viewport,
            now,
        );
        self.collect_scene_from_layout_at(
            font_manager,
            &layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
        )
    }

    pub(crate) fn build_scene_layout(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
    ) -> ResolvedSceneLayout<VM> {
        self.build_scene_layout_at(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            Instant::now(),
        )
    }

    pub(crate) fn build_scene_layout_at(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
        now: Instant,
    ) -> ResolvedSceneLayout<VM> {
        let (mut layout, dependencies) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let mut taffy = TaffyTree::new();
                let mut root = self.root.clone();
                apply_virtual_runtime_state_to_element(
                    &mut root,
                    scroll_offsets,
                    virtual_states,
                    VirtualViewportHint {
                        width: viewport.width,
                        height: viewport.height,
                    },
                );
                let resolved_root = root.resolve(theme);
                let root_layout = resolved_root
                    .build_layout_tree(
                        &mut taffy, animations, theme, units, None, viewport, true, now,
                    )
                    .expect("widget tree layout should build");
                taffy
                    .compute_layout_with_measure(
                        root_layout.node,
                        TaffySize {
                            width: AvailableSpace::Definite(viewport.width.get()),
                            height: AvailableSpace::Definite(viewport.height.get()),
                        },
                        |known_dimensions, _, _, node_context, _| {
                            measure_node(
                                node_context,
                                known_dimensions,
                                font_manager,
                                theme,
                                media,
                                units,
                            )
                        },
                    )
                    .expect("widget tree layout should compute");

                ResolvedSceneLayout {
                    source_root: root,
                    root_id: resolved_root.id,
                    resolved_root,
                    layout_root: root_layout,
                    taffy,
                    units,
                    dependencies: DependencyGraph::default(),
                    paths: HashMap::new(),
                    parents: HashMap::new(),
                    depths: HashMap::new(),
                }
            })
        });
        layout.dependencies = dependencies;
        layout.rebuild_indexes();
        layout
    }

    #[cfg(test)]
    pub(crate) fn collect_scene_from_layout(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.collect_scene_from_layout_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
        )
    }

    pub(crate) fn collect_scene_from_layout_at(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
    ) -> ComputedScene<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            None,
            None,
            None,
            None,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
            &HashMap::new(),
            None,
        )
        .computed
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
            tooltip_hover_started_at,
            active_tooltip,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_at(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
    ) -> CollectedSceneCache<VM> {
        let next_tooltip_wakeup: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
        let ((mut computed, lifecycle_states, chunks, chunk_parts, visual_contexts), dependencies) =
            with_widget_stack(|| {
                with_dependency_collection(|| {
                    let mut lifecycle_states = HashMap::new();
                    let mut chunks = HashMap::new();
                    let mut chunk_parts = HashMap::new();
                    let mut visual_contexts = HashMap::new();
                    let mut context = CollectContext {
                        taffy: &layout.taffy,
                        font_manager,
                        theme,
                        media,
                        focused_input,
                        focused_text_state,
                        focused_text_value,
                        focused_text_layout,
                        text_layout_overrides,
                        active_slider_value,
                        caret_visible,
                        selected_text,
                        selected_text_state,
                        hovered_scrollbar,
                        active_scrollbar,
                        widget_states,
                        select_open_states,
                        scroll_offsets,
                        viewport,
                        units: layout.units,
                        animations,
                        now,
                        focus: super::scene::FocusCollectState::default(),
                        tooltip_hover_started_at,
                        next_tooltip_wakeup: &next_tooltip_wakeup,
                        active_tooltip,
                    };
                    let computed = layout.resolved_root.collect_subtree_cache(
                        &layout.layout_root,
                        VisualContext {
                            origin: Point {
                                x: viewport.x,
                                y: viewport.y,
                            },
                            opacity: 1.0,
                            clip_rect: viewport,
                            clip_mask: None,
                        },
                        &mut context,
                        &mut lifecycle_states,
                        &mut chunks,
                        &mut chunk_parts,
                        &mut visual_contexts,
                    );
                    (
                        computed,
                        lifecycle_states,
                        chunks,
                        chunk_parts,
                        visual_contexts,
                    )
                })
            });
        computed.finalize_portals(viewport);
        computed.dependencies = dependencies.clone();
        CollectedSceneCache {
            computed,
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
            dependencies,
            next_tooltip_wakeup: next_tooltip_wakeup.get(),
        }
    }

    #[cfg(test)]
    pub(crate) fn collect_scene_from_layout_with_focus_value(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.collect_scene_cache_from_layout_with_focus_value(
            font_manager,
            layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            None,
            selected_text,
            selected_text_state,
            caret_visible,
            &HashMap::new(),
            None,
        )
        .computed
    }
}

pub enum WidgetCommand<VM> {
    Command(Command<VM>),
    Value(ValueCommand<VM, String>, String),
}

pub struct WidgetEventResult<VM> {
    pub command: Option<WidgetCommand<VM>>,
    pub focus: Option<WidgetId>,
    pub request_redraw: bool,
}

pub fn rect(x: Dp, y: Dp, width: Dp, height: Dp) -> Rect {
    Rect::new(x, y, width, height)
}
