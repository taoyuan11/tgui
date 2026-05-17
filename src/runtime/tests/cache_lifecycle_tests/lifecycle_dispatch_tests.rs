use super::*;

#[test]
fn lifecycle_mount_dispatches_once_without_update() {
    let invalidation = InvalidationSignal::new();
    invalidation.mark_dirty();
    let tree = WidgetTree::new(
        Text::new("hello")
            .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
            .on_update(Command::new(|vm: &mut LifecycleVm| vm.updates += 1)),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    dispatch_lifecycle_if_dirty(&mut handler);

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.mounts, 1);
    assert_eq!(vm.updates, 0);
    assert_eq!(vm.unmounts, 0);
}

#[test]
fn lifecycle_mount_command_invalidation_does_not_remount_same_component() {
    let invalidation = InvalidationSignal::new();
    invalidation.mark_dirty();
    let tree = WidgetTree::new(
        Text::new("hello")
            .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
            .on_update(Command::new(|vm: &mut LifecycleVm| vm.updates += 1)),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    dispatch_lifecycle_if_dirty(&mut handler);
    dispatch_lifecycle_if_dirty(&mut handler);

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.mounts, 1);
    assert_eq!(vm.updates, 0);
    assert_eq!(vm.unmounts, 0);
}

#[test]
fn lifecycle_update_command_without_state_change_does_not_loop() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let label = context.state(String::from("first"));
    let tree = WidgetTree::new(
        Text::new(label.signal())
            .key("stable")
            .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
            .on_update(Command::new(|vm: &mut LifecycleVm| vm.updates += 1)),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    label.set(String::from("second"));
    dispatch_lifecycle_if_dirty(&mut handler);
    dispatch_lifecycle_if_dirty(&mut handler);

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.mounts, 1);
    assert_eq!(vm.updates, 1);
    assert_eq!(vm.unmounts, 0);
}

#[test]
fn lifecycle_update_dispatches_for_rebuilt_stable_identity() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let label = context.state(String::from("first"));
    let tree = WidgetTree::new(
        Text::new(label.signal())
            .key("stable")
            .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
            .on_update(Command::new(|vm: &mut LifecycleVm| vm.updates += 1)),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    label.set(String::from("second"));
    dispatch_lifecycle_if_dirty(&mut handler);

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.mounts, 1);
    assert_eq!(vm.updates, 1);
    assert_eq!(vm.unmounts, 0);
}

#[test]
fn lifecycle_update_only_dispatches_for_changed_widget() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::from("hi");

    let root_updates = Arc::new(AtomicUsize::new(0));
    let area_updates = Arc::new(AtomicUsize::new(0));
    let textarea_updates = Arc::new(AtomicUsize::new(0));

    let root_updates_capture = root_updates.clone();
    let area_updates_capture = area_updates.clone();
    let textarea_updates_capture = textarea_updates.clone();

    let tree = WidgetTree::new(
        Stack::<LifecycleVm>::new()
            .on_update(Command::new(move |_vm: &mut LifecycleVm| {
                root_updates_capture.fetch_add(1, Ordering::SeqCst);
            }))
            .child(
                Stack::<LifecycleVm>::new()
                    .on_update(Command::new(move |_vm: &mut LifecycleVm| {
                        area_updates_capture.fetch_add(1, Ordering::SeqCst);
                    }))
                    .child(Textarea::new(controller).on_update(Command::new(
                        move |_vm: &mut LifecycleVm| {
                            textarea_updates_capture.fetch_add(1, Ordering::SeqCst);
                        },
                    ))),
            ),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler
        .focused_widget_id()
        .expect("textarea should be focused after click");
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);
    dispatch_lifecycle_if_dirty(&mut handler);

    assert_eq!(root_updates.load(Ordering::SeqCst), 0);
    assert_eq!(area_updates.load(Ordering::SeqCst), 0);
    assert_eq!(textarea_updates.load(Ordering::SeqCst), 0);
}

#[test]
fn lifecycle_unmount_dispatches_when_component_is_removed() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let tree = WidgetTree::new(Stack::<LifecycleVm>::new().child(visible.signal().map(
        |visible| {
            let element: Element<LifecycleVm> = if visible {
                Text::new("shown")
                    .key("tracked")
                    .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
                    .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1))
                    .into()
            } else {
                Stack::<LifecycleVm>::new().into()
            };
            element
        },
    )));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    visible.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.mounts, 1);
    assert_eq!(vm.unmounts, 1);
    assert_eq!(vm.updates, 0);
}

