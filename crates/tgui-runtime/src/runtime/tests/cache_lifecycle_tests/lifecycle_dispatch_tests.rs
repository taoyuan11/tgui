use super::*;
use crate::ui::widget::{Drawer, DrawerHost, DrawerMode, Image, Modal, Portal};

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
            .size(dp(160.0), dp(24.0))
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
            .size(dp(160.0), dp(24.0))
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
    let tree = WidgetTree::new_legacy(Stack::<LifecycleVm>::new().dynamic_child(
        visible.signal().map_unchecked(|visible| {
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
        }),
    ));
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

#[test]
fn portal_content_lifecycle_tracks_open_state() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let content = Text::new("portal content")
        .key("portal-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let portal: Element<LifecycleVm> = crate::ui::widget::Portal::new(content)
        .open(open.signal())
        .anchor(Rect::new(dp(24.0), dp(18.0), dp(1.0), dp(1.0)))
        .into();
    let tree = WidgetTree::new_legacy(Stack::new().child(portal));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!((vm.mounts, vm.unmounts), (0, 0));
    }

    open.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!((vm.mounts, vm.unmounts), (1, 0));
    }

    open.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!((vm.mounts, vm.unmounts), (1, 1));
}

#[test]
fn rich_tooltip_content_lifecycle_tracks_visibility() {
    let invalidation = InvalidationSignal::new();
    let tooltip_content = Text::new("tooltip content")
        .key("tooltip-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let tree = WidgetTree::new(
        Button::new("Inspect")
            .size(dp(120.0), dp(40.0))
            .tooltip(Tooltip::content(tooltip_content).delay(Duration::ZERO)),
    );
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    handler.reduced_motion = true;

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!((vm.mounts, vm.unmounts), (0, 0));
    }

    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(40.0), dp(20.0)));
    assert!(handler.handle_hover(viewport));
    let _ = handler.computed_scene();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler
            .view_model
            .lock()
            .expect("view model lock should not be poisoned");
        assert_eq!((vm.mounts, vm.unmounts), (1, 0));
    }

    handler.cursor_position = Some(Point::new(dp(500.0), dp(500.0)));
    assert!(handler.handle_hover(viewport));
    let _ = handler.computed_scene();
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!((vm.mounts, vm.unmounts), (1, 1));
}

#[test]
fn modal_content_lifecycle_tracks_open_state() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let content = Text::new("modal content")
        .key("modal-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let tree = WidgetTree::new(Modal::new(open.signal()).content(content));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    handler.reduced_motion = true;

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (0, 0));
    }

    open.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (1, 0));
    }

    open.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler.view_model.lock().unwrap();
    assert_eq!((vm.mounts, vm.unmounts), (1, 1));
}

#[test]
fn overlay_drawer_content_lifecycle_tracks_open_state() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let content = Text::new("drawer content")
        .key("overlay-drawer-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let tree = WidgetTree::new(Drawer::new(open.signal()).content(content));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    handler.reduced_motion = true;

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (0, 0));
    }

    open.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (1, 0));
    }

    open.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler.view_model.lock().unwrap();
    assert_eq!((vm.mounts, vm.unmounts), (1, 1));
}

#[test]
fn push_drawer_only_gates_panel_lifecycle_and_interactions() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let main_content: Element<LifecycleVm> = Button::new("Main action")
        .size(dp(120.0), dp(32.0))
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 10))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 10))
        .into();
    let main_content_id = main_content.id;
    let panel_content = Text::new("drawer panel")
        .key("push-drawer-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let drawer = Drawer::new(open.signal())
        .mode(DrawerMode::Push)
        .content(panel_content);
    let tree = WidgetTree::new(DrawerHost::new(main_content, drawer).size(dp(480.0), dp(320.0)));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    handler.reduced_motion = true;

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (10, 0));
    }
    assert!(handler
        .computed_scene()
        .hit_regions
        .iter()
        .any(|region| { region.interaction.widget_id() == main_content_id }));

    open.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (11, 0));
    }

    open.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler.view_model.lock().unwrap();
    assert_eq!((vm.mounts, vm.unmounts), (11, 1));
    drop(vm);
    assert!(handler
        .computed_scene()
        .hit_regions
        .iter()
        .any(|region| { region.interaction.widget_id() == main_content_id }));
}

#[test]
fn closed_modal_suppresses_nested_portal_lifecycle() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let portal_content = Text::new("portal in modal")
        .key("modal-portal-lifecycle-content")
        .on_mount(Command::new(|vm: &mut LifecycleVm| vm.mounts += 1))
        .on_unmount(Command::new(|vm: &mut LifecycleVm| vm.unmounts += 1));
    let portal: Element<LifecycleVm> = Portal::new(portal_content)
        .open(true)
        .anchor(Rect::new(dp(24.0), dp(18.0), dp(1.0), dp(1.0)))
        .into();
    let tree = WidgetTree::new(Modal::new(open.signal()).content(portal));
    let mut handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);
    handler.reduced_motion = true;

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (0, 0));
    }

    open.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    {
        let vm = handler.view_model.lock().unwrap();
        assert_eq!((vm.mounts, vm.unmounts), (1, 0));
    }

    open.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);
    let vm = handler.view_model.lock().unwrap();
    assert_eq!((vm.mounts, vm.unmounts), (1, 1));
}

#[test]
fn closed_modal_skips_media_event_subtree_walk() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(false);
    let image: Element<LifecycleVm> = Image::new(crate::media::MediaSource::path(
        "missing-modal-media-event-test.png",
    ))
    .on_loading(Command::new(|_: &mut LifecycleVm| {}));
    let tree = WidgetTree::new(Modal::new(open.signal()).content(image));
    let handler = test_handler_with_vm(LifecycleVm::default(), Some(tree), invalidation);

    crate::ui::widget::media_event_walk_probe::reset();
    let states = handler
        .widget_tree
        .as_ref()
        .unwrap()
        .media_event_states(&handler.media_manager, &handler.theme);
    assert!(states.is_empty());
    assert_eq!(crate::ui::widget::media_event_walk_probe::visits(), 0);

    open.set(true);
    crate::ui::widget::media_event_walk_probe::reset();
    let _ = handler
        .widget_tree
        .as_ref()
        .unwrap()
        .media_event_states(&handler.media_manager, &handler.theme);
    assert!(crate::ui::widget::media_event_walk_probe::visits() > 0);
}
