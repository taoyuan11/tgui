use super::*;

use std::time::Duration;

use crate::animation::Transition;
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::icon::SvgIconId;
use crate::ui::widget::{
    Accordion, AccordionItem, Button, Collapse, CollapseStyle, ComputedScene, ResolvedElement,
    ResolvedSceneLayout, WidgetId,
};

fn resolved_children<VM>(element: &ResolvedElement<VM>) -> &[ResolvedElement<VM>] {
    match &element.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    }
}

fn subtree_has_button<VM>(element: &ResolvedElement<VM>) -> bool {
    matches!(element.kind, ResolvedWidgetKind::Button { .. })
        || resolved_children(element).iter().any(subtree_has_button)
}

fn subtree_has_text<VM>(element: &ResolvedElement<VM>, expected: &str) -> bool {
    match &element.kind {
        ResolvedWidgetKind::Text { text, .. } if text.content.resolve() == expected => true,
        _ => resolved_children(element)
            .iter()
            .any(|child| subtree_has_text(child, expected)),
    }
}

fn subtree_icon_source<VM>(element: &ResolvedElement<VM>) -> Option<SvgIconId> {
    match &element.kind {
        ResolvedWidgetKind::Icon { icon } => Some(icon.source),
        _ => resolved_children(element)
            .iter()
            .find_map(subtree_icon_source),
    }
}

fn resolved_max_height<VM>(element: &ResolvedElement<VM>) -> Option<Length> {
    element.layout.max_height.as_ref().map(Value::resolve)
}

fn collapse_scene_at(
    tree: &WidgetTree<()>,
    animations: &mut AnimationEngine,
    now: Instant,
) -> ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        &FontManager::new(&FontCatalog::default()),
        &Theme::default(),
        &test_media(),
        UnitContext::default(),
        animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 640.0, 720.0),
        None,
        None,
        None,
        None,
        false,
        now,
    )
}

fn widget_keyboard_activation(
    scene: &ComputedScene<()>,
    widget_id: WidgetId,
) -> Option<(bool, bool)> {
    scene.hit_regions.iter().find_map(|region| {
        let (id, enter, space) = region.interaction.keyboard_activation()?;
        (id == widget_id).then_some((enter, space))
    })
}

fn scene_has_widget_hit(scene: &ComputedScene<()>, widget_id: WidgetId) -> bool {
    scene
        .hit_regions
        .iter()
        .any(|region| region.interaction.widget_id() == widget_id)
}

fn collapse_layout(expanded: bool) -> ResolvedSceneLayout<()> {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Collapse::new("Runtime notes", Text::new("Panel content")).expanded(expanded),
    );

    tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 200.0),
    )
}

#[test]
fn collapse_runtime_layout_tracks_real_theme_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Collapse::new("Runtime notes", Text::new("Panel content")).expanded(true));

    for (density, expected_height, expected_padding) in [
        (
            Density::Compact,
            dp(32.0),
            Insets::symmetric(dp(8.0), dp(4.0)),
        ),
        (
            Density::Comfortable,
            dp(40.0),
            Insets::symmetric(dp(12.0), dp(8.0)),
        ),
        (
            Density::Spacious,
            dp(48.0),
            Insets::symmetric(dp(16.0), dp(12.0)),
        ),
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
            Rect::new(0.0, 0.0, 320.0, 200.0),
        );
        let header = &resolved_children(&layout.resolved_root)[0];
        let header_bounds = layout
            .widget_bounds(header.id)
            .expect("collapse header should have bounds");
        assert_eq!(header_bounds.height, expected_height);
        let ResolvedWidgetKind::Container {
            layout: header_layout,
            ..
        } = &header.kind
        else {
            panic!("collapse header should resolve to a container");
        };
        assert_eq!(
            header_layout.padding,
            Some(Value::Static(expected_padding)),
            "header padding should be resolved from the active theme density"
        );
    }
}

#[test]
fn collapse_light_and_dark_themes_reach_real_scene_surfaces() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Collapse::new("Runtime notes", Text::new("Panel content")).expanded(true));

    for theme in [Theme::light(), Theme::dark()] {
        let mut animations = AnimationEngine::default();
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 200.0),
            None,
            None,
            None,
            None,
            false,
        );
        let style = CollapseStyle::default_for_theme(&theme);
        assert!(
            rendered
                .primitives
                .shapes
                .iter()
                .any(|shape| shape.color == style.panel_background.resolve()),
            "panel surface should use the active {} theme",
            theme.name
        );
        assert!(
            rendered.primitives.shapes.iter().any(|shape| {
                shape.color == style.border.resolve()
                    && (shape.stroke_width - style.border_width.get()).abs() < f32::EPSILON
            }),
            "collapse border should use the active {} theme",
            theme.name
        );
    }
}

