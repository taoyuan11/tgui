use super::*;

use std::time::Duration;

use crate::animation::Transition;
use crate::ui::layout::{Length, Value};
use crate::ui::widget::{Accordion, AccordionItem, Collapse, ResolvedElement, ResolvedSceneLayout};

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
