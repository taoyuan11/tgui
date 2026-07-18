pub(super) use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    Button, ContextMenu, HitInteraction, Menu, MenuBar, MenuBarStyle, MenuIcon, MenuItem,
    MenuStyle, RenderCommand,
};

const SIMPLE_MENU_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16" rx="3" fill="#2f80ed"/></svg>"##;

#[test]
fn menu_builder_produces_element_with_descriptor() {
    let element: Element<()> = Menu::new(Button::new("File"))
        .items(vec![
            MenuItem::new("New").on_select(Command::new(|_: &mut ()| {})),
            MenuItem::separator(),
            MenuItem::new("Open"),
            MenuItem::new("Disabled").disable(true),
        ])
        .into();
    assert!(element.menu.is_some(), "menu descriptor must be attached");
    let descriptor = element.menu.as_ref().unwrap();
    assert_eq!(descriptor.items.len(), 4);
}

#[test]
fn menu_open_false_renders_only_trigger() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")])
            .open(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 200.0),
        None,
        None,
        None,
        None,
        false,
    );

    // 关闭状态下不应该有菜单 overlay 文本（只有 button 标签 "File"）。
    let overlay_texts = rendered.primitives.overlay_texts.len();
    assert_eq!(
        overlay_texts, 0,
        "closed menu should not emit overlay texts, got {overlay_texts}"
    );
}

#[test]
fn menu_open_true_emits_items_in_overlay() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::new("New").shortcut_hint("Ctrl+N"),
                MenuItem::separator(),
                MenuItem::new("Open"),
            ])
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    let labels: Vec<&str> = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(
        labels.iter().any(|t| *t == "New"),
        "open menu should render 'New' item, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Open"),
        "open menu should render 'Open' item, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Ctrl+N"),
        "shortcut hint 'Ctrl+N' should be rendered, got {labels:?}"
    );
}

#[test]
fn menu_open_true_emits_filled_background_and_separate_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")])
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    let style = MenuStyle::default_for_theme(&theme);
    let background = style.background.resolve();
    let border = style.border.resolve();
    let border_width = style.border_width.resolve().get();
    assert!(
        rendered.primitives.overlay_shapes.iter().any(|shape| {
            shape.color == background
                && shape.color.a > 0
                && shape.stroke_width == 0.0
                && shape.rect.width >= style.min_width
        }),
        "open menu should render a filled background shape, got {} overlay shapes",
        rendered.primitives.overlay_shapes.len()
    );
    assert!(
        rendered.primitives.overlay_shapes.iter().any(|shape| {
            shape.color == border && (shape.stroke_width - border_width).abs() < f32::EPSILON
        }),
        "open menu should render border separately from the background"
    );
}

#[test]
fn menu_hover_scene_uses_density_geometry_and_rounded_state_layer() {
    let mut theme = Theme::light();
    theme.density = crate::ui::theme::Density::Compact;
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let menu_element: Element<()> = Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
        .items(vec![MenuItem::new("New")])
        .open(true)
        .into();
    let menu_id = menu_element.id;
    let tree = WidgetTree::new(menu_element);
    let mut states = WidgetStateMap::default();
    let mut state = states.get_select_option(menu_id, 0);
    state.hovered = true;
    states.set_select_option(menu_id, 0, state);

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    let style = MenuStyle::default_for_theme(&theme);
    let hover = style.item_background.hovered.resolve();
    let state_layer = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| shape.color == hover)
        .expect("hovered menu item should emit a state-layer shape");
    assert_eq!(state_layer.rect.height, style.item_min_height);
    assert_eq!(
        state_layer.corner_radius,
        (style.radius.resolve() - style.padding.left).get()
    );
    assert!(state_layer.rect.x > dp(0.0));
}

