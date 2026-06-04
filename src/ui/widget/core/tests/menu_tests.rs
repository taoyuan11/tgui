pub(super) use super::*;

use crate::ui::widget::{Button, ContextMenu, Menu, MenuBar, MenuIcon, MenuItem, RenderCommand};

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
        .map(|text| text.content.as_str())
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
        .map(|t| t.content.as_str())
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
        .filter(|t| t.content == "\u{25B8}")
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
        })
        .sum();
    assert_eq!(total_children, 2);
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
        .map(|t| t.content.as_str())
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
        .map(|t| t.content.clone())
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
        .map(|t| t.content.clone())
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
