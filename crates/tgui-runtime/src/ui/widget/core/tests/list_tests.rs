use super::*;

use crate::theme::WidgetState;
use crate::ui::widget::{
    ItemLayout, List, ListItem, ListSection, ListSelectionMode, ListStyle, VirtualCacheState,
    WidgetKey,
};

#[test]
fn list_default_style_follows_theme_density() {
    let expected = [
        (
            crate::ui::theme::Density::Compact,
            dp(32.0),
            dp(8.0),
            dp(4.0),
            dp(6.0),
        ),
        (
            crate::ui::theme::Density::Comfortable,
            dp(40.0),
            dp(12.0),
            dp(8.0),
            dp(8.0),
        ),
        (
            crate::ui::theme::Density::Spacious,
            dp(48.0),
            dp(16.0),
            dp(12.0),
            dp(12.0),
        ),
    ];

    for (density, height, padding_x, padding_y, radius) in expected {
        let mut theme = Theme::light();
        theme.density = density;
        let style = ListStyle::default_for_theme(&theme);

        assert_eq!(style.item_height, height);
        assert_eq!(style.item_padding.left, padding_x);
        assert_eq!(style.item_padding.top, padding_y);
        assert_eq!(style.item_radius, radius);
    }
}

#[test]
fn compact_list_hover_scene_uses_rounded_inset_state_layer() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut theme = Theme::light();
    theme.density = crate::ui::theme::Density::Compact;
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(vec![ListItem::keyed("a", "Alpha")], |ctx| {
            Text::new(ctx.item).into()
        })
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(32.0),
            spacing: Dp::ZERO,
            overscan: 0,
        })
        .size(dp(240.0), dp(64.0)),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 64.0),
    );
    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("List should resolve to a virtual widget");
    };
    let row_id = children[0].id;
    let mut states = WidgetStateMap::default();
    states.set(
        row_id,
        WidgetState {
            hovered: true,
            ..WidgetState::default()
        },
    );

    let rendered = tree
        .collect_scene_from_layout(
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
            &HashMap::new(),
            Rect::new(0.0, 0.0, 240.0, 64.0),
            None,
            None,
            None,
            None,
            false,
        )
        .rendered();
    let style = ListStyle::default_for_theme(&theme);
    let hover = style.item_hover_background.resolve();
    let state_layer = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.color == hover)
        .expect("hovered List row should emit its state layer");
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Alpha")
        .expect("List row label should render");

    assert_eq!(state_layer.rect.height, style.item_height);
    assert_eq!(state_layer.corner_radius, style.item_radius.get());
    assert_eq!(label.frame.x - state_layer.rect.x, style.item_padding.left);
}

#[test]
fn list_loading_slot_takes_priority_over_empty_slot() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(Vec::<ListItem<&'static str>>::new(), |ctx| {
            Text::new(ctx.item).into()
        })
        .empty(Text::new("empty"))
        .loading(true)
        .loading_view(Text::new("loading")),
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
    let labels = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"loading"));
    assert!(!labels.contains(&"empty"));
}

#[test]
fn list_empty_slot_renders_when_not_loading() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(Vec::<ListItem<&'static str>>::new(), |ctx| {
            Text::new(ctx.item).into()
        })
        .empty(Text::new("empty"))
        .loading(false)
        .loading_view(Text::new("loading")),
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
    let labels = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"empty"));
    assert!(!labels.contains(&"loading"));
}

#[test]
fn list_virtualizes_rows_and_keeps_selection_wrapper_inside_window() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let items: Vec<ListItem<String>> = (0..1_000)
        .map(|index| ListItem::keyed(index, format!("Row {index}")))
        .collect::<Vec<_>>();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<String, ()>::new(items, |ctx| Text::new(ctx.item).into())
            .height(dp(160.0))
            .width(dp(240.0))
            .overscan(1),
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

    let ResolvedWidgetKind::Virtual {
        children,
        window_plan,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("List should resolve to the Virtual widget path");
    };

    assert!(children.len() < 12);
    assert_eq!(children.len(), window_plan.placements.len());
    assert!(children.iter().all(|child| child.list_item.is_some()));
}