#[test]
fn menu_checkable_item_renders_checkmark() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("Options").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::checkable("Wrap").checked(true),
                MenuItem::checkable("Bold").checked(false),
            ])
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    let texts: Vec<&str> = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|t| t.content.as_ref())
        .collect();
    let check_count = texts.iter().filter(|t| **t == "\u{2713}").count();
    assert_eq!(
        check_count, 1,
        "only the checked item should render ✓, got texts {texts:?}"
    );
}

#[test]
fn menu_submenu_item_renders_arrow() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::submenu("Recent", vec![MenuItem::new("a.txt")]),
                MenuItem::new("Exit"),
            ])
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );
    let arrow_count = rendered
        .primitives
        .overlay_texts
        .iter()
        .filter(|t| t.content.as_ref() == "\u{25B8}")
        .count();
    assert_eq!(arrow_count, 1, "submenu item should render ▸ arrow");
}

#[test]
fn context_menu_builder_attaches_descriptor_and_long_press() {
    let element: Element<()> = ContextMenu::new(Button::new("Photo"))
        .items(vec![MenuItem::new("Copy"), MenuItem::new("Delete")])
        .on_show(ValueCommand::new(
            |_: &mut (), _: crate::ui::widget::LongPressEvent| {},
        ))
        .into();
    assert!(
        element.context_menu.is_some(),
        "context_menu descriptor must be attached"
    );
    assert!(
        element.interactions.gesture.is_some(),
        "context menu should auto-attach long-press gesture",
    );
    let descriptor = element.context_menu.as_ref().unwrap();
    assert_eq!(descriptor.items.len(), 2);
}

#[test]
fn menubar_builder_produces_horizontal_flex_with_entries() {
    let element: Element<()> = MenuBar::new(None::<usize>)
        .entry("File", vec![MenuItem::new("New")])
        .entry("Edit", vec![MenuItem::new("Undo")])
        .into();
    // MenuBar 落地为 Flex，每个 entry 是一个 Menu（Button + menu descriptor）。
    let WidgetKind::Container {
        layout, children, ..
    } = &element.kind
    else {
        panic!("MenuBar should produce a Container kind");
    };
    assert!(matches!(layout.kind, ContainerKind::Flex { .. }));
    // 两个 entry。
    let total_children: usize = children
        .iter()
        .map(|src| match src {
            crate::ui::widget::common::ChildSource::Static(items) => items.len(),
            crate::ui::widget::common::ChildSource::Dynamic(_) => 0,
            crate::ui::widget::common::ChildSource::KeyedFor(_) => 0,
            crate::ui::widget::common::ChildSource::Switch { .. } => 0,
            crate::ui::widget::common::ChildSource::Show { .. } => 0,
        })
        .sum();
    assert_eq!(total_children, 2);
}

