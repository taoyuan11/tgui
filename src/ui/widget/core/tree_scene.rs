use super::*;
use std::time::Instant;

impl<VM> WidgetTree<VM> {
    #[cfg(test)]
    pub(crate) fn has_lifecycle_handlers(&self) -> bool {
        with_widget_stack(|| super::tree_query::element_has_lifecycle_handlers(&self.root))
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
        self.compute_scene_with_units_and_widget_state_at(
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
            Instant::now(),
        )
    }
}
