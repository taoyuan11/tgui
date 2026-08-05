use super::*;

use crate::platform::event::MouseButton;
use crate::runtime::HoverTargetId;
use crate::ui::widget::{
    DataGrid, DataGridCellContext, DataGridCellEditCommit, DataGridColumn,
    DataGridColumnReorderEvent, DataGridColumnWidthChange, DataGridRow, DataGridSection,
    DataGridSelectionChange, DataGridSelectionMode, DataGridSort, DataGridSortChange,
    DataGridSortDirection, ItemLayout, List, ListItem, MenuItem, Tree, TreeNode, WidgetKey,
};

fn table_columns() -> Vec<DataGridColumn<&'static str, TestVm>> {
    vec![
        DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
            .width(dp(120.0))
            .min_width(dp(80.0))
            .max_width(dp(160.0))
            .sortable(true)
            .reorderable(false),
        DataGridColumn::new("role", "Role".to_string(), |ctx| {
            Text::new(format!("role {}", ctx.row_index)).into()
        })
        .width(dp(120.0))
        .sortable(true)
        .reorderable(false),
    ]
}

fn pinned_table_columns() -> Vec<DataGridColumn<&'static str, TestVm>> {
    vec![
        DataGridColumn::new("id", "ID".to_string(), |ctx| {
            Text::new(format!("#{}", ctx.row_index)).into()
        })
        .width(dp(72.0))
        .pin(DataGridColumnPin::Start),
        DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
            .width(dp(180.0)),
        DataGridColumn::new("role", "Role".to_string(), |ctx| {
            Text::new(format!("role {}", ctx.row_index)).into()
        })
        .width(dp(180.0)),
        DataGridColumn::new("status", "Status".to_string(), |_ctx| {
            Text::new("Ready").into()
        })
        .width(dp(84.0))
        .pin(DataGridColumnPin::End),
    ]
}

fn cell_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    row_key: impl Into<WidgetKey>,
    column_key: impl Into<WidgetKey>,
) -> (WidgetId, Point) {
    let row_key = row_key.into();
    let column_key = column_key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == row_key && state.column_key == column_key =>
            {
                let center = Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                );
                region
                    .clip_rect
                    .map(|clip| clip.contains(center))
                    .unwrap_or(true)
                    .then_some((*id, center))
            }
            _ => None,
        })
        .expect("requested data grid cell should be visible")
}

fn cell_visible_point(
    handler: &mut BoundRuntimeHandler<TestVm>,
    row_key: impl Into<WidgetKey>,
    column_key: impl Into<WidgetKey>,
) -> (WidgetId, Point) {
    let row_key = row_key.into();
    let column_key = column_key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { state, .. }
                if state.row_key == row_key && state.column_key == column_key =>
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
        .expect("requested data grid cell should have a visible fragment")
}

fn retained_row_visible_point(
    handler: &mut BoundRuntimeHandler<TestVm>,
    tree: bool,
    row_index: usize,
) -> Point {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| {
            let matches = match (&region.interaction, tree) {
                (HitInteraction::ListItem { state, .. }, false) => state.item_index == row_index,
                (HitInteraction::TreeNode { state, .. }, true) => state.node_index == row_index,
                _ => false,
            };
            if !matches {
                return None;
            }
            let visible = region
                .clip_rect
                .and_then(|clip| region.rect.intersect(clip))
                .unwrap_or(region.rect);
            Some(Point::new(
                visible.x + visible.width * 0.75,
                visible.y + visible.height * 0.5,
            ))
        })
        .expect("retained hover row should be visible")
}

fn header_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    column_key: impl Into<WidgetKey>,
) -> Point {
    let column_key = column_key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridHeader { state, .. } if state.column_key == column_key => {
                Some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ))
            }
            _ => None,
        })
        .expect("requested data grid header should be visible")
}

fn header_hit(
    handler: &mut BoundRuntimeHandler<TestVm>,
    column_key: impl Into<WidgetKey>,
) -> (Rect, Option<Rect>) {
    let column_key = column_key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridHeader { state, .. } if state.column_key == column_key => {
                Some((region.rect, region.clip_rect))
            }
            _ => None,
        })
        .expect("requested data grid header should be visible")
}

