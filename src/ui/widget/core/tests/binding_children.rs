use super::*;

fn binding_driven_children_relayout_when_child_count_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let expanded = context.state(false);
    let tree = WidgetTree::new(Stack::<()>::new().child(expanded.signal().map(|value| {
        if value {
            vec![
                Element::from(Text::new("first")),
                Element::from(Text::new("second")),
            ]
        } else {
            vec![Element::from(Text::new("first"))]
        }
    })));

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
    let tree = WidgetTree::new(Stack::<()>::new().size(dp(100.0), dp(100.0)).child(
        visible.signal().map(move |value| {
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

#[derive(Default)]
struct ScopeChildVm {
    count: i32,
    checked: bool,
    selected_key: String,
    selected_value: String,
    canvas_hits: usize,
    context_hits: usize,
}

#[derive(Default)]
struct ScopeRootVm {
    child: ScopeChildVm,
    other: ScopeChildVm,
    root_count: i32,
}

