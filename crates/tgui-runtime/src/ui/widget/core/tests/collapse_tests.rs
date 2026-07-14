use super::*;

use std::time::Duration;

use crate::animation::Transition;
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{
    Accordion, AccordionItem, Collapse, CollapseStyle, ResolvedElement, ResolvedSceneLayout,
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
    assert!(matches!(
        children[1].layout.max_height,
        Some(Value::Static(Length::Px(value))) if value == Dp::ZERO
    ));
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
