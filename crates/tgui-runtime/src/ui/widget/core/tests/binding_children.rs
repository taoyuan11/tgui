use super::*;
use crate::ui::widget::StrictReactiveViolation;
use crate::ui::widget::{For, Show, ViewSwitch};

#[test]
fn strict_reactive_tree_accepts_static_children() {
    let tree = WidgetTree::try_new_strict(Stack::<()>::new().child(Text::new("static")));

    assert!(tree.is_ok());
}

#[test]
fn strict_reactive_tree_accepts_retained_show_children() {
    let context = test_context();
    let visible = context.state(true);
    let tree = WidgetTree::try_new_strict(
        Stack::<()>::new().child(Show::new(visible.signal(), Text::new("shown"))),
    );

    assert!(tree.is_ok());
}

#[test]
fn strict_reactive_tree_accepts_retained_keyed_for_children() {
    let context = test_context();
    let items = context.state(vec![1usize, 2, 3]);
    let tree = WidgetTree::try_new_strict(Stack::<()>::new().child(For::new(
        items.signal(),
        |item| *item,
        |_index, item| Text::new(format!("item {item}")),
    )));

    assert!(tree.is_ok());
}

#[test]
fn strict_reactive_tree_accepts_retained_view_switch_children() {
    let context = test_context();
    let active = context.state(0usize);
    let tree = WidgetTree::try_new_strict(
        Stack::<()>::new().child(
            ViewSwitch::new(active.signal())
                .case(Text::new("first"))
                .case(Text::new("second"))
                .fallback(Text::new("fallback")),
        ),
    );

    assert!(tree.is_ok());
}

#[test]
fn strict_reactive_tree_rejects_signal_driven_children() {
    let context = test_context();
    let expanded = context.state(false);
    let tree = WidgetTree::try_new_strict(Stack::<()>::new().dynamic_child(
        expanded.signal().map_unchecked(|value| {
            if value {
                vec![
                    Element::from(Text::new("first")),
                    Element::from(Text::new("second")),
                ]
            } else {
                vec![Element::from(Text::new("first"))]
            }
        }),
    ));

    assert!(matches!(
        tree,
        Err(StrictReactiveViolation::DynamicChildren)
    ));
}

#[test]
fn binding_driven_children_relayout_when_child_count_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let expanded = context.state(false);
    let tree = WidgetTree::new_legacy(Stack::<()>::new().dynamic_child(
        expanded.signal().map_unchecked(|value| {
            if value {
                vec![
                    Element::from(Text::new("first")),
                    Element::from(Text::new("second")),
                ]
            } else {
                vec![Element::from(Text::new("first"))]
            }
        }),
    ));

    let mut animations = AnimationEngine::default();
    let compact = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(compact.primitives.texts.len(), 1);

    expanded.set(true);
    let expanded_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(expanded_render.primitives.texts.len(), 2);
}

#[test]
fn hit_testing_tracks_currently_resolved_children() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let visible = context.state(true);
    let clickable: Element<()> = Stack::new()
        .size(dp(40.0), dp(40.0))
        .style_full(|ctx| {
            container_style(
                ctx,
                Some(crate::foundation::color::Color::WHITE),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        })
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let tree = WidgetTree::new_legacy(Stack::<()>::new().size(dp(100.0), dp(100.0)).dynamic_child(
        visible.signal().map_unchecked(move |value| {
            if value {
                vec![clickable.clone()]
            } else {
                Vec::<Element<()>>::new()
            }
        }),
    ));

    let mut animations = AnimationEngine::default();
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Widget { .. })));

    visible.set(false);
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(hit.is_none());
}
