use super::*;
use std::time::Duration;

use crate::animation::{AnimationKey, WidgetProperty};
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    ComputedScene, DrawerHost, DrawerMode, DrawerStyle, Flex, ResolvedElement, ResolvedSceneLayout,
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

fn drawer_scene_at(
    tree: &WidgetTree<()>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    reduced_motion: bool,
    viewport: Rect,
    now: Instant,
) -> ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        font_manager,
        theme,
        media,
        UnitContext::default(),
        animations,
        reduced_motion,
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
        now,
    )
}

fn shape_rect_with_color(scene: &ComputedScene<()>, color: Color) -> Rect {
    scene
        .scene
        .shapes
        .iter()
        .find(|shape| shape.color == color)
        .unwrap_or_else(|| panic!("missing drawer panel color {color:?}"))
        .rect
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
                assert_eq!(inset, crate::ui::layout::Length::Px(Dp::ZERO));
                let expected = match placement {
                    DrawerPlacement::Left => Point::new(dp(-260.0), Dp::ZERO),
                    DrawerPlacement::Right => Point::new(dp(260.0), Dp::ZERO),
                    DrawerPlacement::Top => Point::new(Dp::ZERO, dp(-200.0)),
                    DrawerPlacement::Bottom => Point::new(Dp::ZERO, dp(200.0)),
                };
                assert_eq!(panel.visual.offset.resolve(), expected);
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
fn overlay_drawer_motion_is_scene_only_theme_timed_and_reduced_motion_safe() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);
    let panel_color = Color::rgba(17, 123, 201, 255);
    let context = test_context();
    let open = context.state(false);
    let inside: Element<()> = crate::ui::widget::Button::new("inside").into();
    let inside_id = inside.id;
    let drawer: Element<()> = Drawer::new(open.signal())
        .placement(DrawerPlacement::Left)
        .content(inside)
        .style(move |style, _| {
            style.width = dp(200.0);
            style.background = Value::Static(panel_color);
            style.border = Value::Static(Color::TRANSPARENT);
            style.border_width = Value::Static(Dp::ZERO);
        })
        .into();
    let panel_id = Drawer::_panel_id_of(&drawer).expect("drawer panel id");
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(viewport.width, viewport.height)
            .child(drawer),
    );
    let mut theme = Theme::light();
    theme.motion.slow_ms = 240;
    let start = Instant::now();
    let mut animations = AnimationEngine::default();

    let _closed = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        start,
    );
    open.set(true);
    let animation_start = start + Duration::from_millis(1);
    let _opening_start = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        animation_start,
    );
    let offset_key = AnimationKey::Widget {
        id: panel_id.raw(),
        property: WidgetProperty::Offset,
    };
    assert!(animations.contains_key(offset_key));

    let mid = animation_start + Duration::from_millis(120);
    let refresh = animations.refresh(mid);
    assert!(refresh.changed);
    assert!(
        !refresh.layout_changed,
        "overlay drawer translation must not force layout on every animation frame"
    );
    assert!(refresh.scene_widget_ids.contains(&panel_id.raw()));
    let opening_mid = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        mid,
    );
    let mid_rect = shape_rect_with_color(&opening_mid, panel_color);
    assert!(mid_rect.x > dp(-200.0) && mid_rect.x < Dp::ZERO);

    let almost_done = animation_start + Duration::from_millis(239);
    let _almost_done = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        almost_done,
    );
    assert!(animations.has_active_animations());
    let settled = animation_start + Duration::from_millis(241);
    let opened = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        settled,
    );
    assert_eq!(shape_rect_with_color(&opened, panel_color).x, Dp::ZERO);
    assert!(!animations.has_active_animations());

    open.set(false);
    let close_start = settled + Duration::from_millis(1);
    let closing = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        close_start,
    );
    assert!(closing.hit_regions.iter().all(|hit| {
        !matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == inside_id)
    }));
    assert!(animations.has_active_animations());

    let reduced = drawer_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        viewport,
        close_start + Duration::from_millis(1),
    );
    assert_eq!(
        shape_rect_with_color(&reduced, panel_color).x,
        dp(-200.0),
        "reduced motion should land the closing drawer off-screen immediately"
    );
    assert!(!animations.has_active_animations());
}

#[test]
fn overlay_drawer_position_is_identical_at_120_and_144_hz() {
    fn sampled_x(frame_interval: Duration) -> Dp {
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);
        let panel_color = Color::rgba(31, 149, 91, 255);
        let context = test_context();
        let open = context.state(false);
        let tree: WidgetTree<()> = WidgetTree::new(
            Stack::<()>::new()
                .size(viewport.width, viewport.height)
                .child(
                    Drawer::new(open.signal())
                        .placement(DrawerPlacement::Left)
                        .style(move |style, _| {
                            style.width = dp(200.0);
                            style.background = Value::Static(panel_color);
                            style.border = Value::Static(Color::TRANSPARENT);
                            style.border_width = Value::Static(Dp::ZERO);
                        }),
                ),
        );
        let mut theme = Theme::light();
        theme.motion.slow_ms = 240;
        let start = Instant::now();
        let animation_start = start + Duration::from_millis(1);
        let target_elapsed = Duration::from_millis(120);
        let mut animations = AnimationEngine::default();
        let _ = drawer_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            false,
            viewport,
            start,
        );
        open.set(true);
        let _ = drawer_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            false,
            viewport,
            animation_start,
        );
        let mut elapsed = frame_interval;
        while elapsed < target_elapsed {
            let _ = drawer_scene_at(
                &tree,
                &theme,
                &font_manager,
                &media,
                &mut animations,
                false,
                viewport,
                animation_start + elapsed,
            );
            elapsed += frame_interval;
        }
        let sampled = drawer_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            false,
            viewport,
            animation_start + target_elapsed,
        );
        shape_rect_with_color(&sampled, panel_color).x
    }

    let at_120_hz = sampled_x(Duration::from_secs_f64(1.0 / 120.0));
    let at_144_hz = sampled_x(Duration::from_secs_f64(1.0 / 144.0));
    assert!(
        (at_120_hz - at_144_hz).abs() <= dp(0.001),
        "same absolute timestamp must not depend on refresh cadence: {at_120_hz:?} vs {at_144_hz:?}"
    );
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