pub(super) fn assert_data_grid_scene_equivalent(
    actual: &crate::ui::widget::ComputedScene<TestVm>,
    expected: &crate::ui::widget::ComputedScene<TestVm>,
) {
    let assert_shapes = |actual: &[crate::ui::widget::RenderPrimitive],
                         expected: &[crate::ui::widget::RenderPrimitive]| {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.rect, expected.rect);
            assert_eq!(actual.color, expected.color);
            assert_eq!(actual.corner_radius, expected.corner_radius);
            assert_eq!(actual.stroke_width, expected.stroke_width);
            assert_eq!(actual.clip_rect, expected.clip_rect);
            assert_eq!(actual.clip_mask, expected.clip_mask);
        }
    };
    assert_eq!(actual.scene.backdrop_blurs, expected.scene.backdrop_blurs);
    assert_eq!(actual.scene.brushes, expected.scene.brushes);
    assert_eq!(
        actual.scene.canvas_composites.len(),
        expected.scene.canvas_composites.len()
    );
    assert_eq!(actual.scene.meshes.len(), expected.scene.meshes.len());
    assert_eq!(actual.scene.textures.len(), expected.scene.textures.len());
    assert_eq!(actual.scene.texts, expected.scene.texts);
    assert_eq!(
        actual.scene.text_decorations,
        expected.scene.text_decorations
    );
    assert_shapes(&actual.scene.shapes, &expected.scene.shapes);
    assert_shapes(&actual.scene.overlay_shapes, &expected.scene.overlay_shapes);
    assert_eq!(
        actual.scene.overlay_textures.len(),
        expected.scene.overlay_textures.len()
    );
    assert_eq!(actual.scene.overlay_texts, expected.scene.overlay_texts);
    assert_eq!(
        actual.scene.overlay_text_decorations,
        expected.scene.overlay_text_decorations
    );
    let command_kind = |command: &crate::ui::widget::RenderCommand| match command {
        crate::ui::widget::RenderCommand::BackdropBlur(_) => 0_u8,
        crate::ui::widget::RenderCommand::Brush(_) => 1,
        crate::ui::widget::RenderCommand::CanvasComposite(_) => 2,
        crate::ui::widget::RenderCommand::Shape(_) => 3,
        crate::ui::widget::RenderCommand::Texture(_) => 4,
        #[cfg(feature = "video")]
        crate::ui::widget::RenderCommand::VideoTexture(_) => 5,
        crate::ui::widget::RenderCommand::Text(_) => 6,
        crate::ui::widget::RenderCommand::TextDecoration(_) => 7,
        crate::ui::widget::RenderCommand::Mesh(_) => 8,
    };
    assert_eq!(
        actual
            .scene
            .commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>(),
        expected
            .scene
            .commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        actual
            .scene
            .overlay_commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>(),
        expected
            .scene
            .overlay_commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>()
    );
    assert_eq!(actual.hit_regions.len(), expected.hit_regions.len());
    for (actual, expected) in actual.hit_regions.iter().zip(expected.hit_regions.iter()) {
        assert_eq!(actual.rect, expected.rect);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert_eq!(actual.transform_chain, expected.transform_chain);
        assert_eq!(actual.scope_path, expected.scope_path);
        assert_eq!(actual.gpu_scroll_container, expected.gpu_scroll_container);
        assert_eq!(
            actual.interaction.target_id(),
            expected.interaction.target_id()
        );
    }
    assert_eq!(
        actual.overlay_hit_regions.len(),
        expected.overlay_hit_regions.len()
    );
    assert_eq!(actual.scroll_regions.len(), expected.scroll_regions.len());
    for (actual, expected) in actual
        .scroll_regions
        .iter()
        .zip(expected.scroll_regions.iter())
    {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.content_viewport, expected.content_viewport);
        assert_eq!(actual.visible_frame, expected.visible_frame);
        assert_eq!(actual.content_bounds, expected.content_bounds);
        assert_eq!(
            actual.gpu_base_scroll_offset,
            expected.gpu_base_scroll_offset
        );
        assert_eq!(actual.scroll_offset, expected.scroll_offset);
        assert_eq!(actual.overflow_x, expected.overflow_x);
        assert_eq!(actual.overflow_y, expected.overflow_y);
        assert_eq!(actual.horizontal_track, expected.horizontal_track);
        assert_eq!(actual.horizontal_thumb, expected.horizontal_thumb);
        assert_eq!(actual.vertical_track, expected.vertical_track);
        assert_eq!(actual.vertical_thumb, expected.vertical_thumb);
    }
    assert_eq!(actual.ime_cursor_area, expected.ime_cursor_area);
    assert_eq!(actual.focus_scopes, expected.focus_scopes);
    assert_eq!(actual.overlay_anchors, expected.overlay_anchors);
    assert_eq!(actual.transform_records, expected.transform_records);
    assert_eq!(
        actual.virtual_state_updates.len(),
        expected.virtual_state_updates.len()
    );
    for (actual, expected) in actual
        .virtual_state_updates
        .iter()
        .zip(expected.virtual_state_updates.iter())
    {
        assert_eq!(actual.widget_id, expected.widget_id);
        assert_eq!(actual.viewport_hint.width, expected.viewport_hint.width);
        assert_eq!(actual.viewport_hint.height, expected.viewport_hint.height);
        assert_eq!(actual.measured_extents, expected.measured_extents);
        assert_eq!(actual.measurement_signature, expected.measurement_signature);
        assert_eq!(actual.widget_ids_by_key, expected.widget_ids_by_key);
        assert_eq!(actual.invalidate_layout, expected.invalidate_layout);
    }
}

fn resize_handle_center(
    handler: &mut BoundRuntimeHandler<TestVm>,
    column_key: impl Into<WidgetKey>,
) -> Point {
    let column_key = column_key.into();
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridResizeHandle { state, .. }
                if state.column_key == column_key =>
            {
                Some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ))
            }
            _ => None,
        })
        .expect("requested data grid resize handle should be visible")
}

fn pointer_move(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
}

fn pointer_release(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let event_loop = TestEventLoop;
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
}

#[test]
fn table_horizontal_virtual_scroll_patch_updates_header_and_pinned_clips() {
    let invalidation = InvalidationSignal::new();
    let rows = (0..48)
        .map(|index| DataGridRow::keyed(format!("row-{index}"), "Alpha"))
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        DataGrid::new(rows, pinned_table_columns())
            .size(dp(240.0), dp(160.0))
            .row_height(dp(32.0))
            .overscan(3),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 160.0),
    );
    for _ in 0..3 {
        let _ = handler.computed_scene();
    }

    let (initial_name_header, _) = header_hit(&mut handler, "name");
    let scroll_id = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.can_scroll_x())
        .map(|region| region.id)
        .expect("DataGrid body should expose horizontal scroll");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();
    handler.set_scroll_offset(scroll_id, Point::new(dp(96.0), Dp::ZERO));
    let (scrolled_name_header, scrolled_name_clip) = header_hit(&mut handler, "name");

    assert_eq!(
        crate::runtime::scene_runtime::scroll_fast_path_probe::virtual_scene_hits(),
        1,
        "horizontal DataGrid body scroll should use the virtual scene patch path"
    );
    assert!(
        scrolled_name_header.x < initial_name_header.x,
        "header cells should move with horizontal body scroll during virtual scene patches"
    );
    assert_eq!(scrolled_name_header.x, dp(72.0 - 96.0));
    let scrolled_name_clip = scrolled_name_clip.expect("header should keep a clip rect");
    assert_eq!(scrolled_name_clip.x, dp(72.0));
    assert_eq!(scrolled_name_clip.right(), dp(240.0 - 84.0));

    handler.invalidate_computed_scene();
    let (full_name_header, full_name_clip) = header_hit(&mut handler, "name");
    assert_eq!(scrolled_name_header, full_name_header);
    assert_eq!(Some(scrolled_name_clip), full_name_clip);
}

#[test]
fn table_hover_stays_on_one_logical_row_across_pinned_cell_boundaries() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("row-0", "Alpha"),
                DataGridRow::keyed("row-1", "Beta"),
            ],
            pinned_table_columns(),
        )
        .size(dp(240.0), dp(160.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 160.0),
    );
    let viewport = handler.viewport_rect();
    let (start_row_id, start) = cell_visible_point(&mut handler, "row-0", "id");
    let (middle_row_id, middle) = cell_visible_point(&mut handler, "row-0", "name");
    let (end_row_id, end) = cell_visible_point(&mut handler, "row-0", "status");
    assert_eq!(start_row_id, middle_row_id);
    assert_eq!(middle_row_id, end_row_id);

    handler.cursor_position = Some(start);
    assert!(handler.handle_hover(viewport));
    assert!(handler
        .hovered_widgets
        .iter()
        .any(|hovered| hovered.target_id == HoverTargetId::Widget(start_row_id)));
    let row_hover_epoch = handler.hover_epoch;

    handler.cursor_position = Some(middle);
    let _ = handler.handle_hover(viewport);
    assert_eq!(handler.hover_epoch, row_hover_epoch);

    handler.cursor_position = Some(end);
    let _ = handler.handle_hover(viewport);
    assert_eq!(handler.hover_epoch, row_hover_epoch);

    let (next_row_id, next_row) = cell_visible_point(&mut handler, "row-1", "id");
    assert_ne!(next_row_id, start_row_id);
    handler.cursor_position = Some(next_row);
    assert!(handler.handle_hover(viewport));
    assert_eq!(handler.hover_epoch, row_hover_epoch.wrapping_add(1));
    assert!(handler
        .hovered_widgets
        .iter()
        .any(|hovered| hovered.target_id == HoverTargetId::Widget(next_row_id)));
}

