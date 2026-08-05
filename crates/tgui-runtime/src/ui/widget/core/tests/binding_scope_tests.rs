use super::*;

#[test]
fn mapped_dynamic_children_recompute_when_nested_state_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let page = context.state("p3".to_string());
    let label = context.state("before".to_string());
    let label_for_page = label.clone();
    let tree =
        WidgetTree::new_legacy(
            Stack::<()>::new().dynamic_child(page.signal().map_unchecked(
                move |_page| -> Element<()> { Text::new(label_for_page.get()).into() },
            )),
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

#[derive(Default)]
struct ScopeChildVm {
    count: i32,
    context_hits: usize,
}

#[derive(Default)]
struct ScopeRootVm {
    child: ScopeChildVm,
    root_count: i32,
}

fn scope_child(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
    &mut root.child
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

#[test]
fn scoped_show_and_view_switch_retain_commands_in_boxed_children() {
    use crate::ui::widget::common::ChildSource;
    use crate::ui::widget::{Show, ViewSwitch};

    let child: Element<ScopeChildVm> = Stack::new()
        .child(Show::new(
            true,
            Stack::new().on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1)),
        ))
        .child(
            ViewSwitch::new(0)
                .case(Stack::new().on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 10)))
                .fallback(
                    Stack::new().on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 100)),
                ),
        )
        .into();
    let scoped = child.scope(scope_child);
    let WidgetKind::Container { children, .. } = scoped.kind else {
        panic!("scoped stack should remain a container");
    };
    let mut sources = children.into_iter();
    let ChildSource::Show { child, .. } = sources.next().expect("show source") else {
        panic!("first scoped source should remain Show");
    };
    let ChildSource::Switch {
        mut cases,
        fallback,
        ..
    } = sources.next().expect("switch source")
    else {
        panic!("second scoped source should remain Switch");
    };
    assert!(sources.next().is_none());

    let show_command = child.interactions.on_click.expect("scoped Show command");
    let case_command = cases
        .remove(0)
        .interactions
        .on_click
        .expect("scoped Switch case command");
    let fallback_command = fallback
        .expect("boxed Switch fallback")
        .interactions
        .on_click
        .expect("scoped Switch fallback command");
    let mut vm = ScopeRootVm::default();
    show_command.execute(&mut vm);
    case_command.execute(&mut vm);
    fallback_command.execute(&mut vm);

    assert_eq!(vm.child.count, 111);
    assert_eq!(vm.root_count, 0);
}
