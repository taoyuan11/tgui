use super::*;

#[test]
fn textarea_show_scrollbar_dependency_update_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let show_scrollbar = context.state(false);
    let tree = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3")
            .height(dp(52.0))
            .show_scrollbar(show_scrollbar.signal()),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    show_scrollbar.set(true);
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
fn textarea_auto_wrap_dependency_update_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let auto_wrap = context.state(false);
    let tree = WidgetTree::new(
        Textarea::new("a very long line of text that should change the measured content")
            .height(dp(52.0))
            .auto_wrap(auto_wrap.signal()),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    auto_wrap.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
    assert!(handler.scene_layout_cache_matches(
        cached,
        handler.viewport_rect(),
        handler.unit_context()
    ));
}

#[test]
fn canvas_items_dependency_update_preserves_cached_layout_shell() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let expanded = context.state(false);
    let tree = WidgetTree::new(Canvas::new(expanded.signal().map(|expanded| {
        let width = if expanded { 96.0 } else { 48.0 };
        CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::WHITE)
                .begin_path()
                .move_to(0.0, 0.0)
                .line_to(width, 0.0)
                .line_to(width, 24.0)
                .line_to(0.0, 24.0)
                .close_path()
                .fill();
        })
    })));
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    expanded.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("canvas subtree patch should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
}

#[test]
fn removing_opaque_dependency_subtree_clears_global_fallback() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(false);
    let checked = context.state(false);
    let backing = Arc::new(Mutex::new(String::from("first")));
    let opaque = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::new(
        Stack::<TestVm>::new()
            .child(visible.signal().map({
                let opaque = opaque.clone();
                move |visible| {
                    let element: Element<TestVm> = if visible {
                        Text::new(opaque.clone()).key("opaque").into()
                    } else {
                        Text::new("static").key("static").into()
                    };
                    element
                }
            }))
            .child(Checkbox::new(checked.signal())),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let root_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .expect("cached layout should exist")
        .root_id();

    visible.set(true);
    assert!(handler.patch_cached_layout_for_roots(&[root_id], Instant::now()));
    assert!(handler.patch_cached_scene_for_roots(&[root_id], Instant::now(), true));
    assert!(handler
        .cached_scene
        .as_ref()
        .expect("patched cache should remain available")
        .dependencies
        .has_global_dependency());

    visible.set(false);
    assert!(handler.patch_cached_layout_for_roots(&[root_id], Instant::now()));
    assert!(handler.patch_cached_scene_for_roots(&[root_id], Instant::now(), true));
    assert!(!handler
        .cached_scene
        .as_ref()
        .expect("patched cache should remain available")
        .dependencies
        .has_global_dependency());

    checked.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should stay local after opaque removal");
    assert!(cached.computed_valid);
}

#[test]
fn opaque_signal_dirty_update_falls_back_to_full_scene_invalidation() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let backing = Arc::new(Mutex::new(String::from("first")));
    let signal = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::new(Text::new(signal));
    let mut handler = test_handler(Some(tree), invalidation.clone());

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    *backing.lock().expect("test signal lock poisoned") = String::from("second");
    invalidation.mark_dirty();
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("global invalidation should retain cache shell");
    assert!(!cached.layout_valid);
    assert!(!cached.computed_valid);
}

#[test]
fn select_portal_repositions_and_clears_after_scene_patch() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let expanded = context.state(false);
    let visible = context.state(true);
    let tree = WidgetTree::new(Stack::<TestVm>::new().child(visible.signal().map({
        let expanded = expanded.clone();
        move |visible| {
            if !visible {
                let hidden: Element<TestVm> = Text::new("hidden").into();
                return hidden;
            }
            let width = expanded
                .signal()
                .map(|expanded| if expanded { dp(180.0) } else { dp(120.0) });
            let select: Element<TestVm> = Select::new(
                vec![
                    SelectOption::new("email".to_string(), "Email".to_string()),
                    SelectOption::new("sms".to_string(), "SMS".to_string()),
                ],
                None::<String>,
            )
            .open(true)
            .size(width, dp(40.0))
            .into();
            select
        }
    })));
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = handler.computed_scene().clone();
    let initial_rect = initial
        .scene
        .overlay_shapes
        .first()
        .expect("open select should emit overlay")
        .rect;
    assert!(!initial.overlay_close_handlers.is_empty());

    expanded.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let expanded_scene = handler.computed_scene().clone();
    let expanded_rect = expanded_scene
        .scene
        .overlay_shapes
        .first()
        .expect("expanded select should still emit overlay")
        .rect;
    assert!(expanded_rect.width > initial_rect.width);

    visible.set(false);
    handler.request_redraw_if_dirty(Instant::now());
    let hidden_scene = handler.computed_scene().clone();
    assert!(hidden_scene.scene.overlay_shapes.is_empty());
    assert!(hidden_scene.overlay_close_handlers.is_empty());
}