#[test]
fn table_row_hover_two_row_patch_matches_full_scene_recollect() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("row-0", "Alpha"),
                DataGridRow::keyed("row-1", "Beta"),
            ],
            pinned_table_columns(),
        )
        .size(dp(240.0), dp(160.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 160.0),
    );
    let _ = handler.request_redraw_if_dirty(Instant::now());
    let viewport = handler.viewport_rect();
    let (_, first_row) = cell_visible_point(&mut handler, "row-0", "id");
    let (_, second_row) = cell_visible_point(&mut handler, "row-1", "id");

    handler.cursor_position = Some(first_row);
    assert!(handler.handle_hover(viewport));
    crate::runtime::scene_runtime::row_hover_patch_probe::reset();
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
        1
    );

    handler.cursor_position = Some(second_row);
    assert!(handler.handle_hover(viewport));
    let retained = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
        2
    );

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_data_grid_scene_equivalent(&retained, &full);
}

#[test]
fn table_row_hover_patch_rejects_common_signal_or_scroll_invalidation() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("row-0", "Alpha"),
                DataGridRow::keyed("row-1", "Beta"),
            ],
            pinned_table_columns(),
        )
        .size(dp(240.0), dp(160.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation.clone(),
        test_config_with_size(240.0, 160.0),
    );
    let _ = handler.request_redraw_if_dirty(Instant::now());
    let viewport = handler.viewport_rect();
    let (_, first_row) = cell_visible_point(&mut handler, "row-0", "id");
    let (_, second_row) = cell_visible_point(&mut handler, "row-1", "id");
    handler.cursor_position = Some(first_row);
    assert!(handler.handle_hover(viewport));
    let _ = handler.computed_scene();

    invalidation.mark_dirty();
    handler.cursor_position = Some(second_row);
    assert!(handler.handle_hover(viewport));
    assert!(handler.row_hover_patch_pending.is_none());
    crate::runtime::scene_runtime::row_hover_patch_probe::reset();
    let _ = handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
        0
    );

    let body = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.can_scroll_x())
        .map(|region| region.id)
        .expect("DataGrid body should scroll horizontally");
    handler.cursor_position = Some(first_row);
    assert!(handler.handle_hover(viewport));
    handler.set_scroll_offset(body, Point::new(dp(24.0), Dp::ZERO));
    crate::runtime::scene_runtime::row_hover_patch_probe::reset();
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
        0
    );
}

#[test]
fn list_and_tree_row_hover_patch_match_full_scene_recollect() {
    for tree_kind in [false, true] {
        let invalidation = InvalidationSignal::new();
        let item_layout = ItemLayout::Fixed {
            item_extent: dp(34.0),
            spacing: Dp::ZERO,
            overscan: 2,
        };
        let root: Element<TestVm> = if tree_kind {
            let nodes = (0..64)
                .map(|index| TreeNode::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            Tree::<usize, TestVm>::new(nodes, |context| {
                Text::new(format!("Tree {}", context.item)).into()
            })
            .item_layout(item_layout)
            .size(dp(240.0), dp(160.0))
            .into()
        } else {
            let items = (0..64)
                .map(|index| ListItem::keyed(format!("row-{index}"), index))
                .collect::<Vec<_>>();
            List::<usize, TestVm>::new(items, |context| {
                Text::new(format!("List {}", context.item)).into()
            })
            .item_layout(item_layout)
            .size(dp(240.0), dp(160.0))
            .into()
        };
        let mut config = test_config_with_size(240.0, 160.0);
        config.reduced_motion = true;
        let mut handler =
            test_handler_with_config(TestVm, Some(WidgetTree::new(root)), invalidation, config);
        let _ = handler.request_redraw_if_dirty(Instant::now());
        let viewport = handler.viewport_rect();
        let first = retained_row_visible_point(&mut handler, tree_kind, 0);
        let second = retained_row_visible_point(&mut handler, tree_kind, 1);
        handler.cursor_position = Some(first);
        assert!(handler.handle_hover(viewport));
        let _ = handler.computed_scene();

        crate::runtime::scene_runtime::row_hover_patch_probe::reset();
        handler.cursor_position = Some(second);
        assert!(handler.handle_hover(viewport));
        let retained = handler.computed_scene().clone();
        assert_eq!(
            crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
            1
        );
        handler.invalidate_computed_scene();
        let full = handler.computed_scene().clone();
        assert_data_grid_scene_equivalent(&retained, &full);
    }
}

#[test]
fn list_and_tree_row_hover_patch_reject_interactive_or_control_rows() {
    let item_layout = ItemLayout::Fixed {
        item_extent: dp(40.0),
        spacing: Dp::ZERO,
        overscan: 2,
    };
    let cases: Vec<(bool, Element<TestVm>)> = vec![
        (
            false,
            List::<i32, TestVm>::new(
                vec![ListItem::keyed("row-0", 0), ListItem::keyed("row-1", 1)],
                |context| Button::new(format!("Action {}", context.item)).into(),
            )
            .item_layout(item_layout)
            .size(dp(240.0), dp(120.0))
            .into(),
        ),
        (
            false,
            List::<i32, TestVm>::new(
                vec![ListItem::keyed("row-0", 0), ListItem::keyed("row-1", 1)],
                |context| Text::new(format!("Context {}", context.item)).into(),
            )
            .context_menu(vec![MenuItem::new("Open")])
            .item_layout(item_layout)
            .size(dp(240.0), dp(120.0))
            .into(),
        ),
        (
            true,
            Tree::<i32, TestVm>::new(
                vec![TreeNode::keyed("row-0", 0), TreeNode::keyed("row-1", 1)],
                |context| Text::new(format!("Check {}", context.item)).into(),
            )
            .checkable(true)
            .item_layout(item_layout)
            .size(dp(240.0), dp(120.0))
            .into(),
        ),
    ];
    for (tree_kind, root) in cases {
        let invalidation = InvalidationSignal::new();
        let mut handler = test_handler_with_config(
            TestVm,
            Some(WidgetTree::new(root)),
            invalidation,
            test_config_with_size(240.0, 120.0),
        );
        let _ = handler.request_redraw_if_dirty(Instant::now());
        let viewport = handler.viewport_rect();
        let first = retained_row_visible_point(&mut handler, tree_kind, 0);
        let second = retained_row_visible_point(&mut handler, tree_kind, 1);
        handler.cursor_position = Some(first);
        assert!(handler.handle_hover(viewport));
        let _ = handler.computed_scene();
        crate::runtime::scene_runtime::row_hover_patch_probe::reset();
        handler.cursor_position = Some(second);
        assert!(handler.handle_hover(viewport));
        let _ = handler.computed_scene();
        assert_eq!(
            crate::runtime::scene_runtime::row_hover_patch_probe::hits(),
            0
        );
    }
}

#[test]
fn table_header_click_dispatches_controlled_sort() {
    let invalidation = InvalidationSignal::new();
    let sort_state = Arc::new(Mutex::new(Vec::<DataGridSort>::new()));
    let sort_signal_source = Arc::clone(&sort_state);
    let sort_signal = Signal::new(
        move || sort_signal_source.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let sort_for_cmd = Arc::clone(&sort_state);
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("a", "Alpha"),
                DataGridRow::keyed("b", "Beta"),
            ],
            table_columns(),
        )
        .sort(sort_signal)
        .on_sort_change(ValueCommand::new(
            move |_vm: &mut TestVm, change: DataGridSortChange| {
                *sort_for_cmd.lock().unwrap() = change.sort;
            },
        ))
        .size(dp(240.0), dp(140.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 220.0),
    );
    let viewport = handler.viewport_rect();

    let name = header_center(&mut handler, "name");
    handler.cursor_position = Some(name);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        *sort_state.lock().unwrap(),
        vec![DataGridSort {
            column_key: WidgetKey::from("name"),
            direction: DataGridSortDirection::Ascending,
        }]
    );

    handler.modifiers = ModifiersState::SHIFT;
    let role = header_center(&mut handler, "role");
    handler.cursor_position = Some(role);
    handler.handle_mouse_press(
        viewport,
        Instant::now() + Duration::from_millis(700),
        CanvasMouseButton::Left,
    );
    handler.modifiers = ModifiersState::empty();
    assert_eq!(sort_state.lock().unwrap().len(), 2);
}