#[test]
fn large_list_selection_snapshot_is_shared_and_membership_stays_equivalent() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let items = (0..100_000)
        .map(|index| ListItem::keyed(index, index))
        .collect::<Vec<_>>();
    let selected = (0..100_000)
        .step_by(2)
        .map(WidgetKey::from)
        .collect::<Vec<_>>();
    let tree = WidgetTree::new(
        List::<usize, ()>::new(items, |ctx| Text::new(ctx.item.to_string()).into())
            .selected_keys(selected)
            .selection_mode(ListSelectionMode::Multiple)
            .item_layout(ItemLayout::Fixed {
                item_extent: dp(40.0),
                spacing: Dp::ZERO,
                overscan: 0,
            })
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
    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("List should resolve to a virtual widget");
    };
    let states = children
        .iter()
        .map(|child| child.list_item.as_ref().expect("visible row state"))
        .collect::<Vec<_>>();
    let shared_keys = states[0].selection.selected_keys.resolve();
    let shared_membership = states[0].selection.selected_key_membership.resolve();
    for state in states {
        let keys = state.selection.selected_keys.resolve();
        let membership = state.selection.selected_key_membership.resolve();
        assert!(Arc::ptr_eq(&shared_keys, &keys));
        assert!(Arc::ptr_eq(&shared_membership, &membership));
        assert_eq!(
            membership.contains(&state.key),
            keys.contains(&state.key),
            "membership snapshot must preserve controlled selection semantics"
        );
        assert_eq!(
            state
                .selection
                .sibling_index_by_key
                .get(&state.key)
                .copied(),
            Some(state.item_index)
        );
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn list_selection_metadata_stays_out_of_line() {
    assert!(
        std::mem::size_of::<crate::ui::widget::common::ListItemState<()>>() <= 960,
        "selection lookup tables must remain behind Arc so recursive Element/ResolvedElement values do not grow"
    );
}

#[test]
fn list_scroll_reuses_each_previous_row_id_at_most_once() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let list: Element<()> = List::<String, ()>::new(
        (0..6)
            .map(|index| ListItem::keyed(index, format!("Row {index}")))
            .collect::<Vec<_>>(),
        |ctx| Text::new(ctx.item).into(),
    )
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(40.0),
        spacing: Dp::ZERO,
        overscan: 0,
    })
    .size(dp(240.0), dp(120.0))
    .into();
    let list_id = list.id;
    let tree = WidgetTree::new(list);

    let first_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let scroll_offsets = HashMap::from([(list_id, Point::new(Dp::ZERO, dp(40.0)))]);
    let scrolled_layout = tree.build_scene_layout_at_with_previous(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &scroll_offsets,
        &HashMap::new(),
        viewport,
        Instant::now(),
        Some(&first_layout),
    );

    let ResolvedWidgetKind::Virtual { children, .. } = &scrolled_layout.resolved_root.kind else {
        panic!("List should resolve to a virtual widget");
    };
    let row_ids = children
        .iter()
        .filter(|child| child.list_item.is_some())
        .map(|child| child.id)
        .collect::<Vec<_>>();
    let unique_row_ids = row_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(
        row_ids.len(),
        unique_row_ids.len(),
        "visible List rows must not share WidgetId values after scroll: {row_ids:?}"
    );
}

#[test]
fn list_selected_row_emits_selected_background() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let selected = Color::hexa(0xAA5500FF);
    let items: Vec<ListItem<&'static str>> =
        vec![ListItem::keyed("a", "Alpha"), ListItem::keyed("b", "Beta")];
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(items, |ctx| Text::new(ctx.item).into())
            .selected_keys(vec![WidgetKey::from("b")])
            .style_full(move |ctx| {
                let mut style = ListStyle::default_for_theme(ctx.theme);
                style.item_selected_background = selected.into();
                style
            })
            .size(dp(240.0), dp(96.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 96.0),
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
            .any(|shape| shape.stroke_width == 0.0 && shape.color == selected),
        "selected List row should render its selected background"
    );
}