#[test]
fn accordion_gap_and_item_geometry_resolve_from_runtime_density() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Accordion::new(
        vec![
            AccordionItem::new("usage", "Usage", Text::new("Usage panel")),
            AccordionItem::new("theme", "Theme", Text::new("Theme panel")),
        ],
        Some("usage".to_string()),
    ));
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
        Rect::new(0.0, 0.0, 320.0, 280.0),
    );
    let ResolvedWidgetKind::Container {
        layout: accordion_layout,
        children,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("accordion root should resolve to a container");
    };
    let expected = CollapseStyle::default_for_theme(&theme);
    assert_eq!(
        accordion_layout.gap,
        Value::Static(Length::Px(expected.gap))
    );
    let first_header = &resolved_children(&children[0])[0];
    assert_eq!(
        layout
            .widget_bounds(first_header.id)
            .expect("accordion header should have bounds")
            .height,
        dp(48.0)
    );
}

#[test]
fn collapse_component_theme_geometry_is_resolved_at_runtime() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Collapse::new("Runtime notes", Text::new("Panel content")).expanded(true));
    let mut theme = Theme::light();
    theme.components = crate::ui::theme::ComponentThemes::default().collapse(|style, _| {
        style.header_min_height = dp(44.0);
        style.padding = Insets::symmetric(dp(18.0), dp(10.0));
        style.gap = dp(14.0);
    });
    let mut animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 220.0),
    );
    let children = resolved_children(&layout.resolved_root);
    assert_eq!(
        layout
            .widget_bounds(children[0].id)
            .expect("collapse header should have bounds")
            .height,
        dp(44.0)
    );
    let ResolvedWidgetKind::Container {
        layout: panel_layout,
        ..
    } = &children[1].kind
    else {
        panic!("collapse panel should resolve to a container");
    };
    assert_eq!(
        panel_layout.padding,
        Some(Value::Static(Insets::symmetric(dp(18.0), dp(10.0))))
    );
}

#[test]
fn collapse_header_resolves_as_clickable_container_not_button() {
    let layout = collapse_layout(true);
    let children = resolved_children(&layout.resolved_root);

    assert_eq!(children.len(), 2);
    assert!(matches!(
        children[0].kind,
        ResolvedWidgetKind::Container { .. }
    ));
    assert!(children[0].interactions.on_click.is_some());
    assert!(
        !subtree_has_button(&children[0]),
        "collapse title should not be rendered as a Button"
    );
}

#[test]
fn collapsed_collapse_keeps_panel_content_for_exit_animation() {
    let layout = collapse_layout(false);
    let children = resolved_children(&layout.resolved_root);

    assert_eq!(children.len(), 2);
    assert!(
        subtree_has_text(&children[1], "Panel content"),
        "collapsed panel content should stay mounted so close transitions can render"
    );
    assert_eq!(
        resolved_max_height(&children[1]),
        Some(Length::Px(Dp::ZERO))
    );
}

#[test]
fn collapse_header_and_panel_are_flush() {
    let layout = collapse_layout(true);
    let children = resolved_children(&layout.resolved_root);

    assert_eq!(children.len(), 2);

    let header = layout
        .widget_bounds(children[0].id)
        .expect("header should have layout bounds");
    let panel = layout
        .widget_bounds(children[1].id)
        .expect("panel should have layout bounds");

    assert_eq!(
        header.bottom(),
        panel.y,
        "collapse trigger and panel should not leave a hover gap"
    );
}

#[test]
fn collapse_header_click_reads_latest_signal_state() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let open = context.state(true);
    let open_for_command = open.clone();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Collapse::new("Runtime notes", Text::new("Panel content"))
            .expanded(
                open.signal()
                    .animated(Transition::ease_in_out(Duration::from_millis(180))),
            )
            .on_change(ValueCommand::new(move |_: &mut (), next| {
                open_for_command.set(next);
            })),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 200.0),
    );
    let command = resolved_children(&layout.resolved_root)[0]
        .interactions
        .on_click
        .clone()
        .expect("collapse header should be clickable");
    let mut vm = ();

    command.execute(&mut vm);
    assert!(!open.get(), "first click should close the panel");

    command.execute(&mut vm);
    assert!(open.get(), "second click should reopen the panel");
}