#[test]
fn table_keyboard_focus_skips_inert_headers_and_enter_sorts() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::<DataGridSort>::new()));
    let changes_for_command = Arc::clone(&changes);
    let columns: Vec<DataGridColumn<&'static str, TestVm>> = vec![
        DataGridColumn::new("plain", "Plain".to_string(), |ctx| {
            Text::new(ctx.row).into()
        }),
        DataGridColumn::new("sortable", "Sortable".to_string(), |ctx| {
            Text::new(ctx.row).into()
        })
        .sortable(true),
    ];
    let tree = WidgetTree::new(
        DataGrid::<&'static str, TestVm>::new(vec![DataGridRow::keyed("row", "Alpha")], columns)
            .on_sort_change(ValueCommand::new(move |_vm, change: DataGridSortChange| {
                *changes_for_command.lock().unwrap() = change.sort;
            }))
            .size(dp(260.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let focused = handler
        .focused_widget_id()
        .expect("a sortable header should receive focus");
    let focused_column =
        handler
            .computed_scene()
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::DataGridHeader { id, state, .. } if *id == focused => {
                    Some(state.column_key.clone())
                }
                _ => None,
            });
    assert_eq!(focused_column, Some(WidgetKey::from("sortable")));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(
        *changes.lock().unwrap(),
        vec![DataGridSort {
            column_key: WidgetKey::from("sortable"),
            direction: DataGridSortDirection::Ascending,
        }]
    );
}

