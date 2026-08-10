use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    CursorStyle, DataGrid, DataGridCellContext, DataGridColumn, DataGridColumnPin, DataGridDensity,
    DataGridRow, DataGridSection, DataGridSelectionMode, DataGridSort, DataGridSortDirection,
    DataGridStyle, HitInteraction, ItemLayout, ResolvedElement, WidgetId, WidgetKey,
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

fn subtree_container_padding<VM>(element: &ResolvedElement<VM>) -> Option<Value<Insets>> {
    if let ResolvedWidgetKind::Container { layout, .. } = &element.kind {
        if let Some(padding) = layout.padding.clone() {
            return Some(padding);
        }
    }
    resolved_children(&element.kind)
        .iter()
        .find_map(subtree_container_padding)
}

fn resolved_data_grid_header<'a, VM>(
    element: &'a ResolvedElement<VM>,
    column_key: &WidgetKey,
) -> Option<&'a ResolvedElement<VM>> {
    if element
        .data_grid_header
        .as_ref()
        .is_some_and(|header| &header.column_key == column_key)
    {
        return Some(element);
    }
    resolved_children(&element.kind)
        .iter()
        .find_map(|child| resolved_data_grid_header(child, column_key))
}

fn resolved_data_grid_row<'a, VM>(
    element: &'a ResolvedElement<VM>,
    row_key: &WidgetKey,
) -> Option<&'a ResolvedElement<VM>> {
    if resolved_children(&element.kind).iter().any(|child| {
        child
            .data_grid_cell
            .as_ref()
            .is_some_and(|cell| &cell.row_key == row_key)
    }) {
        return Some(element);
    }
    resolved_children(&element.kind)
        .iter()
        .find_map(|child| resolved_data_grid_row(child, row_key))
}

fn rendered_text_labels(rendered: &crate::ui::widget::common::RenderedWidgetScene) -> Vec<String> {
    rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.to_string())
        .collect()
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
fn data_grid_row_identity_survives_virtual_window_and_pinned_scroll() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let rows = (0..64)
        .map(|index| DataGridRow::keyed(format!("row-{index}"), format!("Row {index}")))
        .collect::<Vec<_>>();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(rows, pinned_columns())
            .row_height(dp(32.0))
            .overscan(2)
            .size(dp(240.0), dp(160.0)),
    );
    let viewport = Rect::new(0.0, 0.0, 240.0, 160.0);
    let initial = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let ResolvedWidgetKind::Container { children, .. } = &initial.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let body_id = children[1].id;
    let row_key = WidgetKey::from("row-2");
    let initial_row = resolved_data_grid_row(&initial.resolved_root, &row_key)
        .expect("overlap row should be visible in the initial window");
    let initial_cell_ids = resolved_children(&initial_row.kind)
        .iter()
        .filter_map(|cell| cell.data_grid_cell.as_ref().map(|state| state.row_id))
        .collect::<Vec<_>>();
    assert_eq!(initial_cell_ids.len(), 4);
    assert!(initial_cell_ids.iter().all(|id| *id == initial_row.id));

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(body_id, Point::new(dp(96.0), dp(64.0)));
    let scrolled = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &scroll_offsets,
        &HashMap::new(),
        viewport,
    );
    let scrolled_row = resolved_data_grid_row(&scrolled.resolved_root, &row_key)
        .expect("overscan overlap row should survive virtual window reconstruction");
    let scrolled_cell_ids = resolved_children(&scrolled_row.kind)
        .iter()
        .filter_map(|cell| cell.data_grid_cell.as_ref().map(|state| state.row_id))
        .collect::<Vec<_>>();
    assert_eq!(scrolled_row.id, initial_row.id);
    assert_eq!(scrolled_cell_ids, initial_cell_ids);
}