#[test]
fn list_section_headers_render_but_do_not_become_list_items() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let sections: Vec<ListSection<&'static str, ()>> = vec![
        ListSection::new(
            Text::new("Group A").height(dp(28.0)),
            vec![ListItem::keyed("a", "Alpha")],
        ),
        ListSection::new(
            Text::new("Group B").height(dp(28.0)),
            vec![ListItem::keyed("b", "Beta")],
        ),
    ];
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::sections(sections, |ctx| Text::new(ctx.item).into())
            .selection_mode(ListSelectionMode::Multiple)
            .height(dp(160.0))
            .width(dp(240.0)),
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

    let ResolvedWidgetKind::Virtual { children, .. } = &layout.resolved_root.kind else {
        panic!("sectioned List should resolve to a virtual widget");
    };

    assert_eq!(children.len(), 4);
    assert!(children[0].list_item.is_none());
    assert!(children[1].list_item.is_some());
    assert!(children[2].list_item.is_none());
    assert!(children[3].list_item.is_some());
}

#[test]
fn list_rows_stretch_content_to_viewport_width() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let items = vec![ListItem::keyed("a", "Alpha contact")];
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(items, |ctx| {
            Flex::vertical()
                .gap(dp(2.0))
                .child(Text::new(ctx.item).width(pct(100.0)))
                .child(Text::new("Role - status").width(pct(100.0)))
                .into()
        })
        .height(dp(80.0))
        .width(dp(240.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    let row_title = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "Alpha contact")
        .expect("row title should render");
    assert!(
        row_title.frame.width > dp(180.0),
        "row title should stretch across the list viewport, got {:?}",
        row_title.frame
    );
}

#[test]
fn virtual_list_items_use_current_viewport_width_when_cached_hint_is_narrow() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(vec![ListItem::keyed("a", "Alpha contact")], |ctx| {
            Text::new(ctx.item).width(pct(100.0)).into()
        })
        .height(dp(80.0))
        .width(dp(240.0)),
    );

    let initial_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 80.0),
    );
    let list_id = initial_layout.root_id();
    let mut virtual_states = HashMap::new();
    virtual_states.insert(
        list_id,
        VirtualCacheState {
            viewport_hint: Some(crate::ui::widget::r#virtual::VirtualViewportHint {
                width: dp(24.0),
                height: dp(80.0),
            }),
            ..Default::default()
        },
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &virtual_states,
        Rect::new(0.0, 0.0, 240.0, 80.0),
    );
    let first_row = layout
        .layout_root
        .children
        .first()
        .expect("virtual list should lay out a visible row");
    let first_row_layout = layout
        .taffy
        .layout(first_row.node)
        .expect("row layout should be available");
    assert!(
        first_row_layout.size.width > 180.0,
        "row width should follow the current viewport, got {}",
        first_row_layout.size.width
    );
}

#[test]
fn measured_list_rows_expand_to_fit_multiline_content() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        List::<&'static str, ()>::new(vec![ListItem::keyed("a", "Alpha contact")], |ctx| {
            Flex::vertical()
                .gap(dp(2.0))
                .child(Text::new(ctx.item).width(pct(100.0)))
                .child(Text::new("Product lead - Planning Q3 roadmap").width(pct(100.0)))
                .into()
        })
        .item_layout(ItemLayout::Measured {
            estimate: dp(64.0),
            spacing: dp(4.0),
            overscan: 1,
        })
        .height(dp(120.0))
        .width(dp(240.0)),
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
    let first_row = layout
        .layout_root
        .children
        .first()
        .expect("measured List should lay out the visible row");
    let row_layout = layout
        .taffy
        .layout(first_row.node)
        .expect("row layout should be available");
    assert!(
        row_layout.size.height > 50.0,
        "measured row should expand past the default 40dp item height, got {}",
        row_layout.size.height
    );
}