#[test]
fn menubar_runtime_geometry_and_open_menu_follow_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 520.0, 320.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        MenuBar::new(Some(0usize))
            .entry("File", vec![MenuItem::new("New")])
            .entry("Edit", vec![MenuItem::new("Undo")])
            .style(|style, context| match context.density {
                Density::Compact => {
                    style.height = dp(30.0);
                    style.padding = Insets::all(dp(3.0));
                    style.entry_min_width = dp(64.0);
                    style.entry_gap = dp(2.0);
                }
                Density::Comfortable => {}
                Density::Spacious => {
                    style.height = dp(50.0);
                    style.padding = Insets::all(dp(11.0));
                    style.entry_min_width = dp(104.0);
                    style.entry_gap = dp(10.0);
                }
            }),
    );

    for (mut theme, height, padding, entry_height, entry_min_width, entry_gap) in [
        (
            Theme::light(),
            dp(30.0),
            dp(3.0),
            dp(24.0),
            dp(64.0),
            dp(2.0),
        ),
        (
            Theme::dark(),
            dp(50.0),
            dp(11.0),
            dp(28.0),
            dp(104.0),
            dp(10.0),
        ),
    ] {
        theme.density = if matches!(theme.mode, crate::ui::theme::ResolvedThemeMode::Light) {
            Density::Compact
        } else {
            Density::Spacious
        };
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
        assert_eq!(
            layout.resolved_root.layout.height,
            Some(Value::Static(crate::ui::layout::Length::Px(height)))
        );
        let ResolvedWidgetKind::Container {
            layout: root_layout,
            children,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("MenuBar root should remain a container");
        };
        assert_eq!(
            root_layout.padding,
            Some(Value::Static(Insets::all(padding)))
        );
        assert_eq!(
            root_layout.gap,
            Value::Static(crate::ui::layout::Length::Px(entry_gap))
        );
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|entry| {
            entry.layout.height == Some(Value::Static(crate::ui::layout::Length::Px(entry_height)))
                && entry.layout.min_width
                    == Some(Value::Static(crate::ui::layout::Length::Px(
                        entry_min_width,
                    )))
        }));

        let mut animations = AnimationEngine::default();
        let computed = tree.compute_scene(
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
        assert!(computed
            .rendered()
            .primitives
            .overlay_texts
            .iter()
            .any(|text| text.content.as_ref() == "New"));
        let expected_background = MenuBarStyle::default_for_theme(&theme).background.resolve();
        assert!(computed
            .rendered()
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == expected_background));
        let entry_hits = computed
            .hit_regions
            .iter()
            .filter(|hit| matches!(hit.interaction, HitInteraction::Widget { .. }))
            .collect::<Vec<_>>();
        assert_eq!(entry_hits.len(), 2);
        assert!(entry_hits.iter().all(|hit| hit.rect.height == entry_height));
    }
}

#[test]
fn menubar_default_solved_and_hit_geometry_tracks_density_and_keeps_dropdown_below_bar() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 360.0);

    for mut theme in [Theme::light(), Theme::dark()] {
        for (density, expected_height) in [
            (Density::Compact, dp(32.0)),
            (Density::Comfortable, dp(40.0)),
            (Density::Spacious, dp(48.0)),
        ] {
            theme.density = density;
            let tree: WidgetTree<()> = WidgetTree::new(
                MenuBar::new(Some(0usize))
                    .entry(
                        "File",
                        vec![MenuItem::new("New").on_select(Command::new(|_: &mut ()| {}))],
                    )
                    .entry("Edit", vec![MenuItem::new("Undo")]),
            );
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
            let root_layout = layout
                .taffy
                .layout(layout.layout_root.node)
                .expect("MenuBar root layout");
            assert_eq!(dp(root_layout.size.height), expected_height);
            assert!(layout.layout_root.children.iter().all(|entry| {
                layout
                    .taffy
                    .layout(entry.node)
                    .is_ok_and(|entry_layout| dp(entry_layout.size.height) == expected_height)
            }));

            let mut animations = AnimationEngine::default();
            let computed = tree.compute_scene(
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
            let entry_hits = computed
                .hit_regions
                .iter()
                .filter(|hit| matches!(hit.interaction, HitInteraction::Widget { .. }))
                .collect::<Vec<_>>();
            assert_eq!(entry_hits.len(), 2);
            assert!(entry_hits
                .iter()
                .all(|hit| hit.rect.height == expected_height));
            let dropdown_top = computed
                .overlay_hit_regions
                .iter()
                .map(|hit| hit.rect.y)
                .min_by(|left, right| left.get().total_cmp(&right.get()))
                .expect("open MenuBar entry should emit dropdown hits");
            assert!(dropdown_top >= expected_height);
        }
    }
}

#[test]
fn menu_glyph_icon_renders_when_present() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::new("New").icon(MenuIcon::glyph('📄')),
                MenuItem::new("Open"),
            ])
            .open(true),
    );
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );
    let texts: Vec<&str> = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|t| t.content.as_ref())
        .collect();
    assert!(
        texts.iter().any(|t| *t == "\u{1F4C4}"),
        "glyph icon 📄 should render in overlay, got {texts:?}"
    );
}