#[test]
fn data_grid_row_visual_state_prioritizes_disabled_selected_and_hover() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::light();
    let mut animations = AnimationEngine::default();
    let hover = Color::rgb(17, 79, 131);
    let selected = Color::rgb(139, 53, 19);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![
                DataGridRow::keyed("selected", "Selected".to_string()),
                DataGridRow::keyed("hovered", "Hovered".to_string()),
                DataGridRow::keyed("disabled", "Disabled".to_string()).disable(true),
            ],
            pinned_columns(),
        )
        .selected_keys(vec![WidgetKey::from("selected")])
        .style(move |style, _| {
            style.row_background = Value::Static(Color::TRANSPARENT);
            style.zebra_background = Value::Static(Color::TRANSPARENT);
            style.row_hover_background = Value::Static(hover);
            style.row_selected_background = Value::Static(selected);
        })
        .size(dp(240.0), dp(180.0)),
    );
    let viewport = Rect::new(0.0, 0.0, 240.0, 180.0);
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
    let body_id = children[1].id;
    let row_id = |key: &str| {
        resolved_data_grid_row(&layout.resolved_root, &WidgetKey::from(key))
            .unwrap_or_else(|| panic!("row {key} should be visible"))
            .id
    };
    let mut states = WidgetStateMap::default();
    for key in ["selected", "hovered", "disabled"] {
        states.set(
            row_id(key),
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
    }
    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(body_id, Point::new(dp(96.0), Dp::ZERO));
    let rendered = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &states,
        &HashMap::new(),
        &scroll_offsets,
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let selected_rows = rendered
        .scene
        .shapes
        .iter()
        .filter(|shape| shape.color == selected)
        .collect::<Vec<_>>();
    let hovered_rows = rendered
        .scene
        .shapes
        .iter()
        .filter(|shape| shape.color == hover)
        .collect::<Vec<_>>();
    assert_eq!(selected_rows.len(), 1, "selected must win over hover");
    assert_eq!(hovered_rows.len(), 1, "disabled must suppress hover");
    for shape in selected_rows.into_iter().chain(hovered_rows) {
        let clip = shape
            .clip_rect
            .expect("row background should stay body-clipped");
        assert_eq!(clip.x, Dp::ZERO);
        assert_eq!(clip.right(), viewport.right());
        assert!(shape.rect.x <= Dp::ZERO && shape.rect.right() >= viewport.right());
    }
}

#[test]
fn data_grid_interaction_layers_use_modern_semantic_tokens_in_both_themes() {
    for theme in [Theme::light(), Theme::dark()] {
        let style = DataGridStyle::default_for_theme(&theme);
        assert_eq!(
            style.row_hover_background.resolve(),
            theme.colors.on_surface.with_alpha_factor(0.06)
        );
        assert_eq!(
            style.row_selected_background.resolve(),
            theme.colors.primary.with_alpha_factor(0.12)
        );
        assert_ne!(
            style.row_hover_background.resolve(),
            style.row_selected_background.resolve()
        );
        assert_eq!(
            style.grid_line.resolve(),
            theme.colors.outline_muted.with_alpha_factor(0.42)
        );
    }
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
fn data_grid_header_and_rows_fill_viewport_when_columns_are_narrow() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 560.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            columns(),
        )
        .size(dp(560.0), dp(120.0)),
    );

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
    let row = resolved_data_grid_row(&layout.resolved_root, &WidgetKey::from("a"))
        .expect("data row should resolve");

    assert_eq!(
        layout
            .widget_bounds(children[0].id)
            .expect("DataGrid header bounds")
            .width,
        viewport.width,
        "the header surface should fill the DataGrid viewport"
    );
    assert_eq!(
        layout
            .widget_bounds(row.id)
            .expect("DataGrid row bounds")
            .width,
        viewport.width,
        "row surfaces should fill the DataGrid viewport"
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
    let header = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Name")
        .expect("expected DataGrid header text to render");
    assert_eq!(header.color, theme.colors.on_surface_muted);
    assert_eq!(header.font_size, theme.typography.label.size.get());
    assert_eq!(header.font_weight, theme.typography.label.weight);

    let cell = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Alpha")
        .expect("expected DataGrid cell text to render");
    assert_eq!(cell.color, theme.colors.on_surface);
    assert_eq!(cell.font_size, theme.typography.body_small.size.get());
    assert_eq!(cell.font_weight, theme.typography.body_small.weight);
}