#[test]
fn table_resize_handle_supports_keyboard_width_adjustment() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<DataGridColumnWidthChange>));
    let latest_for_command = Arc::clone(&latest);
    let columns: Vec<DataGridColumn<&'static str, TestVm>> =
        vec![
            DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
                .width(dp(120.0))
                .min_width(dp(80.0))
                .max_width(dp(160.0))
                .resizable(true),
        ];
    let tree = WidgetTree::new(
        DataGrid::<&'static str, TestVm>::new(vec![DataGridRow::keyed("row", "Alpha")], columns)
            .on_column_width_change(ValueCommand::new(move |_vm, change| {
                *latest_for_command.lock().unwrap() = Some(change);
            }))
            .size(dp(220.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let focused = handler
        .focused_widget_id()
        .expect("resize handle should receive focus");
    assert!(handler.computed_scene().hit_regions.iter().any(|region| {
        matches!(
            &region.interaction,
            HitInteraction::DataGridResizeHandle { id, .. } if *id == focused
        )
    }));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );

    let change = latest
        .lock()
        .unwrap()
        .clone()
        .expect("keyboard resize should dispatch");
    assert_eq!(change.column_key, WidgetKey::from("name"));
    assert_eq!(change.width, dp(128.0));
}

#[test]
fn table_selected_keys_signal_updates_cached_scene_and_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let selected = context.state(vec![WidgetKey::from("a")]);
    let selected_color = Color::hexa(0xD14B70FF);
    let row_color = Color::hexa(0x102030FF);
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |ctx: DataGridCellContext<&'static str>| {
            Text::new(format!("{} selected={}", ctx.row, ctx.selected)).into()
        },
    )
    .width(dp(180.0))];
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("a", "Alpha"),
                DataGridRow::keyed("b", "Beta"),
            ],
            columns,
        )
        .selected_keys(selected.signal())
        .row_height(dp(32.0))
        .style(move |style, _| {
            style.row_background = crate::ui::layout::Value::Static(row_color);
            style.zebra_background = crate::ui::layout::Value::Static(row_color);
            style.row_hover_background = crate::ui::layout::Value::Static(row_color);
            style.row_selected_background = crate::ui::layout::Value::Static(selected_color);
        })
        .size(dp(220.0), dp(120.0)),
    );
    let mut config = test_config_with_size(220.0, 120.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let selected_rects = |scene: &crate::ui::widget::ComputedScene<TestVm>| {
        scene
            .scene
            .shapes
            .iter()
            .filter_map(|shape| (shape.color == selected_color).then_some(shape.rect))
            .collect::<Vec<_>>()
    };
    let text_contents = |scene: &crate::ui::widget::ComputedScene<TestVm>| {
        scene
            .scene
            .texts
            .iter()
            .map(|text| text.content.to_string())
            .collect::<Vec<_>>()
    };

    let initial = handler.computed_scene().clone();
    assert_eq!(selected_rects(&initial).len(), 1);
    let initial_text = text_contents(&initial);
    assert!(initial_text
        .iter()
        .any(|text| text == "Alpha selected=true"));
    assert!(initial_text
        .iter()
        .any(|text| text == "Beta selected=false"));

    selected.set(vec![WidgetKey::from("b")]);
    handler.request_redraw_if_dirty(Instant::now());

    // Read the cache directly: calling computed_scene() here would hide a missed dependency by
    // allowing a deferred full rebuild to repair the stale frame.
    let (retained, retained_cache_dependencies) = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("selected-keys invalidation should preserve the cache shell");
        assert!(
            cached.layout_valid,
            "selected keys should update cached layout immediately"
        );
        assert!(
            cached.computed_valid,
            "selected keys should update cached scene immediately"
        );
        (
            cached.computed.clone(),
            (
                cached.dependencies.dependency_count(),
                cached.dependencies.has_global_dependency(),
                cached.dependencies.all_owners(),
            ),
        )
    };
    let retained_selected_rects = selected_rects(&retained);
    assert_eq!(retained_selected_rects.len(), 1);
    assert_ne!(
        retained_selected_rects,
        selected_rects(&initial),
        "the selected-row background should move from row a to row b"
    );
    let retained_text = text_contents(&retained);
    assert!(
        retained_text
            .iter()
            .any(|text| text == "Alpha selected=false"),
        "cached row a renderer should observe selected=false: {retained_text:?}"
    );
    assert!(
        retained_text
            .iter()
            .any(|text| text == "Beta selected=true"),
        "cached row b renderer should observe selected=true: {retained_text:?}"
    );

    let retained_b_cell = retained
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == WidgetKey::from("b") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .expect("selected row b should expose a cell hit region");
    let retained_accessibility = handler.accessibility_tree_update_for_test();
    let retained_b_node_id = crate::accessibility::node_id_from_widget(retained_b_cell);
    let retained_b_node = retained_accessibility
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == retained_b_node_id).then_some(node))
        .expect("selected row b should expose an accessibility node");
    assert_eq!(retained_b_node.is_selected(), Some(true));

    handler.invalidate_scene_with_reason("data_grid_selection_equivalence_full_recollect");
    let full = handler.computed_scene().clone();
    let full_cache_dependencies = {
        let cached = handler
            .cached_scene
            .as_ref()
            .expect("full recollect should repopulate the cache");
        (
            cached.dependencies.dependency_count(),
            cached.dependencies.has_global_dependency(),
            cached.dependencies.all_owners(),
        )
    };
    let full_accessibility = handler.accessibility_tree_update_for_test();

    assert_data_grid_scene_equivalent(&retained, &full);
    assert_eq!(retained_accessibility, full_accessibility);
    assert_eq!(retained_cache_dependencies, full_cache_dependencies);
    assert_eq!(
        (
            retained.dependencies.dependency_count(),
            retained.dependencies.has_global_dependency(),
            retained.dependencies.all_owners(),
        ),
        (
            full.dependencies.dependency_count(),
            full.dependencies.has_global_dependency(),
            full.dependencies.all_owners(),
        )
    );
}

#[test]
fn table_multiple_selection_uses_toggle_and_shift_anchor() {
    let invalidation = InvalidationSignal::new();
    let selected = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
    let selected_for_signal = Arc::clone(&selected);
    let selected_signal = Signal::new(
        move || selected_for_signal.lock().unwrap().clone(),
        invalidation.clone(),
    );
    let selected_for_cmd = Arc::clone(&selected);
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![
                DataGridRow::keyed("a", "Alpha"),
                DataGridRow::keyed("b", "Beta"),
                DataGridRow::keyed("c", "Gamma"),
                DataGridRow::keyed("d", "Delta"),
            ],
            table_columns(),
        )
        .selection_mode(DataGridSelectionMode::Multiple)
        .selected_keys(selected_signal)
        .on_selection_change(ValueCommand::new(
            move |_vm: &mut TestVm, change: DataGridSelectionChange| {
                *selected_for_cmd.lock().unwrap() = change.selected_keys;
            },
        ))
        .size(dp(240.0), dp(220.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 220.0),
    );
    let viewport = handler.viewport_rect();
    let (_, a) = cell_center(&mut handler, "a", "name");

    handler.cursor_position = Some(a);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(*selected.lock().unwrap(), vec![WidgetKey::from("a")]);

    let (_, c) = cell_center(&mut handler, "c", "name");
    handler.modifiers = primary_shortcut_modifier();
    handler.cursor_position = Some(c);
    handler.handle_mouse_press(
        viewport,
        Instant::now() + Duration::from_millis(700),
        CanvasMouseButton::Left,
    );
    handler.modifiers = ModifiersState::empty();
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        *selected.lock().unwrap(),
        vec![WidgetKey::from("a"), WidgetKey::from("c")]
    );

    let (_, d) = cell_center(&mut handler, "d", "name");
    handler.modifiers = ModifiersState::SHIFT;
    handler.cursor_position = Some(d);
    handler.handle_mouse_press(
        viewport,
        Instant::now() + Duration::from_millis(1400),
        CanvasMouseButton::Left,
    );
    handler.modifiers = ModifiersState::empty();
    assert_eq!(
        *selected.lock().unwrap(),
        vec![WidgetKey::from("c"), WidgetKey::from("d")]
    );
}

