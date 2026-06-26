use super::*;

use crate::platform::event::MouseButton;
use crate::ui::widget::{
    DataGrid, DataGridCellContext, DataGridCellEditCommit, DataGridColumn,
    DataGridColumnReorderEvent, DataGridColumnWidthChange, DataGridRow, DataGridSelectionChange,
    DataGridSelectionMode, DataGridSort, DataGridSortChange, DataGridSortDirection, MenuItem,
    WidgetKey,
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