#[test]
fn data_grid_default_surface_applies_modern_shape_tokens() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::light();
    let style = DataGridStyle::default_for_theme(&theme);
    let radius = style
        .surface
        .border_radius
        .as_ref()
        .expect("default DataGrid radius")
        .resolve()
        .get();
    assert_eq!(
        style
            .surface
            .border_width
            .as_ref()
            .expect("default DataGrid border width")
            .resolve(),
        theme.border.none
    );
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

    assert!(rendered.primitives.shapes.iter().any(|shape| {
        shape.rect == Rect::new(0.0, 0.0, 240.0, 120.0)
            && shape.stroke_width == 0.0
            && shape.color == theme.colors.surface
            && shape.corner_radius == radius
    }));
    let grid_line = theme.colors.outline_muted.with_alpha_factor(0.42);
    assert!(rendered.primitives.shapes.iter().any(|shape| {
        shape.stroke_width == theme.border.thin.get() && shape.color == grid_line
    }));
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
fn data_grid_column_signals_update_header_rows_and_horizontal_extent_on_the_same_tree() {
    let context = test_context();
    let first_width = context.state(dp(80.0));
    let second_width = context.state(dp(100.0));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 120.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![DataGridRow::keyed("row", "Alpha".to_string())],
            vec![
                DataGridColumn::new(
                    "first",
                    "First".to_string(),
                    |ctx: DataGridCellContext<String>| Text::new(ctx.row).into(),
                )
                .width(first_width.signal())
                .resizable(false),
                DataGridColumn::new("second", "Second".to_string(), |_ctx| {
                    Text::new("value").into()
                })
                .width(second_width.signal())
                .resizable(false),
            ],
        )
        .size(dp(120.0), dp(120.0)),
    );

    let initial = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let initial_header =
        resolved_data_grid_header(&initial.resolved_root, &WidgetKey::from("second"))
            .expect("second header should resolve");
    assert_eq!(
        initial_header
            .layout
            .width
            .as_ref()
            .map(|width| width.resolve()),
        Some(crate::ui::layout::Length::Px(dp(100.0)))
    );
    let initial_row = resolved_data_grid_row(&initial.resolved_root, &WidgetKey::from("row"))
        .expect("data row should resolve");
    assert_eq!(
        initial_row
            .layout
            .width
            .as_ref()
            .map(|width| width.resolve()),
        Some(crate::ui::layout::Length::Px(dp(180.0)))
    );
    let ResolvedWidgetKind::Container { children, .. } = &initial.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let body_id = children[1].id;
    let initial_scene = tree.collect_scene_from_layout(
        &font_manager,
        &initial,
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
    assert_eq!(
        initial_scene
            .scroll_regions
            .iter()
            .find(|region| region.id == body_id)
            .expect("DataGrid body should expose a scroll region")
            .content_bounds
            .width,
        dp(180.0)
    );

    second_width.set(dp(170.0));
    let updated = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let updated_header =
        resolved_data_grid_header(&updated.resolved_root, &WidgetKey::from("second"))
            .expect("second header should remain resolved");
    assert_eq!(updated_header.id, initial_header.id);
    assert_eq!(
        updated_header
            .layout
            .width
            .as_ref()
            .map(|width| width.resolve()),
        Some(crate::ui::layout::Length::Px(dp(170.0)))
    );
    assert_eq!(
        updated_header
            .data_grid_header
            .as_ref()
            .expect("header interaction state")
            .width,
        dp(170.0)
    );
    let updated_row = resolved_data_grid_row(&updated.resolved_root, &WidgetKey::from("row"))
        .expect("data row should remain resolved");
    assert_eq!(updated_row.id, initial_row.id);
    assert_eq!(
        updated_row
            .layout
            .width
            .as_ref()
            .map(|width| width.resolve()),
        Some(crate::ui::layout::Length::Px(dp(250.0)))
    );
    let updated_scene = tree.collect_scene_from_layout(
        &font_manager,
        &updated,
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
    assert_eq!(
        updated_scene
            .scroll_regions
            .iter()
            .find(|region| region.id == body_id)
            .expect("DataGrid body scroll region should survive the update")
            .content_bounds
            .width,
        dp(250.0)
    );
}

#[test]
fn data_grid_label_and_sort_signals_update_default_header_on_the_same_tree() {
    let context = test_context();
    let label = context.state("Name".to_string());
    let sort = context.state(Vec::<DataGridSort>::new());
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![DataGridRow::keyed("row", "Alpha".to_string())],
            vec![DataGridColumn::new(
                "name",
                label.signal(),
                |ctx: DataGridCellContext<String>| Text::new(ctx.row).into(),
            )
            .sortable(true)],
        )
        .sort(sort.signal())
        .size(dp(240.0), dp(120.0)),
    );

    let initial = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered_text_labels(&initial)
        .iter()
        .any(|text| text == "Name"));

    label.set("Display name".to_string());
    sort.set(vec![DataGridSort {
        column_key: WidgetKey::from("name"),
        direction: DataGridSortDirection::Ascending,
    }]);
    let ascending = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered_text_labels(&ascending)
        .iter()
        .any(|text| text == "Display name ↑"));
    let ascending_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let header =
        resolved_data_grid_header(&ascending_layout.resolved_root, &WidgetKey::from("name"))
            .expect("reactive header should resolve");
    let state = header
        .data_grid_header
        .as_ref()
        .expect("reactive header interaction state");
    assert_eq!(state.label, "Display name");
    assert_eq!(
        state.sort.resolve(),
        vec![DataGridSort {
            column_key: WidgetKey::from("name"),
            direction: DataGridSortDirection::Ascending,
        }]
    );

    sort.set(vec![DataGridSort {
        column_key: WidgetKey::from("name"),
        direction: DataGridSortDirection::Descending,
    }]);
    let descending = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered_text_labels(&descending)
        .iter()
        .any(|text| text == "Display name ↓"));
    assert!(!rendered_text_labels(&descending)
        .iter()
        .any(|text| text == "Display name ↑"));
}