#[test]
fn table_shift_range_uses_live_disabled_signal() {
    for (initially_disabled, disabled_before_range, expected) in [
        (
            false,
            true,
            vec![WidgetKey::from("a"), WidgetKey::from("c")],
        ),
        (
            true,
            false,
            vec![
                WidgetKey::from("a"),
                WidgetKey::from("b"),
                WidgetKey::from("c"),
            ],
        ),
    ] {
        let invalidation = InvalidationSignal::new();
        let disabled = Arc::new(Mutex::new(initially_disabled));
        let disabled_for_signal = Arc::clone(&disabled);
        let disabled_signal = Signal::new(
            move || {
                *disabled_for_signal
                    .lock()
                    .expect("disabled lock should succeed")
            },
            invalidation.clone(),
        );
        let selected = Arc::new(Mutex::new(Vec::<WidgetKey>::new()));
        let selected_for_signal = Arc::clone(&selected);
        let selected_for_command = Arc::clone(&selected);
        let selected_signal = Signal::new(
            move || {
                selected_for_signal
                    .lock()
                    .expect("selected lock should succeed")
                    .clone()
            },
            invalidation.clone(),
        );
        let tree = WidgetTree::new(
            DataGrid::new(
                vec![
                    DataGridRow::keyed("a", "Alpha"),
                    DataGridRow::keyed("b", "Beta").disable(disabled_signal),
                    DataGridRow::keyed("c", "Gamma"),
                ],
                table_columns(),
            )
            .selection_mode(DataGridSelectionMode::Multiple)
            .selected_keys(selected_signal)
            .on_selection_change(ValueCommand::new(
                move |_vm: &mut TestVm, change: DataGridSelectionChange| {
                    *selected_for_command
                        .lock()
                        .expect("selected lock should succeed") = change.selected_keys;
                },
            ))
            .size(dp(240.0), dp(220.0)),
        );
        let mut handler = test_handler_with_config(
            TestVm,
            Some(tree),
            invalidation.clone(),
            test_config_with_size(240.0, 220.0),
        );
        let viewport = handler.viewport_rect();

        *disabled.lock().expect("disabled lock should succeed") = disabled_before_range;
        invalidation.mark_dirty();
        handler.request_redraw_if_dirty(Instant::now());

        let (_, row_a) = cell_center(&mut handler, "a", "name");
        handler.cursor_position = Some(row_a);
        handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

        let (_, row_c) = cell_center(&mut handler, "c", "name");
        handler.modifiers = ModifiersState::SHIFT;
        handler.cursor_position = Some(row_c);
        handler.handle_mouse_press(
            viewport,
            Instant::now() + Duration::from_millis(700),
            CanvasMouseButton::Left,
        );
        handler.modifiers = ModifiersState::empty();

        assert_eq!(
            *selected.lock().expect("selected lock should succeed"),
            expected,
            "range selection must observe disabled={disabled_before_range} after construction"
        );
    }
}

#[test]
fn table_column_resize_drag_dispatches_clamped_width() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<DataGridColumnWidthChange>));
    let latest_for_cmd = Arc::clone(&latest);
    let tree = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("a", "Alpha")], table_columns())
            .on_column_width_change(ValueCommand::new(move |_vm: &mut TestVm, change| {
                *latest_for_cmd.lock().unwrap() = Some(change);
            }))
            .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let start = resize_handle_center(&mut handler, "name");
    let target = Point::new(start.x + dp(100.0), start.y);

    handler.cursor_position = Some(start);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    pointer_move(&mut handler, target);
    pointer_release(&mut handler, target);

    let change = latest
        .lock()
        .unwrap()
        .clone()
        .expect("resize should dispatch");
    assert_eq!(change.column_key, WidgetKey::from("name"));
    assert_eq!(change.width, dp(160.0));
}

#[test]
fn table_column_reorder_dispatches_drop_target() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<DataGridColumnReorderEvent>));
    let latest_for_cmd = Arc::clone(&latest);
    let columns: Vec<DataGridColumn<&'static str, TestVm>> = vec![
        DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
            .width(dp(120.0))
            .sortable(false),
        DataGridColumn::new("role", "Role".to_string(), |ctx| {
            Text::new(format!("role {}", ctx.row_index)).into()
        })
        .width(dp(120.0))
        .sortable(false),
    ];
    let tree = WidgetTree::new(
        DataGrid::<&'static str, TestVm>::new(vec![DataGridRow::keyed("a", "Alpha")], columns)
            .on_column_reorder(ValueCommand::new(move |_vm: &mut TestVm, event| {
                *latest_for_cmd.lock().unwrap() = Some(event);
            }))
            .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let from = header_center(&mut handler, "name");
    let to = header_center(&mut handler, "role");

    handler.cursor_position = Some(from);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.cursor_position = Some(to);
    pointer_release(&mut handler, to);

    let event = latest
        .lock()
        .unwrap()
        .clone()
        .expect("reorder should dispatch");
    assert_eq!(event.from_index, 0);
    assert_eq!(event.to_index, 1);
    assert_eq!(event.column_key, WidgetKey::from("name"));
    assert_eq!(event.target_key, WidgetKey::from("role"));
}

