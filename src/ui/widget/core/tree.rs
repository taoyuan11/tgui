use super::*;

pub struct WidgetTree<VM> {
    pub(super) root: Element<VM>,
}

impl<VM> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
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
            resolved_root,
            layout_root: root_layout,
            taffy,
            units,
        }
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
        self.collect_scene_from_layout_with_focus_value(
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
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

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
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        let mut computed = ComputedScene::default();
        let mut context = CollectContext {
            taffy: &layout.taffy,
            font_manager,
            theme,
            media,
            focused_input,
            focused_text_state,
            focused_text_value,
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
        layout.resolved_root.collect_primitives(
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
            &mut computed,
        );
        computed
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
