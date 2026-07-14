use super::*;
#[cfg(test)]
use crate::ui::widget::common::ChildSource;

impl<VM: 'static> WidgetTree<VM> {
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
            false,
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
        reduced_motion: bool,
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
            reduced_motion,
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
        reduced_motion: bool,
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
            reduced_motion,
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
        reduced_motion: bool,
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
            reduced_motion,
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
    ) -> HitPath<VM> {
        Self::hit_path_from_computed_impl(computed, point, true)
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn hit_path_from_computed_full_scan(
        computed: &ComputedScene<VM>,
        point: Point,
    ) -> HitPath<VM> {
        Self::hit_path_from_computed_impl(computed, point, false)
    }

    fn hit_path_from_computed_impl(
        computed: &ComputedScene<VM>,
        point: Point,
        allow_index: bool,
    ) -> HitPath<VM> {
        let mut path = HitPath::<VM>::new();
        let mut ids = smallvec::SmallVec::<[HitTargetId; 8]>::new();

        if computed.transform_records.is_empty() {
            if let Some(index) = allow_index.then(|| computed.hit_test_index()).flatten() {
                index.for_each_normal_candidate(point.y, |hit_index| {
                    let hit = &computed.hit_regions[hit_index];
                    if hit.contains_without_transform(point) {
                        push_hit_interaction(&mut path, &mut ids, hit.interaction.clone());
                    }
                });
                // Overlay regions are a separate, later stream. Keeping this second pass is
                // required for the same normal -> overlay z-order and occluder semantics as the
                // original chained iterator.
                index.for_each_overlay_candidate(point.y, |hit_index| {
                    let hit = &computed.overlay_hit_regions[hit_index];
                    if hit.contains_without_transform(point) {
                        push_hit_interaction(&mut path, &mut ids, hit.interaction.clone());
                    }
                });
                return path;
            }

            for hit in computed
                .hit_regions
                .iter()
                .chain(computed.overlay_hit_regions.iter())
                .filter(|hit| hit.contains_without_transform(point))
            {
                push_hit_interaction(&mut path, &mut ids, hit.interaction.clone());
            }

            return path;
        }

        for (hit, delta) in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter_map(|hit| {
                hit.hit_delta_if_contains(point, &computed.transform_records)
                    .map(|delta| (hit, delta))
            })
        {
            push_hit_interaction(&mut path, &mut ids, hit.interaction_translated(delta));
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
            false,
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
        reduced_motion: bool,
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
            reduced_motion,
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
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> HitPath<VM> {
        let Some(point) = cursor_position else {
            return HitPath::new();
        };
        let computed = self.compute_scene_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            reduced_motion,
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
    ) -> HitPath<VM> {
        self.hit_path_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            false,
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

    #[cfg(feature = "video")]
    pub(crate) fn video_controllers(&self, theme: &Theme) -> Vec<crate::video::VideoController> {
        with_widget_stack(|| {
            let mut controllers = Vec::new();
            self.root
                .resolve(theme)
                .collect_video_controllers(&mut controllers);
            controllers
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
        let layout = self.build_scene_layout(
            font_manager,
            theme,
            media,
            animations,
            units,
            &HashMap::new(),
            &HashMap::new(),
            viewport,
        );
        layout.query_canvas_scene_all_at_widget(widget_id, font_manager, units, scene_position)
    }
}

fn push_hit_interaction<VM>(
    path: &mut HitPath<VM>,
    ids: &mut smallvec::SmallVec<[HitTargetId; 8]>,
    interaction: HitInteraction<VM>,
) {
    let id = interaction.target_id();

    if matches!(interaction, HitInteraction::Occluder { .. }) {
        ids.clear();
        path.clear();
    }

    if let Some(index) = ids.iter().position(|existing| *existing == id) {
        path[index] = interaction;
    } else {
        ids.push(id);
        path.push(interaction);
    }
}

#[cfg(test)]
mod hit_test_index_tests {
    use super::*;
    use crate::ui::widget::common::{DefaultActivation, TransformRecord};

    fn widget(id: u64) -> HitInteraction<()> {
        HitInteraction::Widget {
            id: WidgetId::from_raw(id),
            interactions: InteractionHandlers::default(),
            focusable: false,
            default_activation: DefaultActivation::None,
        }
    }

    fn region(rect: Rect, geometry: HitGeometry, interaction: HitInteraction<()>) -> HitRegion<()> {
        HitRegion {
            rect,
            clip_rect: None,
            geometry,
            transform_chain: smallvec::SmallVec::new(),
            scope_path: Vec::new(),
            focus: None,
            interaction,
            gpu_scroll_container: None,
        }
    }

    fn fingerprint(path: &HitPath<()>) -> Vec<(u8, HitTargetId)> {
        path.iter()
            .map(|interaction| {
                let kind = match interaction {
                    HitInteraction::Occluder { .. } => 0,
                    HitInteraction::Disabled { .. } => 1,
                    HitInteraction::Widget { .. } => 2,
                    HitInteraction::SelectableText { .. } => 3,
                    HitInteraction::Switch { .. } => 4,
                    HitInteraction::Checkbox { .. } => 5,
                    HitInteraction::Radio { .. } => 6,
                    HitInteraction::SelectTrigger { .. } => 7,
                    HitInteraction::TabTrigger { .. } => 8,
                    HitInteraction::ListItem { .. } => 9,
                    HitInteraction::TreeNode { .. } => 10,
                    HitInteraction::TreeDisclosure { .. } => 11,
                    HitInteraction::TreeCheckbox { .. } => 12,
                    HitInteraction::DataGridCell { .. } => 13,
                    HitInteraction::DataGridHeader { .. } => 14,
                    HitInteraction::DataGridResizeHandle { .. } => 15,
                    HitInteraction::SplitterHandle { .. } => 16,
                    HitInteraction::Slider { .. } => 17,
                    HitInteraction::TextInput { .. } => 18,
                    HitInteraction::SelectOption { .. } => 19,
                    HitInteraction::CanvasItem { .. } => 20,
                };
                (kind, interaction.target_id())
            })
            .collect()
    }

    fn assert_index_matches_full_scan(scene: &ComputedScene<()>, point: Point) {
        let indexed = WidgetTree::hit_path_from_computed(scene, point);
        let full = WidgetTree::hit_path_from_computed_full_scan(scene, point);
        assert_eq!(fingerprint(&indexed), fingerprint(&full), "point={point:?}");
    }

    #[test]
    fn spatial_hit_index_matches_full_scan_for_cells_clips_geometry_and_overlays() {
        let mut scene = ComputedScene::<()>::default();
        for index in 0..96_u64 {
            scene.hit_regions.push(region(
                Rect::new(10.0, index as f32 * 33.0, 120.0, 24.0),
                HitGeometry::Rect,
                widget(index + 1),
            ));
        }

        // Crosses a cell boundary and exercises exact-boundary inclusive Rect::contains.
        scene.hit_regions.push(region(
            Rect::new(0.0, 63.5, 200.0, 1.0),
            HitGeometry::Rect,
            widget(200),
        ));
        // A very tall region uses the global stream and must merge back into original order.
        scene.hit_regions.insert(
            12,
            region(
                Rect::new(0.0, -100.0, 220.0, 5_000.0),
                HitGeometry::Rect,
                widget(201),
            ),
        );

        let mut clipped = region(
            Rect::new(0.0, 120.0, 200.0, 80.0),
            HitGeometry::Rect,
            widget(202),
        );
        clipped.clip_rect = Some(Rect::new(0.0, 140.0, 200.0, 20.0));
        scene.hit_regions.push(clipped);
        scene.hit_regions.push(region(
            Rect::new(0.0, 220.0, 100.0, 100.0),
            HitGeometry::Quad([
                Point::new(0.0, 220.0),
                Point::new(100.0, 220.0),
                Point::new(80.0, 320.0),
                Point::new(20.0, 320.0),
            ]),
            widget(203),
        ));
        scene.hit_regions.push(region(
            Rect::new(0.0, 340.0, 100.0, 100.0),
            HitGeometry::Triangles(Arc::from([[
                Point::new(0.0, 340.0),
                Point::new(100.0, 340.0),
                Point::new(50.0, 440.0),
            ]])),
            widget(204),
        ));

        // Overlay is visited after normal hits. The occluder clears the earlier path, then the
        // duplicate id replaces it with Disabled exactly as the full chained scan does.
        scene.overlay_hit_regions.push(region(
            Rect::new(0.0, 0.0, 10_000.0, 10_000.0),
            HitGeometry::Rect,
            HitInteraction::Occluder {
                id: WidgetId::from_raw(300),
            },
        ));
        scene.overlay_hit_regions.push(region(
            Rect::new(0.0, 0.0, 10_000.0, 10_000.0),
            HitGeometry::Rect,
            widget(301),
        ));
        scene.overlay_hit_regions.push(region(
            Rect::new(0.0, 0.0, 10_000.0, 10_000.0),
            HitGeometry::Rect,
            HitInteraction::Disabled {
                id: WidgetId::from_raw(301),
            },
        ));

        for point in [
            Point::new(20.0, 0.0),
            Point::new(20.0, 63.5),
            Point::new(20.0, 64.0),
            Point::new(20.0, 64.5),
            Point::new(20.0, 130.0),
            Point::new(20.0, 150.0),
            Point::new(5.0, 250.0),
            Point::new(50.0, 250.0),
            Point::new(5.0, 400.0),
            Point::new(50.0, 400.0),
            Point::new(20.0, 3_500.0),
            Point::new(20.0, 20_000.0),
        ] {
            assert_index_matches_full_scan(&scene, point);
        }
    }

    #[test]
    fn transform_records_force_exact_full_scan_fallback() {
        let mut scene = ComputedScene::<()>::default();
        for index in 0..64_u64 {
            scene.hit_regions.push(region(
                Rect::new(0.0, index as f32 * 20.0, 100.0, 18.0),
                HitGeometry::Rect,
                widget(index + 1),
            ));
        }
        let transform_id = WidgetId::from_raw(500);
        scene.hit_regions[20].transform_chain.push(transform_id);
        scene.transform_records.insert(
            transform_id,
            TransformRecord {
                id: transform_id,
                base_offset: Point::ZERO,
                current_offset: Point::new(300.0, 50.0),
            },
        );

        assert!(scene.hit_test_index().is_none());
        assert_index_matches_full_scan(&scene, Point::new(320.0, 455.0));
    }

    #[test]
    fn hit_index_invalidation_rebuilds_after_in_place_bounds_change() {
        let mut scene = ComputedScene::<()>::default();
        for index in 0..64_u64 {
            scene.hit_regions.push(region(
                Rect::new(0.0, index as f32 * 80.0, 100.0, 30.0),
                HitGeometry::Rect,
                widget(index + 1),
            ));
        }
        assert_index_matches_full_scan(&scene, Point::new(10.0, 810.0));

        scene.hit_regions[10].rect.y = dp(4_000.0);
        scene.invalidate_hit_test_index();
        assert_index_matches_full_scan(&scene, Point::new(10.0, 810.0));
        assert_index_matches_full_scan(&scene, Point::new(10.0, 4_010.0));
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
    source
        .resolve(None)
        .iter()
        .any(element_has_lifecycle_handlers)
}
