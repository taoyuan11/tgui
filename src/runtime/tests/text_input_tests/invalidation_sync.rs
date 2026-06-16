use super::*;

#[test]
fn textarea_on_change_set_does_not_force_global_invalidation() {
    let invalidation = InvalidationSignal::new();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_capture = callback_count.clone();
    let controller = TextController::from("hi");
    let tree = WidgetTree::new(Textarea::new(controller).on_change_set(ValueCommand::new(
        move |_vm: &mut TextInputVm, change_set: crate::mvvm::TextChangeSet| {
            assert!(!change_set.changes.is_empty());
            callback_count_capture.fetch_add(1, Ordering::SeqCst);
        },
    )));
    let mut handler =
        test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation.clone());
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

    let revision_before = invalidation.revision();

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);

    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    assert_eq!(invalidation.revision(), revision_before);
}

#[test]
fn textarea_on_change_state_update_does_not_rescan_unrelated_signal() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let unrelated = context.state(0usize);
    let probe = context.state(true);
    let probe_reads = Arc::new(AtomicUsize::new(0));
    let probe_signal = probe.signal().map({
        let probe_reads = probe_reads.clone();
        move |visible| {
            probe_reads.fetch_add(1, Ordering::SeqCst);
            visible
        }
    });
    let controller = TextController::from("hi");
    let unrelated_for_command = unrelated.clone();
    let tree = WidgetTree::new_legacy(
        Flex::<TextInputVm>::vertical()
            .child(
                Textarea::new(controller)
                    .height(dp(80.0))
                    .on_change(Command::new(move |_vm: &mut TextInputVm| {
                        unrelated_for_command.set(unrelated_for_command.get() + 1);
                    })),
            )
            .dynamic_child(
                probe_signal.map_unchecked(|visible| -> Element<TextInputVm> {
                    if visible {
                        Text::new("stable").into()
                    } else {
                        Text::new("hidden").into()
                    }
                }),
            ),
    );
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
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
    let baseline = probe_reads.load(Ordering::SeqCst);

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.handle_keyboard_input(&text_key_event("a"));
    flush_text_input_commits(&mut handler);
    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();

    assert_eq!(probe_reads.load(Ordering::SeqCst), baseline);
}

#[test]
fn textarea_on_change_state_update_keeps_text_input_scene_in_sync() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let change_count = context.state(0usize);
    let controller = TextController::from("hi");
    let change_count_for_command = change_count.clone();
    let tree = WidgetTree::new(
        Flex::<TextInputVm>::vertical()
            .width(dp(180.0))
            .height(dp(120.0))
            .child(
                Text::new(change_count.signal().map(|count| format!("count={count}")))
                    .width(dp(180.0))
                    .height(dp(20.0)),
            )
            .child(
                Textarea::new(controller)
                    .width(dp(180.0))
                    .height(dp(80.0))
                    .on_change(Command::new(move |_vm: &mut TextInputVm| {
                        change_count_for_command.set(change_count_for_command.get() + 1);
                    })),
            ),
    );
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: true,
                    on_change,
                    ..
                } => {
                    assert!(on_change.is_some());
                    Some(region.rect)
                }
                _ => None,
            })
            .expect("textarea hit region should exist")
    };
    handler.last_invalidation_revision = handler.invalidation.revision();

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.focused_text_input_id().is_some());
    assert!(handler.handle_keyboard_input(&text_key_event("a")));
    let focused_id = handler
        .focused_text_input_id()
        .expect("textarea should stay focused");
    assert!(handler
        .text_input_buffers
        .get(&focused_id)
        .map(|session| !session.pending_changes.is_empty())
        .unwrap_or(false));
    let flush_outcome = handler.flush_pending_text_input_changes();
    assert!(flush_outcome.changed);
    assert_eq!(change_count.get(), 1);
    handler.request_redraw_if_dirty(Instant::now());

    let computed = handler.computed_scene();
    let texts = computed
        .scene
        .texts
        .iter()
        .map(|primitive| primitive.content.clone())
        .collect::<Vec<_>>();
    assert!(
        texts
            .iter()
            .any(|content| content.contains('a') && content.contains("hi")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|content| content.as_ref() == "count=1"),
        "{texts:?}"
    );
}

#[test]
fn textarea_on_change_state_update_does_not_resolve_unrelated_dynamic_sibling() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let change_count = context.state(0usize);
    let probe = context.state(true);
    let probe_reads = Arc::new(AtomicUsize::new(0));
    let probe_signal = probe.signal().map_unchecked({
        let probe_reads = probe_reads.clone();
        move |visible| -> Element<TextInputVm> {
            probe_reads.fetch_add(1, Ordering::SeqCst);
            if visible {
                Text::new("stable").into()
            } else {
                Text::new("hidden").into()
            }
        }
    });
    let controller = TextController::from("hi");
    let change_count_for_command = change_count.clone();
    let tree = WidgetTree::new_legacy(
        Flex::<TextInputVm>::vertical()
            .width(dp(180.0))
            .height(dp(140.0))
            .dynamic_child(probe_signal)
            .child(
                Text::new(change_count.signal().map(|count| format!("count={count}")))
                    .width(dp(180.0))
                    .height(dp(20.0)),
            )
            .child(
                Textarea::new(controller)
                    .width(dp(180.0))
                    .height(dp(80.0))
                    .on_change(Command::new(move |_vm: &mut TextInputVm| {
                        change_count_for_command.set(change_count_for_command.get() + 1);
                    })),
            ),
    );
    let mut handler = test_handler_with_vm(TextInputVm::default(), Some(tree), invalidation);
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
    let baseline = probe_reads.load(Ordering::SeqCst);
    handler.last_invalidation_revision = handler.invalidation.revision();

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.handle_keyboard_input(&text_key_event("a")));
    flush_text_input_commits(&mut handler);
    let (dirty_kind, dirty_dependencies) = handler
        .invalidation
        .dirty_dependencies_since(handler.last_invalidation_revision);
    assert!(
        matches!(
            dirty_kind,
            crate::foundation::binding::DirtyDependencySet::Dependencies { .. }
        ),
        "{dirty_kind:?}"
    );
    let action = handler.invalidate_cached_scene_for_dependencies(
        dirty_kind,
        &dirty_dependencies,
        &[],
        0,
        Instant::now(),
    );
    assert_eq!(action, "scene_subtree_patch", "{action}");
    handler.last_invalidation_revision = handler.invalidation.revision();
    handler.sync_bindings(Instant::now());

    let computed = handler.computed_scene();
    let texts = computed
        .scene
        .texts
        .iter()
        .map(|primitive| primitive.content.clone())
        .collect::<Vec<_>>();

    assert_eq!(probe_reads.load(Ordering::SeqCst), baseline);
    assert!(
        texts
            .iter()
            .any(|content| content.contains('a') && content.contains("hi")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|content| content.as_ref() == "count=1"),
        "{texts:?}"
    );
}