#[test]
fn data_grid_loading_signal_switches_header_and_rows_on_the_same_tree() {
    let context = test_context();
    let loading = context.state(false);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![DataGridRow::keyed("row", "Alpha".to_string())],
            columns(),
        )
        .loading(loading.signal())
        .loading_view(Text::new("Working"))
        .size(dp(240.0), dp(120.0)),
    );

    let loaded = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let loaded_labels = rendered_text_labels(&loaded);
    assert!(loaded_labels.iter().any(|text| text == "Name"));
    assert!(loaded_labels.iter().any(|text| text == "Alpha"));
    assert!(!loaded_labels.iter().any(|text| text == "Working"));
    let loaded_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let loaded_grid = resolved_children(&loaded_layout.resolved_root.kind)
        .first()
        .expect("reactive DataGrid should expose its loaded branch");
    assert_eq!(
        loaded_grid
            .data_grid_root
            .as_ref()
            .expect("loaded DataGrid root metadata")
            .row_count,
        1
    );

    loading.set(true);
    let loading_scene = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let loading_labels = rendered_text_labels(&loading_scene);
    assert!(loading_labels.iter().any(|text| text == "Working"));
    assert!(!loading_labels.iter().any(|text| text == "Name"));
    assert!(!loading_labels.iter().any(|text| text == "Alpha"));
    let loading_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    assert!(
        resolved_data_grid_header(&loading_layout.resolved_root, &WidgetKey::from("name"))
            .is_none()
    );
    let loading_branch = resolved_children(&loading_layout.resolved_root.kind)
        .first()
        .expect("reactive DataGrid should expose its loading branch");
    assert!(loading_branch.data_grid_root.is_none());

    loading.set(false);
    let restored = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let restored_labels = rendered_text_labels(&restored);
    assert!(restored_labels.iter().any(|text| text == "Name"));
    assert!(restored_labels.iter().any(|text| text == "Alpha"));
    assert!(!restored_labels.iter().any(|text| text == "Working"));
}

