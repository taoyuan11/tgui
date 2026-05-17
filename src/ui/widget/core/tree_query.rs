use super::*;
#[cfg(test)]
use crate::ui::widget::common::ChildSource;

impl<VM> WidgetTree<VM> {
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
        with_widget_stack(|| {
            let mut states = Vec::new();
            self.root
                .resolve(theme)
                .collect_media_event_states(media, &mut states);
            states
        })
    }

    pub(crate) fn lifecycle_event_states(&self, theme: &Theme) -> Vec<LifecycleEventState<VM>> {
        with_widget_stack(|| {
            let mut states = Vec::new();
            self.root
                .resolve(theme)
                .collect_lifecycle_event_states(&mut states);
            states
        })
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        viewport: Rect,
        scene_position: Point,
    ) -> Option<super::CanvasSceneHit> {
        self.query_canvas_scene_all_at_widget(
            widget_id,
            font_manager,
            theme,
            media,
            units,
            animations,
            viewport,
            scene_position,
        )
        .into_iter()
        .next()
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_all_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        viewport: Rect,
        scene_position: Point,
    ) -> Vec<super::CanvasSceneHit> {
        let layout =
            self.build_scene_layout(font_manager, theme, media, animations, units, viewport);
        layout.query_canvas_scene_all_at_widget(widget_id, font_manager, units, scene_position)
    }
}

#[cfg(test)]
pub(super) fn element_has_lifecycle_handlers<VM>(element: &Element<VM>) -> bool {
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

#[cfg(test)]
fn child_source_has_lifecycle_handlers<VM>(source: &ChildSource<VM>) -> bool {
    source.resolve(None).iter().any(element_has_lifecycle_handlers)
}
