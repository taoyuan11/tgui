use super::*;

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