#[test]
fn table_editable_cell_double_click_commits_text_value() {
    let invalidation = InvalidationSignal::new();
    let latest = Arc::new(Mutex::new(None::<DataGridCellEditCommit>));
    let latest_for_cmd = Arc::clone(&latest);
    let columns: Vec<DataGridColumn<&'static str, TestVm>> = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |ctx: DataGridCellContext<&'static str>| Text::new(ctx.row).into(),
    )
    .text_value(|row| row.to_string())
    .editable(true)];
    let tree = WidgetTree::new(
        DataGrid::<&'static str, TestVm>::new(vec![DataGridRow::keyed("a", "Alpha")], columns)
            .on_cell_edit_commit(ValueCommand::new(move |_vm: &mut TestVm, commit| {
                *latest_for_cmd.lock().unwrap() = Some(commit);
            }))
            .size(dp(180.0), dp(100.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (_, point) = cell_center(&mut handler, "a", "name");

    handler.cursor_position = Some(point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_mouse_press(
        viewport,
        Instant::now() + Duration::from_millis(80),
        CanvasMouseButton::Left,
    );

    let commit = latest.lock().unwrap().clone().expect("edit should commit");
    assert_eq!(commit.row_key, WidgetKey::from("a"));
    assert_eq!(commit.column_key, WidgetKey::from("name"));
    assert_eq!(commit.value, "Alpha");
}

#[test]
fn table_cell_context_menu_opens_on_right_click() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        DataGrid::new(vec![DataGridRow::keyed("a", "Alpha")], table_columns())
            .context_menu(vec![MenuItem::new("Rename"), MenuItem::new("Delete")])
            .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let (cell_id, point) = cell_center(&mut handler, "a", "name");

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    assert!(handler.context_menu_anchor_states.contains_key(&cell_id));
}

#[test]
fn table_live_disabled_row_blocks_context_menu_right_click_and_long_press() {
    let invalidation = InvalidationSignal::new();
    let disabled = Arc::new(Mutex::new(false));
    let disabled_for_signal = Arc::clone(&disabled);
    let disabled_signal = Signal::new(
        move || {
            *disabled_for_signal
                .lock()
                .expect("disabled lock should succeed")
        },
        invalidation.clone(),
    );
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha").disable(disabled_signal)],
            table_columns(),
        )
        .context_menu(vec![MenuItem::new("Rename")])
        .size(dp(240.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation.clone());
    let viewport = handler.viewport_rect();
    let (cell_id, point) = cell_center(&mut handler, "a", "name");

    *disabled.lock().expect("disabled lock should succeed") = true;
    invalidation.mark_dirty();

    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    assert!(
        !handler.context_menu_anchor_states.contains_key(&cell_id),
        "a data-grid row disabled after construction must reject a right-click context menu"
    );

    let long_press_started = Instant::now();
    handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    let _ = handler.drive_animations(
        &TestEventLoop,
        long_press_started + crate::runtime::LONG_PRESS_THRESHOLD + Duration::from_millis(10),
    );
    assert!(
        !handler.context_menu_anchor_states.contains_key(&cell_id),
        "a data-grid row disabled after construction must reject a long-press context menu"
    );
}

#[test]
fn focused_data_grid_cell_uses_live_disabled_state_for_keyboard_activation() {
    let invalidation = InvalidationSignal::new();
    let disabled = Arc::new(Mutex::new(false));
    let disabled_for_signal = Arc::clone(&disabled);
    let disabled_signal = Signal::new(
        move || {
            *disabled_for_signal
                .lock()
                .expect("disabled lock should succeed")
        },
        invalidation.clone(),
    );
    let edit_count = Arc::new(AtomicUsize::new(0));
    let action_count = Arc::new(AtomicUsize::new(0));
    let selection_count = Arc::new(AtomicUsize::new(0));
    let columns = vec![
        DataGridColumn::new(
            "edit",
            "Edit".to_string(),
            |ctx: DataGridCellContext<&'static str>| Text::new(ctx.row).into(),
        )
        .width(dp(100.0))
        .text_value(|row| row.to_string())
        .editable(true),
        DataGridColumn::new(
            "action",
            "Action".to_string(),
            |ctx: DataGridCellContext<&'static str>| Text::new(ctx.row).into(),
        )
        .width(dp(100.0)),
    ];
    let tree = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("row", "Alpha").disable(disabled_signal)],
            columns,
        )
        .selection_mode(DataGridSelectionMode::Multiple)
        .on_selection_change(ValueCommand::new({
            let selection_count = Arc::clone(&selection_count);
            move |_vm: &mut TestVm, _change| {
                selection_count.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .on_cell_action(ValueCommand::new({
            let action_count = Arc::clone(&action_count);
            move |_vm: &mut TestVm, _action| {
                action_count.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .on_cell_edit_commit(ValueCommand::new({
            let edit_count = Arc::clone(&edit_count);
            move |_vm: &mut TestVm, _commit| {
                edit_count.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .size(dp(200.0), dp(120.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation.clone());
    let (edit_id, _) = cell_center(&mut handler, "row", "edit");
    let (action_id, _) = cell_center(&mut handler, "row", "action");

    handler.focused_widget = Some(FocusedWidget {
        widget_id: edit_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter,))));
    handler.focused_widget = Some(FocusedWidget {
        widget_id: action_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter,))));
    handler.focused_widget = Some(FocusedWidget {
        widget_id: edit_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space,))));
    assert_eq!(edit_count.swap(0, Ordering::SeqCst), 1);
    assert_eq!(action_count.swap(0, Ordering::SeqCst), 1);
    assert_eq!(selection_count.swap(0, Ordering::SeqCst), 1);

    *disabled.lock().expect("disabled lock should succeed") = true;
    invalidation.mark_dirty();

    handler.focused_widget = Some(FocusedWidget {
        widget_id: edit_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter)));
    handler.focused_widget = Some(FocusedWidget {
        widget_id: action_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter)));
    handler.focused_widget = Some(FocusedWidget {
        widget_id: edit_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space)));

    assert_eq!(
        edit_count.load(Ordering::SeqCst),
        0,
        "Enter must not commit an edit after the focused row becomes disabled"
    );
    assert_eq!(
        action_count.load(Ordering::SeqCst),
        0,
        "Enter must not dispatch an action after the focused row becomes disabled"
    );
    assert_eq!(
        selection_count.load(Ordering::SeqCst),
        0,
        "Space must not change selection after the focused row becomes disabled"
    );
}

fn primary_shortcut_modifier() -> ModifiersState {
    #[cfg(target_os = "macos")]
    {
        crate::platform::keyboard::meta_modifier()
    }

    #[cfg(not(target_os = "macos"))]
    {
        ModifiersState::CONTROL
    }
}

#[test]
fn virtual_data_grid_arrow_down_keeps_column_across_materialized_boundary() {
    let invalidation = InvalidationSignal::new();
    let rows = (0..64)
        .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let columns: Vec<DataGridColumn<usize, TestVm>> = vec![
        DataGridColumn::new("name", "Name".to_string(), |context| {
            Text::new(format!("row {}", context.row)).into()
        })
        .width(dp(120.0)),
        DataGridColumn::new("role", "Role".to_string(), |context| {
            Text::new(format!("role {}", context.row)).into()
        })
        .width(dp(120.0)),
    ];
    let tree = WidgetTree::new(
        DataGrid::new(rows, columns)
            .row_height(dp(28.0))
            .overscan(0)
            .size(dp(240.0), dp(112.0)),
    );
    let mut config = test_config_with_size(240.0, 112.0);
    config.reduced_motion = true;
    let mut handler = test_handler_with_config(TestVm, Some(tree), invalidation, config);

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.column_key == WidgetKey::from("name") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .max_by_key(|(_, state, _)| state.virtual_row_index)
        .expect("virtual DataGrid should materialize at least one row");
    let next_index = state.row_index + 1;
    assert!(next_index < 64, "test must stop before the source end");
    let next_key = WidgetKey::from(format!("row-{next_index}"));
    let column_key = state.column_key.clone();
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((state.grid_id, state.row_key, column_key.clone()));
    handler.focus_visible = true;

    let arrow_down = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown));
    assert!(handler.handle_keyboard_input(&arrow_down));
    let _ = handler.computed_scene();
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&next_key, &column_key)),
        "ArrowDown must advance one row without falling through to the next column"
    );

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.column_key == WidgetKey::from("name") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .min_by_key(|(_, state, _)| state.virtual_row_index)
        .expect("scrolled DataGrid should materialize a first boundary row");
    let previous_key = state
        .row_index
        .checked_sub(1)
        .and_then(|index| state.selection.sibling_keys.get(index))
        .cloned()
        .expect("test must stop after the source start");
    let column_key = state.column_key.clone();
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((state.grid_id, state.row_key, column_key.clone()));

    let arrow_up = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowUp));
    assert!(handler.handle_keyboard_input(&arrow_up));
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&previous_key, &column_key)),
        "ArrowUp must cross the leading boundary without changing columns"
    );
}