#[test]
fn data_grid_loading_signal_prioritizes_loading_over_empty_without_showing_header() {
    let context = test_context();
    let loading = context.state(false);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 220.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(Vec::<DataGridRow<String>>::new(), columns())
            .loading(loading.signal())
            .loading_view(
                Stack::new()
                    .child(Text::new("Working"))
                    .size(dp(240.0), dp(160.0)),
            )
            .empty(
                Stack::new()
                    .child(Text::new("Nothing here"))
                    .size(dp(240.0), dp(150.0)),
            )
            .size(dp(240.0), dp(220.0)),
    );

    let empty = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let empty_labels = rendered_text_labels(&empty);
    assert!(empty_labels.iter().any(|text| text == "Nothing here"));
    assert!(!empty_labels.iter().any(|text| text == "Working"));
    assert!(!empty_labels.iter().any(|text| text == "Name"));
    let empty_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let empty_branch = resolved_children(&empty_layout.resolved_root.kind)
        .first()
        .expect("reactive DataGrid should expose its empty branch");
    assert_eq!(
        empty_branch
            .layout
            .height
            .as_ref()
            .map(|height| height.resolve()),
        Some(crate::ui::layout::Length::Px(dp(150.0)))
    );
    assert!(empty_branch.data_grid_root.is_none());

    loading.set(true);
    let loading_scene = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let loading_labels = rendered_text_labels(&loading_scene);
    assert!(loading_labels.iter().any(|text| text == "Working"));
    assert!(!loading_labels.iter().any(|text| text == "Nothing here"));
    assert!(!loading_labels.iter().any(|text| text == "Name"));
    let loading_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let loading_branch = resolved_children(&loading_layout.resolved_root.kind)
        .first()
        .expect("reactive DataGrid should expose its loading branch");
    assert_eq!(
        loading_branch
            .layout
            .height
            .as_ref()
            .map(|height| height.resolve()),
        Some(crate::ui::layout::Length::Px(dp(160.0)))
    );
    assert!(loading_branch.data_grid_root.is_none());

    loading.set(false);
    let empty_again = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let empty_again_labels = rendered_text_labels(&empty_again);
    assert!(empty_again_labels.iter().any(|text| text == "Nothing here"));
    assert!(!empty_again_labels.iter().any(|text| text == "Working"));
}

#[test]
fn data_grid_disabled_signal_updates_cell_behavior_on_the_same_tree() {
    let context = test_context();
    let disabled = context.state(false);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![DataGridRow::keyed("row", "Alpha".to_string()).disable(disabled.signal())],
            vec![DataGridColumn::new(
                "name",
                "Name".to_string(),
                |ctx: DataGridCellContext<String>| {
                    Text::new(if ctx.disabled { "disabled" } else { "enabled" }).into()
                },
            )],
        )
        .size(dp(240.0), dp(120.0)),
    );

    let enabled_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let enabled_row =
        resolved_data_grid_row(&enabled_layout.resolved_root, &WidgetKey::from("row"))
            .expect("enabled row should resolve");
    let enabled_cell = resolved_children(&enabled_row.kind)
        .first()
        .expect("enabled cell should resolve");
    assert_eq!(enabled_cell.focus.focusable, Some(true));
    assert_eq!(
        enabled_cell
            .interactions
            .cursor_style
            .as_ref()
            .map(|cursor| cursor.resolve()),
        Some(CursorStyle::Default)
    );
    assert!(!enabled_cell
        .data_grid_cell
        .as_ref()
        .expect("enabled cell state")
        .disabled
        .resolve());

    disabled.set(true);
    let disabled_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let disabled_row =
        resolved_data_grid_row(&disabled_layout.resolved_root, &WidgetKey::from("row"))
            .expect("disabled row should remain resolved");
    assert_eq!(disabled_row.id, enabled_row.id);
    let disabled_cell = resolved_children(&disabled_row.kind)
        .first()
        .expect("disabled cell should remain resolved");
    assert_eq!(disabled_cell.focus.focusable, Some(false));
    assert_eq!(
        disabled_cell
            .interactions
            .cursor_style
            .as_ref()
            .map(|cursor| cursor.resolve()),
        Some(CursorStyle::Default)
    );
    assert!(disabled_cell
        .data_grid_cell
        .as_ref()
        .expect("disabled cell state")
        .disabled
        .resolve());
    let disabled_scene = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let labels = rendered_text_labels(&disabled_scene);
    assert!(labels.iter().any(|text| text == "disabled"));
    assert!(!labels.iter().any(|text| text == "enabled"));
}

