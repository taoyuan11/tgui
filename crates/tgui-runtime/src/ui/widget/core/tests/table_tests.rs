use super::*;

use crate::ui::widget::{
    DataGrid, DataGridColumn, DataGridColumnPin, DataGridDensity, DataGridRow, DataGridSection,
    DataGridSelectionMode, HitInteraction, ResolvedElement, WidgetId, WidgetKey,
};

fn resolved_children<'a, VM>(kind: &'a ResolvedWidgetKind<VM>) -> &'a [ResolvedElement<VM>] {
    match kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    }
}

fn subtree_has_data_grid_header<VM>(element: &ResolvedElement<VM>) -> bool {
    element.data_grid_header.is_some()
        || resolved_children(&element.kind)
            .iter()
            .any(subtree_has_data_grid_header)
}

fn columns<VM: 'static>() -> Vec<DataGridColumn<String, VM>> {
    vec![
        DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
            .width(dp(180.0))
            .sortable(true),
        DataGridColumn::new("role", "Role".to_string(), |ctx| {
            Text::new(format!("role {}", ctx.row_index)).into()
        })
        .width(dp(160.0))
        .sortable(true),
    ]
}

fn pinned_columns<VM: 'static>() -> Vec<DataGridColumn<String, VM>> {
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
        DataGridColumn::new("actions", "Actions".to_string(), |_ctx| {
            Text::new("Edit").into()
        })
        .width(dp(84.0))
        .pin(DataGridColumnPin::End),
    ]
}

#[test]
fn data_grid_resolves_header_and_virtualized_rows() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let rows = (0..1_000)
        .map(|index| DataGridRow::keyed(index, format!("Row {index}")))
        .collect::<Vec<_>>();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(rows, columns())
            .selection_mode(DataGridSelectionMode::Multiple)
            .size(dp(240.0), dp(180.0)),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 180.0),
    );

    assert!(layout.resolved_root.data_grid_root.is_some());
    let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    assert_eq!(children.len(), 2);
    assert!(
        resolved_children(&children[0].kind)
            .iter()
            .any(subtree_has_data_grid_header),
        "header cells should carry DataGrid header state"
    );
    let ResolvedWidgetKind::Virtual { children, .. } = &children[1].kind else {
        panic!("DataGrid body should use VirtualList");
    };
    assert!(children.len() < 20);
    assert!(
        children
            .iter()
            .flat_map(|row| resolved_children(&row.kind))
            .any(|cell| cell.data_grid_cell.is_some()),
        "visible row cells should carry DataGrid cell state"
    );
}

#[test]
fn data_grid_exposes_horizontal_scroll_bounds_for_wide_columns() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            columns(),
        )
        .selected_keys(vec![WidgetKey::from("a")])
        .size(dp(220.0), dp(120.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .scroll_regions
            .iter()
            .any(|region| region.content_bounds.width > region.content_viewport.width),
        "wide DataGrid columns should create horizontal scrollable content"
    );
}

#[test]
fn data_grid_defaults_render_readably_in_dark_theme() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::dark();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            columns(),
        )
        .size(dp(240.0), dp(120.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == theme.colors.surface),
        "DataGrid surface should follow dark theme defaults"
    );
    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == theme.colors.surface_low),
        "DataGrid header should follow dark theme defaults"
    );
    for label in ["Name", "Alpha"] {
        let text = rendered
            .primitives
            .texts
            .iter()
            .find(|text| text.content.as_ref() == label)
            .unwrap_or_else(|| panic!("expected DataGrid text {label:?} to render"));
        assert_eq!(text.color, theme.colors.on_surface);
    }
}

#[test]
fn data_grid_header_background_follows_current_theme() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::light();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            columns(),
        )
        .size(dp(240.0), dp(120.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == theme.colors.surface_low),
        "DataGrid header should follow the current theme background"
    );
}

#[test]
fn data_grid_supports_sections_empty_loading_and_density() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let section = DataGridSection::new(
        Text::new("Engineering"),
        vec![DataGridRow::keyed("a", "Ada".to_string())],
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::sections(vec![section], columns())
            .density(DataGridDensity::Compact)
            .size(dp(240.0), dp(120.0)),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 120.0),
    );
    let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let ResolvedWidgetKind::Virtual { children, .. } = &children[1].kind else {
        panic!("DataGrid body should use VirtualList");
    };
    assert!(
        children
            .iter()
            .any(|row| row.key == Some(WidgetKey::from("section-0"))),
        "section header should be part of the virtual rows"
    );
    let compact_row = children
        .iter()
        .find(|row| {
            resolved_children(&row.kind)
                .iter()
                .any(|cell| cell.data_grid_cell.is_some())
        })
        .expect("compact data row should resolve");
    assert_eq!(
        compact_row
            .layout
            .height
            .as_ref()
            .map(|value| value.resolve()),
        Some(crate::ui::layout::Length::Px(dp(32.0)))
    );

    let empty: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(Vec::<DataGridRow<String>>::new(), columns())
            .empty(Text::new("Nothing here")),
    );
    let empty_layout = empty.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 120.0),
    );
    assert!(
        empty_layout.resolved_root.data_grid_root.is_none(),
        "empty slot should replace the grid body"
    );

    let loading: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(vec![DataGridRow::keyed("a", "Ada".to_string())], columns())
            .loading(true)
            .loading_view(Text::new("Still loading")),
    );
    let loading_layout = loading.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 120.0),
    );
    assert!(
        loading_layout.resolved_root.data_grid_root.is_none(),
        "loading slot should replace the grid body"
    );
}

