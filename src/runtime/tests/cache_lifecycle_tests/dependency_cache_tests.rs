use super::*;

#[test]
fn unrelated_state_update_preserves_cached_scene() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let unrelated = context.state(0);
    let tree = WidgetTree::new(Text::new("stable"));
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());

    unrelated.set(1);
    handler.request_redraw_if_dirty(Instant::now());

    assert!(handler.cached_scene.is_some());
}

#[test]
fn unrelated_state_update_does_not_rescan_lifecycle_handlers() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let unrelated = context.state(0);
    let probe = context.state(true);
    let probe_reads = Arc::new(AtomicUsize::new(0));
    let probe_signal = probe.signal().map({
        let probe_reads = probe_reads.clone();
        move |visible| {
            probe_reads.fetch_add(1, Ordering::SeqCst);
            visible
        }
    });
    let tree = WidgetTree::new(Stack::<TestVm>::new().child(probe_signal.map(
        |visible| -> Element<TestVm> {
            if visible {
                Text::new("stable").into()
            } else {
                Text::new("hidden").into()
            }
        },
    )));
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let baseline = probe_reads.load(Ordering::SeqCst);

    unrelated.set(1);
    handler.dispatch_lifecycle_events_if_needed();

    assert_eq!(probe_reads.load(Ordering::SeqCst), baseline);
}

#[test]
fn text_input_update_without_lifecycle_handlers_does_not_rescan_tree() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = context.text_controller("hello");
    let probe = context.state(true);
    let probe_reads = Arc::new(AtomicUsize::new(0));
    let probe_signal = probe.signal().map({
        let probe_reads = probe_reads.clone();
        move |visible| {
            probe_reads.fetch_add(1, Ordering::SeqCst);
            visible
        }
    });
    let tree = WidgetTree::new(
        Stack::<TestVm>::new()
            .child(probe_signal.map(|visible| -> Element<TestVm> {
                if visible {
                    Text::new("stable").into()
                } else {
                    Text::new("hidden").into()
                }
            }))
            .child(Textarea::new(controller)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
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

    assert_eq!(probe_reads.load(Ordering::SeqCst), baseline);
}

#[test]
fn set_definition_for_existing_window_preserves_cached_scene() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());
    assert!(!handler.text_input_regions.is_empty());

    handler.set_definition(
        WindowRole::Main,
        test_config(),
        WindowBindings::default(),
        Vec::new(),
        crate::application::WindowClosePolicy::Close,
    );

    assert!(handler.cached_scene.is_some());
    assert!(!handler.text_input_regions.is_empty());
}

#[test]
fn scene_only_dependency_update_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let checked = context.state(false);
    let tree = WidgetTree::new(Checkbox::new(checked.signal()));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    checked.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
    assert!(handler.scene_layout_cache_matches(cached, viewport, units));
}

#[test]
fn layout_dependency_update_preserves_cached_layout_shell() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let label = context.state(String::from("short"));
    let tree = WidgetTree::new(Text::new(label.signal()));
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    label.set(String::from("a much longer label"));
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("layout subtree patch should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
}

#[test]
fn dynamic_child_dependency_update_preserves_cached_layout_shell() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let tree = WidgetTree::new(
        Stack::<TestVm>::new().child(visible.signal().map(|visible| {
            if visible {
                Text::new("shown")
            } else {
                Text::new("hidden")
            }
        })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    visible.set(false);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("dynamic child subtree patch should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
}

#[test]
fn leaf_dependency_update_does_not_rebuild_unaffected_sibling_chunk() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let changed = context.state(String::from("short"));
    let sibling_state = context.state(0);
    let sibling_reads = Arc::new(AtomicUsize::new(0));
    let sibling_background = {
        let sibling_reads = sibling_reads.clone();
        sibling_state.signal().map(move |_| {
            sibling_reads.fetch_add(1, Ordering::SeqCst);
            Color::WHITE
        })
    };
    let changed_button: Element<TestVm> = Button::new(changed.signal()).key("changed").into();
    let sibling_surface: Element<TestVm> = Stack::new()
        .size(dp(24.0), dp(24.0))
        .style(move |mode| {
            let mut style = ContainerStyle::default_for(mode);
            style.surface.background = Some(sibling_background.clone().into());
            style
        })
        .into();
    let stack: Element<TestVm> = Stack::new().child([changed_button, sibling_surface]).into();
    let tree = WidgetTree::new(stack);
    let mut handler = test_handler(Some(tree), invalidation.clone());

    let _ = handler.computed_scene();
    assert_eq!(sibling_reads.load(Ordering::SeqCst), 1);

    changed.set(String::from("a much longer label"));
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("cached scene should remain available");
    assert!(cached.computed_valid);
    assert_eq!(sibling_reads.load(Ordering::SeqCst), 1);
}

