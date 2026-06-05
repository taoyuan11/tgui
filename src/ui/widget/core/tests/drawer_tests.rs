use super::*;
use crate::ui::widget::{DrawerHost, DrawerMode, Flex};

#[test]
fn closed_signal_drawer_does_not_intercept_underlying_button_hits() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let open = context.state(false);
    let mut animations = AnimationEngine::default();

    let button: Element<()> = crate::ui::widget::Button::new("open")
        .size(dp(120.0), dp(40.0))
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(300.0), dp(200.0))
            .child(button)
            .child(
                Drawer::new(open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("drawer")),
            ),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 300.0, 200.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );

    assert!(
        matches!(hit, Some(HitInteraction::Widget { id, .. }) if id == button_id),
        "closed drawer should leave the underlying button clickable"
    );
}

#[test]
fn closed_sibling_drawers_do_not_intercept_toolbar_button_hits() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let left_open = context.state(false);
    let right_open = context.state(false);
    let top_open = context.state(false);
    let bottom_open = context.state(false);
    let mut animations = AnimationEngine::default();

    let left_button: Element<()> = crate::ui::widget::Button::new("open left")
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let left_button_id = left_button.id;

    let tree: WidgetTree<()> = WidgetTree::new(
        Flex::<()>::new(Axis::Vertical)
            .gap(dp(16.0))
            .padding(Insets::all(dp(24.0)))
            .child(Text::new("Drawer / Sidebar"))
            .child(Text::new("Open drawers from each side"))
            .child(
                Flex::<()>::new(Axis::Horizontal)
                    .gap(dp(12.0))
                    .child(left_button)
                    .child(
                        crate::ui::widget::Button::new("open right")
                            .on_click(Command::new(|_: &mut ()| {})),
                    )
                    .child(
                        crate::ui::widget::Button::new("open top")
                            .on_click(Command::new(|_: &mut ()| {})),
                    )
                    .child(
                        crate::ui::widget::Button::new("open bottom")
                            .on_click(Command::new(|_: &mut ()| {})),
                    ),
            )
            .child(
                Drawer::new(left_open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("left drawer")),
            )
            .child(
                Drawer::new(right_open.signal())
                    .placement(DrawerPlacement::Right)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("right drawer")),
            )
            .child(
                Drawer::new(top_open.signal())
                    .placement(DrawerPlacement::Top)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("top drawer")),
            )
            .child(
                Drawer::new(bottom_open.signal())
                    .placement(DrawerPlacement::Bottom)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("bottom drawer")),
            ),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 800.0, 632.0),
        None,
        None,
        None,
        None,
        false,
    );
    let button_rect = computed
        .hit_regions
        .iter()
        .find_map(|hit| match &hit.interaction {
            HitInteraction::Widget { id, .. } if *id == left_button_id => Some(hit.rect),
            _ => None,
        })
        .expect("left toolbar button should have a hit region");
    let point = Point::new(
        button_rect.x + button_rect.width * 0.5,
        button_rect.y + button_rect.height * 0.5,
    );
    let hit = WidgetTree::hit_path_from_computed(&computed, point).pop();

    assert!(
        matches!(hit, Some(HitInteraction::Widget { id, .. }) if id == left_button_id),
        "closed sibling drawers should leave the toolbar button clickable"
    );
}

#[test]
fn closed_signal_drawer_does_not_register_focus_trap() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let open = context.state(false);
    let mut animations = AnimationEngine::default();

    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(300.0), dp(200.0))
            .child(crate::ui::widget::Button::new("open").size(dp(120.0), dp(40.0)))
            .child(
                Drawer::new(open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("drawer")),
            ),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 300.0, 200.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        computed
            .focus_scopes
            .iter()
            .all(|scope| !scope.active || !scope.options.is_trap()),
        "closed drawer must not install an active focus trap"
    );
}

#[test]
fn open_signal_drawer_registers_focus_trap() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let open = context.state(true);
    let mut animations = AnimationEngine::default();

    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(300.0), dp(200.0))
            .child(crate::ui::widget::Button::new("open").size(dp(120.0), dp(40.0)))
            .child(
                Drawer::new(open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
                    .content(Text::new("drawer")),
            ),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 300.0, 200.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(
        computed
            .focus_scopes
            .iter()
            .any(|scope| scope.active && scope.options.is_trap()),
        "open drawer should trap focus inside the panel"
    );
}

#[test]
fn drawer_host_push_attaches_push_descriptor_without_backdrop_hits() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let element: Element<()> = DrawerHost::new(
        Text::new("main"),
        Drawer::new(true)
            .mode(DrawerMode::Push)
            .placement(DrawerPlacement::Left)
            .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
            .content(Text::new("drawer")),
    )
    .size(dp(400.0), dp(240.0))
    .into();
    let descriptor = element.drawer.as_ref().expect("push host descriptor");
    assert_eq!(descriptor.mode, DrawerMode::Push);
    assert!(!descriptor.close_on_backdrop_click);
    let root_id = element.id;

    let tree = WidgetTree::new(element);
    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 400.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(computed.overlay_hit_regions.is_empty());
    assert!(computed
        .overlay_close_handlers
        .iter()
        .any(|handler| handler.source_widget_id == Some(root_id)));
}
