use super::*;

#[test]
fn hover_path_reuses_cached_computed_scene() {
    let invalidation = InvalidationSignal::new();
    let resolve_count = Arc::new(AtomicUsize::new(0));
    let child = {
        let resolve_count = resolve_count.clone();
        Signal::new(
            move || {
                resolve_count.fetch_add(1, Ordering::SeqCst);
                Text::new("hover").cursor(CursorStyle::Pointer)
            },
            invalidation.clone(),
        )
    };
    let tree = WidgetTree::new(Flex::new(Axis::Vertical).child(child));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert_eq!(handler.hover_path(viewport).len(), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    assert_eq!(handler.hover_path(viewport).len(), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
}

#[test]
fn clearing_pointer_position_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hover").cursor(CursorStyle::Pointer));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));
    let hover_epoch = handler.hover_epoch;
    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());

    handler.clear_pointer_position();

    assert!(handler.hovered_widgets.is_empty());
    assert_eq!(handler.hover_epoch, hover_epoch.wrapping_add(1));
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scrollbar_hover_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.vertical_thumb.is_some())
        .copied()
        .expect("textarea scroll region with a vertical thumb should exist");
    let thumb = region
        .vertical_thumb
        .expect("vertical scrollbar thumb should exist");

    assert!(handler.cached_scene.is_some());

    handler.cursor_position = Some(Point {
        x: thumb.x + Dp::new(thumb.width.get() * 0.5),
        y: thumb.y + Dp::new(thumb.height.get() * 0.5),
    });

    assert!(handler.sync_scrollbar_hover());
    assert_eq!(
        handler.hovered_scrollbar.map(|handle| handle.id),
        Some(region.id)
    );
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scene_cache_invalidates_when_units_change() {
    let invalidation = InvalidationSignal::new();
    let handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    assert!(!handler.scene_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.25),
        false,
        None,
    ));
}

#[test]
fn scene_layout_cache_survives_visual_only_animation_epoch_change() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.animation_epoch = 1;

    assert!(handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn theme_animation_invalidates_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.layout_animation_epoch = 1;

    assert!(!handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn theme_mode_change_invalidates_cached_layout_when_theme_changes() {
    let invalidation = InvalidationSignal::new();
    let mode = Signal::new(|| ThemeMode::Light, invalidation.clone());
    let (theme_set, _light, _dark) = custom_theme_set();
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode);
    handler.sync_theme_binding();

    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.window_bindings.theme_mode = Some(Signal::new(|| ThemeMode::Dark, invalidation));
    handler.sync_theme_binding();

    assert!(!handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn animation_scene_invalidation_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());
    assert!(!handler.text_input_regions.is_empty());

    handler.animation_epoch = handler.animation_epoch.wrapping_add(1);
    handler.invalidate_computed_scene();

    assert!(handler.cached_scene.is_some());
    assert!(handler.text_input_regions.is_empty());
    assert!(handler.scene_layout_cache_matches(
        handler
            .cached_scene
            .as_ref()
            .expect("cached scene should remain available"),
        viewport,
        units,
    ));
}

#[test]
fn visual_only_animation_refresh_prefers_scene_patch() {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::for_benchmarks();
    let opacity = ctx.state(0.0_f32);
    let tree = WidgetTree::new(
        Stack::<TestVm>::new()
            .opacity(opacity.project(|value| 0.3 + value * 0.7))
            .child(Text::new("animated visual only")),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let before_root = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .map(|layout| layout.root_id())
        .expect("cached layout should exist");

    opacity.set(1.0);
    handler.animation_epoch = handler.animation_epoch.wrapping_add(1);
    assert!(handler.patch_animation_scene_widgets(&[before_root.raw()], Instant::now()));
    assert!(handler.cached_scene.is_some());
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .map(|layout| layout.root_id())
        == Some(before_root));
}
