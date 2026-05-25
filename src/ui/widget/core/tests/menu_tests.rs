pub(super) use super::*;

use crate::ui::widget::{Button, ContextMenu, Menu, MenuBar, MenuItem};

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
