use super::*;

use crate::ui::layout::{Length, Value};
use crate::ui::widget::{Collapse, ResolvedElement, ResolvedSceneLayout};

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