#[test]
fn data_grid_normalizes_non_finite_and_inverted_column_width_bounds() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::<String, ()>::new(
            vec![DataGridRow::keyed("row", "Alpha".to_string())],
            vec![
                DataGridColumn::new("name", "Name".to_string(), |ctx| Text::new(ctx.row).into())
                    .width(Dp::new(f32::NAN))
                    .min_width(dp(72.0))
                    .max_width(dp(40.0)),
            ],
        )
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
        viewport,
    );
    let header = resolved_data_grid_header(&layout.resolved_root, &WidgetKey::from("name"))
        .expect("normalized header should resolve");
    let state = header
        .data_grid_header
        .as_ref()
        .expect("normalized header state");
    assert_eq!(state.width, dp(72.0));
    assert_eq!(state.min_width, dp(72.0));
    assert_eq!(state.max_width, Some(dp(72.0)));
    let row = resolved_data_grid_row(&layout.resolved_root, &WidgetKey::from("row"))
        .expect("normalized row should resolve");
    assert_eq!(
        row.layout.width.as_ref().map(|width| width.resolve()),
        Some(crate::ui::layout::Length::Px(dp(72.0)))
    );
}

#[test]
fn data_grid_density_scales_header_and_row_together() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();

    for (density, expected_height) in [
        (DataGridDensity::Compact, dp(32.0)),
        (DataGridDensity::Regular, dp(40.0)),
        (DataGridDensity::Spacious, dp(48.0)),
    ] {
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            DataGrid::new(
                vec![DataGridRow::keyed("a", "Alpha".to_string())],
                columns(),
            )
            .density(density)
            .size(dp(240.0), dp(144.0)),
        );
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 240.0, 144.0),
        );
        let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
            panic!("DataGrid root should resolve to a container");
        };
        assert_eq!(
            children[0]
                .layout
                .height
                .as_ref()
                .map(|value| value.resolve()),
            Some(crate::ui::layout::Length::Px(expected_height)),
            "header height should follow {density:?} density"
        );
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
            .expect("density test row should resolve");
        assert_eq!(
            row.layout.height.as_ref().map(|value| value.resolve()),
            Some(crate::ui::layout::Length::Px(expected_height)),
            "row height should follow {density:?} density"
        );
    }
}

