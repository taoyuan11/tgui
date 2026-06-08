use super::*;

#[test]
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
fn mapped_dynamic_children_recompute_when_nested_state_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let page = context.state("p3".to_string());
    let label = context.state("before".to_string());
    let label_for_page = label.clone();
    let tree = WidgetTree::new(
        Stack::<()>::new().child(
            page.signal()
                .map(move |_page| -> Element<()> { Text::new(label_for_page.get()).into() }),
        ),
    );

    let mut animations = AnimationEngine::default();
    let initial = tree.render_output(
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
    let texts = initial
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(texts.contains(&"before"));

    label.set("after".to_string());
    let updated = tree.render_output(
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
    let texts = updated
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(texts.contains(&"after"));
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

fn scope_child(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.child
}

fn scope_other(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.other
}

#[test]
fn scoped_command_targets_child_view_model() {
    let child: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let root = child.scope(scope_child);

    let command = root.interactions.on_click.expect("scoped command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.root_count, 0);
}

#[test]
fn scoped_context_command_receives_child_context() {
    let command = Command::new_with_context(
        |vm: &mut ScopeChildVm, _ctx: &CommandContext<ScopeChildVm>| {
            vm.context_hits += 1;
        },
    )
    .scope(std::sync::Arc::new(scope_child));

    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.context_hits, 1);
}

#[test]
fn scoped_lifecycle_command_targets_child_view_model() {
    let child: Element<ScopeChildVm> = Stack::new()
        .on_mount(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let root = child.scope(scope_child);

    let command = root
        .lifecycle_events
        .on_mount
        .expect("scoped lifecycle command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);

    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.root_count, 0);
}