#[test]
fn accordion_header_click_reads_latest_signal_state() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let expanded_key = context.state(Some("usage".to_string()));
    let expanded_key_for_command = expanded_key.clone();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Accordion::new(
            vec![
                AccordionItem::new("usage", "Usage", Text::new("Usage panel")),
                AccordionItem::new("theme", "Theme", Text::new("Theme panel")),
            ],
            expanded_key
                .signal()
                .animated(Transition::ease_in_out(Duration::from_millis(180))),
        )
        .on_change(ValueCommand::new(move |_: &mut (), next| {
            expanded_key_for_command.set(next);
        })),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 240.0),
    );
    let first_item = &resolved_children(&layout.resolved_root)[0];
    let command = resolved_children(first_item)[0]
        .interactions
        .on_click
        .clone()
        .expect("accordion header should be clickable");
    let mut vm = ();

    command.execute(&mut vm);
    assert_eq!(
        expanded_key.get(),
        None,
        "first click should close the item"
    );

    command.execute(&mut vm);
    assert_eq!(
        expanded_key.get(),
        Some("usage".to_string()),
        "second click should reopen the item"
    );
}

#[test]
fn uncontrolled_collapse_toggles_panel_and_icon_without_a_callback() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Collapse::new("Runtime notes", Text::new("Panel content")));
    let build = |animations: &mut AnimationEngine| {
        tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 240.0),
        )
    };

    let initial = build(&mut animations);
    let initial_children = resolved_children(&initial.resolved_root);
    assert_eq!(
        resolved_max_height(&initial_children[1]),
        Some(Length::Px(Dp::ZERO))
    );
    assert_eq!(
        subtree_icon_source(&initial_children[0]),
        Some(SvgIconId::ChevronDown)
    );
    let command = initial_children[0]
        .interactions
        .on_click
        .clone()
        .expect("default collapse header should toggle itself");
    let mut vm = ();

    command.execute(&mut vm);
    let expanded = build(&mut animations);
    let expanded_children = resolved_children(&expanded.resolved_root);
    assert_eq!(
        resolved_max_height(&expanded_children[1]),
        Some(Length::Auto)
    );
    assert_eq!(
        subtree_icon_source(&expanded_children[0]),
        Some(SvgIconId::ChevronUp)
    );

    command.execute(&mut vm);
    let collapsed = build(&mut animations);
    let collapsed_children = resolved_children(&collapsed.resolved_root);
    assert_eq!(
        resolved_max_height(&collapsed_children[1]),
        Some(Length::Px(Dp::ZERO))
    );
    assert_eq!(
        subtree_icon_source(&collapsed_children[0]),
        Some(SvgIconId::ChevronDown)
    );
}

#[test]
fn collapse_signal_updates_panel_and_icon_on_the_same_tree() {
    let context = test_context();
    let expanded = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(
        Collapse::new("Runtime notes", Text::new("Panel content")).expanded(expanded.signal()),
    );
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let build = |animations: &mut AnimationEngine| {
        tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 240.0),
        )
    };

    let initial = build(&mut animations);
    let initial_children = resolved_children(&initial.resolved_root);
    assert_eq!(
        resolved_max_height(&initial_children[1]),
        Some(Length::Px(Dp::ZERO))
    );
    assert_eq!(
        subtree_icon_source(&initial_children[0]),
        Some(SvgIconId::ChevronDown)
    );

    expanded.set(true);
    let updated = build(&mut animations);
    let updated_children = resolved_children(&updated.resolved_root);
    assert_eq!(
        resolved_max_height(&updated_children[1]),
        Some(Length::Auto)
    );
    assert_eq!(
        subtree_icon_source(&updated_children[0]),
        Some(SvgIconId::ChevronUp)
    );
}

#[test]
fn collapse_does_not_clip_content_taller_than_legacy_panel_limit() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Collapse::new(
            "Long panel",
            Stack::new().height(dp(480.0)).child(Text::new("bottom")),
        )
        .expanded(true),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 640.0, 720.0),
    );
    let panel = &resolved_children(&layout.resolved_root)[1];
    let panel_bounds = layout
        .widget_bounds(panel.id)
        .expect("expanded panel bounds");

    assert_eq!(resolved_max_height(panel), Some(Length::Auto));
    assert!(
        panel_bounds.height > dp(320.0),
        "long content must not be clipped by an arbitrary 320dp cap: {panel_bounds:?}"
    );
}