#[test]
fn data_grid_runtime_metrics_follow_theme_on_the_same_tree_and_keep_overrides() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 360.0, 220.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            (0..40)
                .map(|index| DataGridRow::keyed(index, format!("Row {index}")))
                .collect::<Vec<_>>(),
            columns(),
        )
        .style(|style, context| match context.density {
            Density::Compact => {
                style.header_height = dp(34.0);
                style.regular_row_height = dp(30.0);
                style.cell_padding = Insets::all(dp(2.0));
            }
            Density::Comfortable => {}
            Density::Spacious => {
                style.header_height = dp(58.0);
                style.regular_row_height = dp(54.0);
                style.cell_padding = Insets::all(dp(14.0));
            }
        })
        .size(dp(340.0), dp(200.0)),
    );

    for (density, header_height, row_height, padding) in [
        (Density::Compact, dp(34.0), dp(30.0), dp(2.0)),
        (Density::Spacious, dp(58.0), dp(54.0), dp(14.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
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
        assert_eq!(
            children[0]
                .layout
                .height
                .as_ref()
                .map(|value| value.resolve()),
            Some(crate::ui::layout::Length::Px(header_height))
        );
        let header_cell = resolved_children(&children[0].kind)
            .first()
            .expect("header cell should resolve");
        assert_eq!(
            subtree_container_padding(header_cell),
            Some(Value::Static(Insets::all(padding)))
        );

        let ResolvedWidgetKind::Virtual {
            item_layout,
            children: rows,
            ..
        } = &children[1].kind
        else {
            panic!("DataGrid body should use VirtualList");
        };
        assert!(matches!(
            item_layout,
            ItemLayout::Fixed { item_extent, .. } if (*item_extent - row_height).abs() <= dp(0.1)
        ));
        let row = rows
            .iter()
            .find(|row| {
                resolved_children(&row.kind)
                    .iter()
                    .any(|cell| cell.data_grid_cell.is_some())
            })
            .expect("visible data row should resolve");
        assert_eq!(
            row.layout.height.as_ref().map(|value| value.resolve()),
            Some(crate::ui::layout::Length::Px(row_height))
        );
        let cell = resolved_children(&row.kind)
            .first()
            .expect("data cell should resolve");
        let ResolvedWidgetKind::Container {
            layout: cell_layout,
            ..
        } = &cell.kind
        else {
            panic!("data cell should remain a container");
        };
        assert_eq!(
            cell_layout.padding,
            Some(Value::Static(Insets::all(padding)))
        );
    }

    let override_tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(
            vec![DataGridRow::keyed("a", "Alpha".to_string())],
            columns(),
        )
        .style(|style, context| {
            style.regular_row_height = match context.density {
                Density::Compact => dp(24.0),
                Density::Comfortable => dp(40.0),
                Density::Spacious => dp(72.0),
            };
        })
        .row_height(dp(44.0))
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(60.0),
            spacing: dp(3.0),
            overscan: 5,
        })
        .size(dp(340.0), dp(180.0)),
    );
    let mut theme = Theme::light();
    theme.density = Density::Spacious;
    let mut animations = AnimationEngine::default();
    let layout = override_tree.build_scene_layout(
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
    let ResolvedWidgetKind::Virtual {
        item_layout,
        children: rows,
        ..
    } = &children[1].kind
    else {
        panic!("DataGrid body should use VirtualList");
    };
    assert!(matches!(
        item_layout,
        ItemLayout::Fixed { item_extent, spacing, overscan }
            if (*item_extent - dp(60.0)).abs() <= dp(0.1)
                && (*spacing - dp(3.0)).abs() <= dp(0.1)
                && *overscan == 5
    ));
    let row = rows
        .iter()
        .find(|row| {
            resolved_children(&row.kind)
                .iter()
                .any(|cell| cell.data_grid_cell.is_some())
        })
        .expect("visible data row should resolve");
    assert_eq!(
        row.layout.height.as_ref().map(|value| value.resolve()),
        Some(crate::ui::layout::Length::Px(dp(44.0)))
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

#[test]
fn large_data_grid_selection_snapshot_is_shared_and_membership_stays_equivalent() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let rows = (0..10_000)
        .map(|index| DataGridRow::keyed(index, format!("Row {index}")))
        .collect::<Vec<_>>();
    let selected = (0..10_000)
        .step_by(2)
        .map(WidgetKey::from)
        .collect::<Vec<_>>();
    let tree: WidgetTree<()> = WidgetTree::new(
        DataGrid::new(rows, columns())
            .selected_keys(selected)
            .selection_mode(DataGridSelectionMode::Multiple)
            .size(dp(240.0), dp(160.0)),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 160.0),
    );
    let root_selection = layout
        .resolved_root
        .data_grid_root
        .as_ref()
        .expect("DataGrid root selection metadata")
        .selection
        .clone();
    let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
        panic!("DataGrid root should resolve to a container");
    };
    let ResolvedWidgetKind::Virtual { children: rows, .. } = &children[1].kind else {
        panic!("DataGrid body should use VirtualList");
    };
    let states = rows
        .iter()
        .flat_map(|row| resolved_children(&row.kind))
        .filter_map(|cell| cell.data_grid_cell.as_ref())
        .collect::<Vec<_>>();
    let shared_keys = root_selection.selected_keys.resolve();
    let shared_membership = root_selection.selected_key_membership.resolve();
    for state in states {
        assert!(Arc::ptr_eq(&root_selection, &state.selection));
        let keys = state.selection.selected_keys.resolve();
        let membership = state.selection.selected_key_membership.resolve();
        assert!(Arc::ptr_eq(&shared_keys, &keys));
        assert!(Arc::ptr_eq(&shared_membership, &membership));
        assert_eq!(
            membership.contains(&state.row_key),
            keys.contains(&state.row_key),
            "membership snapshot must preserve controlled DataGrid selection semantics"
        );
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn data_grid_selection_metadata_stays_out_of_line() {
    let size = std::mem::size_of::<crate::ui::widget::common::DataGridCellState<()>>();
    assert!(
        size <= 800,
        "DataGrid selection lookup tables must remain behind Arc; got {size} bytes"
    );
}