#[test]
fn grouped_virtual_data_grid_crosses_section_materialized_boundary() {
    let invalidation = InvalidationSignal::new();
    let sections = vec![
        DataGridSection::new(
            Text::new("Section A"),
            (0..3)
                .map(|index| DataGridRow::keyed(format!("a-{index}"), index))
                .collect(),
        ),
        DataGridSection::new(
            Text::new("Section B"),
            (0..3)
                .map(|index| DataGridRow::keyed(format!("b-{index}"), index + 3))
                .collect(),
        ),
    ];
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: DataGridCellContext<usize>| Text::new(context.row.to_string()).into(),
    )
    .width(dp(160.0))];
    let tree = WidgetTree::new(
        DataGrid::sections(sections, columns)
            .row_height(dp(28.0))
            .overscan(0)
            .size(dp(200.0), dp(112.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(200.0, 112.0),
    );
    let body_id = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { state, .. } => Some(state.scroll_container_id),
            _ => None,
        })
        .expect("grouped DataGrid should materialize a cell");
    handler.set_scroll_offset(body_id, Point::new(Dp::ZERO, dp(56.0)));

    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == WidgetKey::from("a-2") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("last row before the section header should be materialized");
    let column_key = state.column_key.clone();
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((state.grid_id, state.row_key, column_key.clone()));

    let arrow_down = pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown));
    assert!(handler.handle_keyboard_input(&arrow_down));
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from("b-0"), &column_key))
    );
}

#[test]
fn virtual_data_grid_home_end_and_page_keep_column_across_full_source() {
    let invalidation = InvalidationSignal::new();
    let rows = (0..100)
        .map(|index| DataGridRow::keyed(format!("row-{index}"), index))
        .collect::<Vec<_>>();
    let columns: Vec<DataGridColumn<usize, TestVm>> = vec![
        DataGridColumn::new(
            "name",
            "Name".to_string(),
            |context: DataGridCellContext<usize>| Text::new(context.row.to_string()).into(),
        )
        .width(dp(120.0)),
        DataGridColumn::new(
            "role",
            "Role".to_string(),
            |context: DataGridCellContext<usize>| Text::new(format!("role {}", context.row)).into(),
        )
        .width(dp(120.0)),
    ];
    let tree = WidgetTree::new(
        DataGrid::new(rows, columns)
            .row_height(dp(28.0))
            .overscan(0)
            .size(dp(240.0), dp(124.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 124.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == WidgetKey::from("row-0")
                    && state.column_key == WidgetKey::from("role") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("first DataGrid role cell should be materialized");
    let grid_id = state.grid_id;
    let body_id = state.scroll_container_id;
    let column_key = state.column_key.clone();
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((grid_id, state.row_key, column_key.clone()));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from("row-99"), &column_key))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from("row-0"), &column_key))
    );

    let page = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == body_id)
        .map(|region| (region.content_viewport.height / dp(28.0)).ceil() as usize)
        .unwrap_or(1)
        .max(1);
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from(format!("row-{page}")), &column_key))
    );
    let page_offset = handler
        .scroll_states
        .get(&body_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    let viewport_height = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == body_id)
        .map(|region| region.content_viewport.height)
        .expect("DataGrid body should remain scrollable");
    assert!(
        (page_offset - viewport_height).abs() <= 0.01,
        "PageDown should advance one body viewport"
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageUp,))));
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from("row-0"), &column_key))
    );
}

#[test]
fn virtual_data_grid_page_long_disabled_run_is_conservative() {
    let invalidation = InvalidationSignal::new();
    let rows = (0..32)
        .map(|index| {
            let row = DataGridRow::keyed(format!("row-{index}"), index);
            if (1..=15).contains(&index) {
                row.disable(true)
            } else {
                row
            }
        })
        .collect::<Vec<_>>();
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: DataGridCellContext<usize>| Text::new(context.row.to_string()).into(),
    )
    .width(dp(160.0))];
    let tree = WidgetTree::new(
        DataGrid::new(rows, columns)
            .row_height(dp(28.0))
            .overscan(0)
            .size(dp(200.0), dp(96.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(200.0, 96.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == WidgetKey::from("row-0") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("first row should be materialized");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((state.grid_id, state.row_key, state.column_key.clone()));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, _)| row_key),
        Some(&WidgetKey::from("row-0"))
    );
    assert!(!handler
        .scroll_states
        .contains_key(&state.scroll_container_id));
}

#[test]
fn grouped_virtual_data_grid_page_skips_section_and_keeps_column() {
    let invalidation = InvalidationSignal::new();
    let sections = (0..2)
        .map(|section| {
            DataGridSection::new(
                Text::new(format!("Section {section}")),
                (0..8)
                    .map(|index| {
                        DataGridRow::keyed(format!("{section}-{index}"), section * 8 + index)
                    })
                    .collect(),
            )
        })
        .collect();
    let columns = vec![
        DataGridColumn::new(
            "name",
            "Name".to_string(),
            |context: DataGridCellContext<usize>| Text::new(context.row.to_string()).into(),
        )
        .width(dp(120.0)),
        DataGridColumn::new(
            "role",
            "Role".to_string(),
            |context: DataGridCellContext<usize>| Text::new(format!("role {}", context.row)).into(),
        )
        .width(dp(120.0)),
    ];
    let tree = WidgetTree::new(
        DataGrid::sections(sections, columns)
            .row_height(dp(28.0))
            .overscan(0)
            .size(dp(240.0), dp(264.0)),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(240.0, 264.0),
    );
    let (id, state, focus) = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| match &region.interaction {
            HitInteraction::DataGridCell { id, state, .. }
                if state.row_key == WidgetKey::from("0-0")
                    && state.column_key == WidgetKey::from("role") =>
            {
                Some((*id, state.clone(), region.focus.clone()?))
            }
            _ => None,
        })
        .expect("first grouped role cell should be materialized");
    let body_id = state.scroll_container_id;
    let column_key = state.column_key.clone();
    let viewport_height = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == body_id)
        .map(|region| region.content_viewport.height)
        .expect("grouped DataGrid body should scroll");
    handler.focused_widget = Some(FocusedWidget {
        widget_id: id,
        scope_path: focus.scope_path,
        on_blur: focus.on_blur,
    });
    handler.data_grid_focus_state = Some((state.grid_id, state.row_key, column_key.clone()));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    assert_eq!(
        handler
            .data_grid_focus_state
            .as_ref()
            .map(|(_, row_key, column_key)| (row_key, column_key)),
        Some((&WidgetKey::from("1-0"), &column_key)),
        "a page target landing on a section must choose the next logical row"
    );
    assert!((handler.scroll_states[&body_id].y - viewport_height).abs() <= 0.01);
}