#[test]
fn uncontrolled_accordion_switches_and_closes_items_without_a_callback() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Accordion::new(
        vec![
            AccordionItem::new("usage", "Usage", Text::new("Usage panel")),
            AccordionItem::new("theme", "Theme", Text::new("Theme panel")),
        ],
        Some("usage".to_string()),
    ));
    let build = |animations: &mut AnimationEngine| {
        tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 360.0),
        )
    };

    let initial = build(&mut animations);
    let items = resolved_children(&initial.resolved_root);
    assert_eq!(
        resolved_max_height(&resolved_children(&items[0])[1]),
        Some(Length::Auto)
    );
    assert_eq!(
        resolved_max_height(&resolved_children(&items[1])[1]),
        Some(Length::Px(Dp::ZERO))
    );
    let first_command = resolved_children(&items[0])[0]
        .interactions
        .on_click
        .clone()
        .expect("first accordion trigger");
    let second_command = resolved_children(&items[1])[0]
        .interactions
        .on_click
        .clone()
        .expect("second accordion trigger");
    let mut vm = ();

    second_command.execute(&mut vm);
    let switched = build(&mut animations);
    let items = resolved_children(&switched.resolved_root);
    assert_eq!(
        resolved_max_height(&resolved_children(&items[0])[1]),
        Some(Length::Px(Dp::ZERO))
    );
    assert_eq!(
        resolved_max_height(&resolved_children(&items[1])[1]),
        Some(Length::Auto)
    );

    second_command.execute(&mut vm);
    let closed = build(&mut animations);
    let items = resolved_children(&closed.resolved_root);
    assert_eq!(
        resolved_max_height(&resolved_children(&items[1])[1]),
        Some(Length::Px(Dp::ZERO))
    );

    first_command.execute(&mut vm);
    let reopened = build(&mut animations);
    let items = resolved_children(&reopened.resolved_root);
    assert_eq!(
        resolved_max_height(&resolved_children(&items[0])[1]),
        Some(Length::Auto)
    );
}

#[test]
fn collapse_keyboard_activation_and_disabled_signal_follow_live_state() {
    let context = test_context();
    let disabled = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(
        Collapse::new("Runtime notes", Text::new("Panel content")).disabled(disabled.signal()),
    );
    let mut animations = AnimationEngine::default();
    let start = Instant::now();
    let enabled_scene = collapse_scene_at(&tree, &mut animations, start);
    let header_id = enabled_scene
        .hit_regions
        .iter()
        .find_map(|region| {
            region
                .interaction
                .keyboard_activation()
                .map(|(id, _, _)| id)
        })
        .expect("enabled collapse trigger should be focusable");
    assert_eq!(
        widget_keyboard_activation(&enabled_scene, header_id),
        Some((true, true)),
        "Enter and Space should both activate a collapse trigger"
    );

    disabled.set(true);
    let disabled_scene =
        collapse_scene_at(&tree, &mut animations, start + Duration::from_millis(1));
    assert_eq!(widget_keyboard_activation(&disabled_scene, header_id), None);
    assert!(
        disabled_scene
            .hit_regions
            .iter()
            .all(|region| region.interaction.widget_id() != header_id),
        "disabled trigger must not receive pointer input"
    );

    disabled.set(false);
    let reenabled_scene =
        collapse_scene_at(&tree, &mut animations, start + Duration::from_millis(2));
    assert_eq!(
        widget_keyboard_activation(&reenabled_scene, header_id),
        Some((true, true))
    );
}

#[test]
fn collapsed_panel_releases_descendant_pointer_and_keyboard_input_immediately() {
    let child: Element<()> = Button::new("Nested action")
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let child_id = child.id;
    let tree: WidgetTree<()> = WidgetTree::new(Collapse::new("Actions", child));
    let mut animations = AnimationEngine::default();
    let start = Instant::now();
    let collapsed = collapse_scene_at(&tree, &mut animations, start);
    assert!(!scene_has_widget_hit(&collapsed, child_id));
    assert_eq!(widget_keyboard_activation(&collapsed, child_id), None);

    let layout = collapse_layout_for_tree(&tree, &mut animations);
    let toggle = resolved_children(&layout.resolved_root)[0]
        .interactions
        .on_click
        .clone()
        .expect("collapse trigger");
    toggle.execute(&mut ());
    let expanded = collapse_scene_at(&tree, &mut animations, start + Duration::from_millis(1));
    assert!(scene_has_widget_hit(&expanded, child_id));
    assert_eq!(
        widget_keyboard_activation(&expanded, child_id),
        Some((true, true))
    );
}

fn collapse_layout_for_tree(
    tree: &WidgetTree<()>,
    animations: &mut AnimationEngine,
) -> ResolvedSceneLayout<()> {
    tree.build_scene_layout(
        &FontManager::new(&FontCatalog::default()),
        &Theme::default(),
        &test_media(),
        animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 640.0, 720.0),
    )
}