#[test]
fn data_grid_pinned_columns_counteract_horizontal_scroll() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            pinned_columns(),
        )
        .size(dp(240.0), dp(120.0)),
    );
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let initial_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container { children, .. } = &initial_layout.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let body_id: WidgetId = children[1].id;
    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(body_id, Point::new(dp(96.0), dp(0.0)));
    let scrolled_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &scroll_offsets,
        &HashMap::new(),
        viewport,
    );
    let widget_states = WidgetStateMap::default();
    let select_open_states = HashMap::new();
    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &scrolled_layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &select_open_states,
        &scroll_offsets,
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let mut start_cell = None;
    let mut middle_cell = None;
    let mut middle_cell_clip = None;
    let mut end_cell = None;
    let mut middle_header = None;
    let mut middle_header_clip = None;
    let mut end_header = None;
    for region in &computed.hit_regions {
        match &region.interaction {
            HitInteraction::DataGridCell { state, .. }
                if state.column_key == WidgetKey::from("id") =>
            {
                start_cell = Some(region.rect);
            }
            HitInteraction::DataGridCell { state, .. }
                if state.column_key == WidgetKey::from("name") =>
            {
                middle_cell = Some(region.rect);
                middle_cell_clip = region.clip_rect;
            }
            HitInteraction::DataGridCell { state, .. }
                if state.column_key == WidgetKey::from("actions") =>
            {
                end_cell = Some(region.rect);
            }
            HitInteraction::DataGridHeader { state, .. }
                if state.column_key == WidgetKey::from("name") =>
            {
                middle_header = Some(region.rect);
                middle_header_clip = region.clip_rect;
            }
            HitInteraction::DataGridHeader { state, .. }
                if state.column_key == WidgetKey::from("actions") =>
            {
                end_header = Some(region.rect);
            }
            _ => {}
        }
    }
    assert_eq!(start_cell.expect("start cell should exist").x, dp(0.0));
    assert!(
        middle_cell.expect("middle cell should exist").x < dp(0.0),
        "unpinned body cells should move left with horizontal scroll"
    );
    let end_cell = end_cell.expect("end cell should exist");
    let end_header = end_header.expect("end header should exist");
    assert_eq!(end_cell.x, dp(240.0 - 84.0));
    assert_eq!(end_header.x, end_cell.x);
    assert_eq!(end_header.width, end_cell.width);
    assert!(
        middle_header.expect("middle header should exist").x < dp(72.0),
        "header cells should synchronize with body horizontal scroll"
    );
    let middle_cell_clip = middle_cell_clip.expect("middle cell should have a clip rect");
    assert_eq!(middle_cell_clip.x, dp(72.0));
    assert_eq!(middle_cell_clip.right(), dp(240.0 - 84.0));
    let middle_header_clip = middle_header_clip.expect("middle header should have a clip rect");
    assert_eq!(middle_header_clip.x, dp(72.0));
    assert_eq!(middle_header_clip.right(), dp(240.0 - 84.0));
}

#[test]
fn data_grid_end_pinned_columns_keep_natural_position_without_overflow() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            pinned_columns(),
        )
        .size(dp(560.0), dp(120.0)),
    );
    let viewport = Rect::new(0.0, 0.0, 560.0, 120.0);
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let rendered = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let mut end_cell = None;
    let mut end_header = None;
    for region in &rendered.hit_regions {
        match &region.interaction {
            HitInteraction::DataGridCell { state, .. }
                if state.column_key == WidgetKey::from("actions") =>
            {
                end_cell = Some(region.rect);
            }
            HitInteraction::DataGridHeader { state, .. }
                if state.column_key == WidgetKey::from("actions") =>
            {
                end_header = Some(region.rect);
            }
            _ => {}
        }
    }

    let expected_x = dp(72.0 + 180.0 + 180.0);
    let end_cell = end_cell.expect("end cell should exist");
    let end_header = end_header.expect("end header should exist");
    assert_eq!(end_cell.x, expected_x);
    assert_eq!(end_header.x, expected_x);
    assert_eq!(end_header.width, end_cell.width);
    assert!(
        end_cell.x < viewport.right() - end_cell.width,
        "end-pinned columns should not be forced to the viewport edge when the grid does not overflow"
    );
}

#[test]
fn data_grid_does_not_paint_trailing_space_as_an_extra_column() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            pinned_columns(),
        )
        .selected_keys(vec![WidgetKey::from("a")])
        .size(dp(560.0), dp(120.0)),
    );

    let viewport = Rect::new(0.0, 0.0, 560.0, 120.0);
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let ResolvedWidgetKind::Virtual { children: rows, .. } = &children[1].kind else {
        panic!("DataGrid body should use VirtualList");
    };
    let row = rows
        .iter()
        .find(|row| {
            resolved_children(&row.kind)
                .iter()
                .any(|cell| cell.data_grid_cell.is_some())
        })
        .expect("data row should resolve");
    let expected_width = dp(72.0 + 180.0 + 180.0 + 84.0);
    assert_eq!(
        row.layout.width.as_ref().map(|value| value.resolve()),
        Some(crate::ui::layout::Length::Px(expected_width))
    );

    let rendered = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered.hit_regions.iter().all(|region| {
        !matches!(region.interaction, HitInteraction::DataGridCell { .. })
            || region.rect.right() <= expected_width
    }));
}