#[test]
fn menu_svg_icon_renders_as_overlay_texture() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::new("New").icon(MenuIcon::svg(SIMPLE_MENU_SVG)),
                MenuItem::new("Open"),
            ])
            .open(true),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(
        rendered.primitives.overlay_textures.len(),
        1,
        "SVG menu icon should render as one overlay texture"
    );
    assert!(
        rendered
            .primitives
            .overlay_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Texture(_))),
        "SVG menu icon should participate in overlay command ordering"
    );
    assert_eq!(
        rendered.primitives.overlay_textures[0].texture.size(),
        (16, 16)
    );
}

#[test]
fn submenu_emits_nested_overlay_when_parent_is_hovered() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    // 先用 Element 拿到 menu_id（descriptor 挂在 trigger 上）。
    let menu_element: Element<()> = Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
        .items(vec![
            MenuItem::submenu(
                "Recent",
                vec![MenuItem::new("a.txt"), MenuItem::new("b.txt")],
            ),
            MenuItem::new("Exit"),
        ])
        .open(true)
        .into();
    let menu_id = menu_element.id;
    let tree: WidgetTree<()> = WidgetTree::new(menu_element);

    // baseline：未 hover 时 submenu 不应渲染。
    let baseline = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );
    let baseline_labels: Vec<String> = baseline
        .primitives
        .overlay_texts
        .iter()
        .map(|t| t.content.to_string())
        .collect();
    assert!(
        !baseline_labels.iter().any(|t| t == "a.txt"),
        "submenu items must not appear without hover, got {baseline_labels:?}"
    );

    // 把父 item (index=0) 标 hovered=true：collect emit 应递归 emit 子菜单。
    let mut states = WidgetStateMap::default();
    let mut state = states.get_select_option(menu_id, 0);
    state.hovered = true;
    states.set_select_option(menu_id, 0, state);

    let with_hover = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hover_labels: Vec<String> = with_hover
        .primitives
        .overlay_texts
        .iter()
        .map(|t| t.content.to_string())
        .collect();
    assert!(
        hover_labels.iter().any(|t| t == "a.txt"),
        "submenu item 'a.txt' should appear when parent hovered, got {hover_labels:?}"
    );
    assert!(
        hover_labels.iter().any(|t| t == "b.txt"),
        "submenu item 'b.txt' should appear when parent hovered, got {hover_labels:?}"
    );
}

#[test]
fn nested_submenu_hover_state_and_overlay_ids_are_path_scoped() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let menu_element: Element<()> = Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
        .items(vec![MenuItem::submenu(
            "Level 1",
            vec![MenuItem::submenu("Level 2", vec![MenuItem::new("Leaf")])],
        )])
        .open(true)
        .into();
    let menu_id = menu_element.id;
    let tree = WidgetTree::new(menu_element);
    let viewport = Rect::new(0.0, 0.0, 900.0, 500.0);

    let mut root_hover = WidgetStateMap::default();
    let mut state = root_hover.get_select_option(menu_id, 0);
    state.hovered = true;
    root_hover.set_select_option(menu_id, 0, state);
    let root_scene = tree.compute_scene_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &root_hover,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let root_labels: Vec<_> = root_scene
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(root_labels.contains(&"Level 2"));
    assert!(
        !root_labels.contains(&"Leaf"),
        "hovering root option 0 must not also hover submenu option 0"
    );

    let nested_owner = crate::ui::widget::menu_item_state_owner(menu_id, &[0]);
    let mut nested_hover = root_hover;
    let mut state = nested_hover.get_select_option(nested_owner, 0);
    state.hovered = true;
    nested_hover.set_select_option(nested_owner, 0, state);
    let nested_scene = tree.compute_scene_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &nested_hover,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(nested_scene
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "Leaf"));
    let overlay_ids: std::collections::HashSet<_> = nested_scene
        .overlay_close_handlers
        .iter()
        .map(|handler| handler.overlay_id)
        .collect();
    assert_eq!(
        overlay_ids.len(),
        nested_scene.overlay_close_handlers.len(),
        "every submenu depth must retain a unique overlay id"
    );
}
