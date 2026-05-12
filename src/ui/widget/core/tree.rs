use super::*;
use crate::ui::widget::common::ChildSource;

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

impl<VM> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn has_lifecycle_handlers(&self) -> bool {
        with_widget_stack(|| element_has_lifecycle_handlers(&self.root))
    }
    #[allow(dead_code)]
    pub(crate) fn compute_scene(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    pub(crate) fn compute_scene_with_widget_state(
        &self,
        font_manager: &FontManager,
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
        self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            UnitContext::default(),
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
        )
    }

    #[allow(dead_code)]
    pub(crate) fn compute_scene_with_units(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    pub(crate) fn compute_scene_with_units_and_widget_state(
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
    ) -> ComputedScene<VM> {
        let layout =
            self.build_scene_layout(font_manager, theme, media, animations, units, viewport);
        self.collect_scene_from_layout(
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
        )
    }

    pub(crate) fn build_scene_layout(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        viewport: Rect,
    ) -> ResolvedSceneLayout<VM> {
        let (mut layout, dependencies) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let mut taffy = TaffyTree::new();
                let now = std::time::Instant::now();
                let resolved_root = self.root.resolve(theme);
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
                    source_root: self.root.clone(),
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
            None,
            None,
            None,
            selected_text,
            selected_text_state,
            caret_visible,
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
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> CollectedSceneCache<VM> {
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
                        now: std::time::Instant::now(),
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
        computed.dependencies = dependencies.clone();
        CollectedSceneCache {
            computed,
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
            dependencies,
        }
    }

    #[allow(dead_code)]
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
            selected_text,
            selected_text_state,
            caret_visible,
        )
        .computed
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn render_output(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    #[cfg(test)]
    pub(crate) fn render_output_with_widget_state(
        &self,
        font_manager: &FontManager,
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
    ) -> RenderedWidgetScene {
        self.render_output_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            UnitContext::default(),
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
        )
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn render_output_with_units(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    #[cfg(test)]
    pub(crate) fn render_output_with_units_and_widget_state(
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
    ) -> RenderedWidgetScene {
        let computed = self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
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
        );
        computed.rendered()
    }

    pub(crate) fn hit_path_from_computed(
        computed: &ComputedScene<VM>,
        point: Point,
    ) -> Vec<HitInteraction<VM>> {
        let mut path = Vec::new();
        let mut ids = Vec::new();

        for hit in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter(|hit| {
                hit.rect.contains(point)
                    && hit
                        .clip_rect
                        .map(|clip_rect| clip_rect.contains(point))
                        .unwrap_or(true)
                    && hit.geometry.contains(point)
            })
        {
            let id = hit.interaction.target_id();

            if let Some(index) = ids.iter().position(|existing| *existing == id) {
                path[index] = hit.interaction.clone();
            } else {
                ids.push(id);
                path.push(hit.interaction.clone());
            }
        }

        path
    }

    #[allow(dead_code)]
    pub(crate) fn hit_test(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Option<HitInteraction<VM>> {
        self.hit_test_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
    }

    pub(crate) fn hit_test_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Option<HitInteraction<VM>> {
        self.hit_path_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
        .pop()
    }

    pub(crate) fn hit_path_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Vec<HitInteraction<VM>> {
        let Some(point) = cursor_position else {
            return Vec::new();
        };
        let computed = self.compute_scene_with_widget_state(
            font_manager,
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
            None,
            None,
            None,
            false,
        );
        Self::hit_path_from_computed(&computed, point)
    }

    #[allow(dead_code)]
    pub(crate) fn hit_path(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Vec<HitInteraction<VM>> {
        self.hit_path_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
    }

    pub(crate) fn media_event_states(
        &self,
        media: &MediaManager,
        theme: &Theme,
    ) -> Vec<MediaEventState<VM>> {
        let mut states = Vec::new();
        self.root
            .resolve(theme)
            .collect_media_event_states(media, &mut states);
        states
    }

    pub(crate) fn lifecycle_event_states(&self, theme: &Theme) -> Vec<LifecycleEventState<VM>> {
        let mut states = Vec::new();
        self.root
            .resolve(theme)
            .collect_lifecycle_event_states(&mut states);
        states
    }
}

fn element_has_lifecycle_handlers<VM>(element: &Element<VM>) -> bool {
    if element.lifecycle_events.has_any() {
        return true;
    }

    match &element.kind {
        WidgetKind::Container { children, .. } => {
            children.iter().any(child_source_has_lifecycle_handlers)
        }
        _ => false,
    }
}

fn child_source_has_lifecycle_handlers<VM>(source: &ChildSource<VM>) -> bool {
    source
        .resolve(None)
        .iter()
        .any(element_has_lifecycle_handlers)
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
