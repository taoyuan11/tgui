use super::*;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    DrawerHost, DrawerMode, DrawerStyle, Flex, ResolvedElement, ResolvedSceneLayout,
};

fn resolved_drawer_panel<'a>(
    layout: &'a ResolvedSceneLayout<()>,
    mode: DrawerMode,
    placement: DrawerPlacement,
) -> &'a ResolvedElement<()> {
    let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
        panic!("drawer root should remain a container");
    };
    match mode {
        DrawerMode::Overlay => &children[1],
        DrawerMode::Push => match placement {
            DrawerPlacement::Left | DrawerPlacement::Top => &children[0],
            DrawerPlacement::Right | DrawerPlacement::Bottom => &children[1],
        },
    }
}

#[test]
fn drawer_density_geometry_reaches_real_scene_primitives() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);

    for (density, expected_width) in [
        (Density::Compact, dp(264.0)),
        (Density::Spacious, dp(320.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let drawer_style = DrawerStyle::default_for_density(&theme, density);
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Stack::<()>::new()
                .size(viewport.width, viewport.height)
                .child(
                    Drawer::new(true)
                        .placement(DrawerPlacement::Left)
                        .style_full(move |_| drawer_style.clone())
                        .content(Text::new("Density-aware drawer")),
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
            viewport,
            None,
            None,
            None,
            None,
            false,
        );

        let panel = computed
            .scene
            .shapes
            .iter()
            .find(|shape| {
                shape.color == theme.colors.outline_muted
                    && (shape.rect.width - expected_width).abs() <= dp(0.1)
                    && (shape.rect.height - viewport.height).abs() <= dp(0.1)
            })
            .unwrap_or_else(|| {
                panic!(
                    "drawer panel should use density width {expected_width:?}; shapes={:?}",
                    computed
                        .scene
                        .shapes
                        .iter()
                        .map(|shape| (shape.rect, shape.color, shape.corner_radius))
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(panel.corner_radius, 0.0);
        assert_eq!(panel.rect.x, Dp::ZERO);
    }
}

#[test]
fn drawer_overlay_and_push_runtime_geometry_tracks_all_placements() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);

    for mode in [DrawerMode::Overlay, DrawerMode::Push] {
        for placement in [
            DrawerPlacement::Left,
            DrawerPlacement::Right,
            DrawerPlacement::Top,
            DrawerPlacement::Bottom,
        ] {
            let context = test_context();
            let open = context.state(true);
            let drawer = Drawer::new(open.signal())
                .mode(mode)
                .placement(placement)
                .content(Text::new("Runtime drawer"))
                .style(|style, context| match context.density {
                    Density::Compact => {
                        style.width = dp(180.0);
                        style.height = dp(120.0);
                        style.padding = Insets::all(dp(6.0));
                    }
                    Density::Comfortable => {}
                    Density::Spacious => {
                        style.width = dp(260.0);
                        style.height = dp(200.0);
                        style.padding = Insets::all(dp(18.0));
                    }
                });
            let tree: WidgetTree<()> = match mode {
                DrawerMode::Overlay => WidgetTree::new(drawer),
                DrawerMode::Push => WidgetTree::new(
                    DrawerHost::new(Text::new("Main"), drawer)
                        .size(viewport.width, viewport.height),
                ),
            };

            for (density, width, height, padding) in [
                (Density::Compact, dp(180.0), dp(120.0), dp(6.0)),
                (Density::Spacious, dp(260.0), dp(200.0), dp(18.0)),
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
                if mode == DrawerMode::Push {
                    assert_eq!(
                        layout.resolved_root.layout.width,
                        Some(Value::Static(crate::ui::layout::Length::Px(viewport.width)))
                    );
                    assert_eq!(
                        layout.resolved_root.layout.height,
                        Some(Value::Static(crate::ui::layout::Length::Px(
                            viewport.height
                        )))
                    );
                }
                let panel = resolved_drawer_panel(&layout, mode, placement);
                match placement {
                    DrawerPlacement::Left | DrawerPlacement::Right => assert_eq!(
                        panel.layout.width,
                        Some(Value::Static(crate::ui::layout::Length::Px(width)))
                    ),
                    DrawerPlacement::Top | DrawerPlacement::Bottom => assert_eq!(
                        panel.layout.height,
                        Some(Value::Static(crate::ui::layout::Length::Px(height)))
                    ),
                }
                let ResolvedWidgetKind::Container {
                    layout: panel_layout,
                    ..
                } = &panel.kind
                else {
                    panic!("drawer panel should remain a container");
                };
                assert_eq!(
                    panel_layout.padding,
                    Some(Value::Static(Insets::all(padding)))
                );
            }

            open.set(false);
            let mut theme = Theme::dark();
            theme.density = Density::Spacious;
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
            let panel = resolved_drawer_panel(&layout, mode, placement);
            if mode == DrawerMode::Overlay {
                let inset = match placement {
                    DrawerPlacement::Left => panel.layout.left.as_ref(),
                    DrawerPlacement::Right => panel.layout.right.as_ref(),
                    DrawerPlacement::Top => panel.layout.top.as_ref(),
                    DrawerPlacement::Bottom => panel.layout.bottom.as_ref(),
                }
                .expect("overlay drawer should keep its animated edge inset")
                .resolve();
                let expected = match placement {
                    DrawerPlacement::Left | DrawerPlacement::Right => dp(-260.0),
                    DrawerPlacement::Top | DrawerPlacement::Bottom => dp(-200.0),
                };
                assert_eq!(inset, crate::ui::layout::Length::Px(expected));
            } else {
                let extent = match placement {
                    DrawerPlacement::Left | DrawerPlacement::Right => panel.layout.width.as_ref(),
                    DrawerPlacement::Top | DrawerPlacement::Bottom => panel.layout.height.as_ref(),
                }
                .expect("push drawer should keep its animated extent")
                .resolve();
                assert_eq!(extent, crate::ui::layout::Length::Px(Dp::ZERO));
            }
        }
    }
}

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
    stacker::grow(16 * 1024 * 1024, || {
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
    });
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
