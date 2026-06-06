use super::*;

use crate::ui::widget::{
    ItemLayout, List, ListItem, ListSection, ListSelectionMode, ListStyle, VirtualCacheState,
    WidgetKey,
};

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
            .style(move |mode| {
                let mut style = ListStyle::default_for(mode);
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
