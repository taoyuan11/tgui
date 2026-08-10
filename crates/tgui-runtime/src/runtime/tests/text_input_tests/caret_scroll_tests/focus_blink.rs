use super::*;
use crate::runtime::CARET_BLINK_INTERVAL;
use crate::ui::widget::SceneDrawStream;

#[test]
fn focused_text_input_schedules_caret_blink_deadline() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let deadline = handler.next_caret_blink_deadline(Instant::now());
    assert!(deadline.is_some());

    handler.update_focus(None, None, false);
    let deadline = handler.next_caret_blink_deadline(Instant::now());
    assert!(deadline.is_none());
}

#[test]
fn caret_blink_requests_redraw_when_visibility_flips() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();

    handler
        .cached_scene
        .as_mut()
        .expect("cached scene should exist after rendering")
        .caret_visible = false;

    let now = handler.caret_blink_origin + std::time::Duration::from_millis(1);
    assert!(handler.caret_blink_needs_redraw(now));
}

#[test]
fn idle_input_blink_requests_redraw_and_dirties_the_retained_caret_draw() {
    let invalidation = InvalidationSignal::new();
    let input: Element<TestVm> = Input::<TestVm>::new("hello").into();
    let input_id = input.id;
    let tree = WidgetTree::new(input);
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();
    assert_eq!(
        handler.cached_focus_target_is_text_input(input_id),
        Some(true),
        "the blink path should use the current focus-navigation cache"
    );

    let visible_color = {
        let cached = handler.cached_scene.as_mut().expect("focused input cache");
        cached.computed.scene.clear_dirty_draw_ranges();
        assert!(cached.caret_visible);
        let binding = cached.caret_decoration.expect("retained caret binding");
        cached.computed.scene.overlay_text_decorations[binding.overlay_text_decoration_index].color
    };
    assert!(visible_color.a > 0);

    let window = Arc::new(super::super::super::window_theme_tests::BindingProbeWindow::new(None));
    handler.window = Some(window.clone());
    handler.caret_blink_origin = Instant::now() - CARET_BLINK_INTERVAL - Duration::from_millis(10);
    window.redraw_calls.store(0, Ordering::Relaxed);

    assert!(!handler.handle_bound_about_to_wait(&TestEventLoop));
    assert!(
        window.redraw_calls.load(Ordering::Relaxed) > 0,
        "the idle blink deadline should request a redraw"
    );

    let _ = handler.computed_scene();
    let cached = handler.cached_scene.as_ref().expect("cache after redraw");
    let binding = cached.caret_decoration.expect("retained caret binding");
    assert!(!cached.caret_visible);
    assert_eq!(
        cached.computed.scene.overlay_text_decorations[binding.overlay_text_decoration_index]
            .color
            .a,
        0
    );
    assert!(cached
        .computed
        .scene
        .dirty_draw_ranges()
        .iter()
        .any(|range| {
            range.stream == SceneDrawStream::Overlay
                && range.range.contains(&binding.overlay_command_index)
        }));

    handler
        .cached_scene
        .as_mut()
        .expect("cache before next blink")
        .computed
        .scene
        .clear_dirty_draw_ranges();
    handler.caret_blink_origin =
        Instant::now() - CARET_BLINK_INTERVAL - CARET_BLINK_INTERVAL - Duration::from_millis(10);
    window.redraw_calls.store(0, Ordering::Relaxed);

    assert!(!handler.handle_bound_about_to_wait(&TestEventLoop));
    assert!(window.redraw_calls.load(Ordering::Relaxed) > 0);
    let _ = handler.computed_scene();
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("cache after next redraw");
    let binding = cached.caret_decoration.expect("retained caret binding");
    assert!(cached.caret_visible);
    assert_eq!(
        cached.computed.scene.overlay_text_decorations[binding.overlay_text_decoration_index].color,
        visible_color
    );
    assert!(cached
        .computed
        .scene
        .dirty_draw_ranges()
        .iter()
        .any(|range| {
            range.stream == SceneDrawStream::Overlay
                && range.range.contains(&binding.overlay_command_index)
        }));
}

#[test]
#[cfg(feature = "bench-support")]
fn caret_blink_updates_retained_decoration_slot() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert!(
        cached.caret_decoration.is_some(),
        "focused text input should retain a caret decoration slot"
    );
    let before_count = cached.computed.scene.overlay_text_decorations.len();
    let before_color = cached.computed.scene.overlay_text_decorations[0].color;
    assert!(before_color.a > 0, "initial caret should be visible");

    crate::runtime::action_stats::reset();
    assert!(handler.try_update_caret_visibility_slot(false));
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(
        cached.computed.scene.overlay_text_decorations.len(),
        before_count,
        "caret blink should not add or remove overlay decorations"
    );
    assert_eq!(cached.computed.scene.overlay_text_decorations[0].color.a, 0);
    assert!(cached
        .computed
        .scene
        .dirty_draw_ranges()
        .iter()
        .any(|range| { range.stream == SceneDrawStream::Overlay && range.range.contains(&0) }));
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "caret_visibility_slot_write" && *count == 1),
        "caret visibility should be written through the retained slot: {snapshot:?}"
    );

    crate::runtime::action_stats::reset();
    assert!(handler.try_update_caret_visibility_slot(true));
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(
        cached.computed.scene.overlay_text_decorations.len(),
        before_count
    );
    assert_eq!(
        cached.computed.scene.overlay_text_decorations[0].color,
        before_color
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "caret_visibility_slot_write" && *count == 1),
        "caret visibility restore should be written through the retained slot: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn single_line_input_edit_updates_retained_text_slots() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();

    let before_counts = {
        let cached = handler.cached_scene.as_ref().expect("cache shell");
        assert!(
            !cached.text_input_slot_bindings.is_empty(),
            "focused single-line input should retain text/selection/caret slots"
        );
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
            cached.computed.scene.commands.len(),
            cached.computed.scene.overlay_commands.len(),
        )
    };

    crate::runtime::action_stats::reset();
    assert!(handler.handle_keyboard_input(&text_key_event("!")));
    let expected_epoch = handler.text_input_epoch;
    {
        let computed = handler.computed_scene();
        assert!(
            computed
                .scene
                .texts
                .iter()
                .any(|primitive| primitive.content.as_ref() == "hello!"),
            "retained slot write should update the input text content"
        );
    }
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(
        cached.text_input_epoch, expected_epoch,
        "slot write should synchronize the cached text input epoch"
    );
    assert_eq!(
        before_counts,
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
            cached.computed.scene.commands.len(),
            cached.computed.scene.overlay_commands.len(),
        ),
        "text edit should keep fixed primitive counts"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "text_input_slot_write" && *count == 1),
        "single-line edit should be written through retained slots: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "text_input_scene_patch"),
        "single-line edit should not need the subtree scene patch path: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn single_line_input_selection_updates_retained_decoration_slot() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();
    let focused_id = handler
        .focused_text_input_id()
        .expect("input should be focused");
    let before_count = handler
        .cached_scene
        .as_ref()
        .expect("cache shell")
        .computed
        .scene
        .text_decorations
        .len();
    assert_eq!(
        before_count, 1,
        "single-line input should reserve one selection slot"
    );

    crate::runtime::action_stats::reset();
    assert!(
        handler.update_text_edit_state(focused_id, "hello", |state| {
            state.anchor = 1;
            state.cursor = 4;
        })
    );
    let expected_epoch = handler.text_input_epoch;
    {
        let computed = handler.computed_scene();
        let selection = computed
            .scene
            .text_decorations
            .last()
            .expect("selection decoration should remain retained");
        assert_eq!(selection.segments.len(), 1);
        assert!(
            selection.segments[0].width > Dp::ZERO,
            "selection slot should update its bounded segment"
        );
    }
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(cached.text_input_epoch, expected_epoch);
    assert_eq!(
        cached.computed.scene.text_decorations.len(),
        before_count,
        "selection update should not add or remove decoration primitives"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "text_input_slot_write" && *count == 1),
        "single-line selection update should be written through retained slots: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn single_line_ime_preedit_updates_retained_text_slots() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new(""));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();
    let before_counts = {
        let cached = handler.cached_scene.as_ref().expect("cache shell");
        assert!(
            !cached.text_input_slot_bindings.is_empty(),
            "focused empty single-line input should still retain text slots"
        );
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
        )
    };

    crate::runtime::action_stats::reset();
    assert!(handler.handle_ime_event(&Ime::Preedit("ni".to_string(), Some((0, "ni".len())),)));
    let expected_epoch = handler.text_input_epoch;
    {
        let computed = handler.computed_scene();
        assert!(
            computed
                .scene
                .texts
                .iter()
                .any(|primitive| primitive.content.as_ref() == "ni"),
            "IME preedit should update the retained text primitive"
        );
        let selection = computed
            .scene
            .text_decorations
            .last()
            .expect("composition decoration should remain retained");
        assert_eq!(selection.segments.len(), 1);
        assert!(
            selection.segments[0].width > Dp::ZERO,
            "composition segment should be written into the retained selection slot"
        );
    }
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(cached.text_input_epoch, expected_epoch);
    assert_eq!(
        before_counts,
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
        ),
        "IME preedit should keep fixed primitive counts"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "text_input_slot_write" && *count == 1),
        "single-line IME preedit should be written through retained slots: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "text_input_scene_patch"),
        "single-line IME preedit should not use subtree scene patch: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn multiline_textarea_selection_updates_retained_decoration_slot() {
    let invalidation = InvalidationSignal::new();
    let content = "hello\nworld";
    let tree: WidgetTree<TestVm> =
        WidgetTree::new(Textarea::<TestVm>::new(content).height(dp(120.0)));
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

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    let _ = handler.computed_scene();
    let focused_id = handler
        .focused_widget_id()
        .expect("textarea should be focused");
    let before = {
        let cached = handler.cached_scene.as_ref().expect("cache shell");
        assert!(
            !cached.text_input_slot_bindings.is_empty(),
            "focused textarea should retain selection/caret slots"
        );
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
        )
    };

    crate::runtime::action_stats::reset();
    assert!(
        handler.update_text_edit_state(focused_id, content, |state| {
            state.anchor = 0;
            state.cursor = content.len();
        })
    );
    let expected_epoch = handler.text_input_epoch;
    {
        let computed = handler.computed_scene();
        let selection = computed
            .scene
            .text_decorations
            .last()
            .expect("textarea selection decoration should remain retained");
        assert!(
            selection.segments.len() > 1,
            "multiline selection should update multiple bounded segments inside one retained primitive"
        );
    }
    let snapshot = crate::runtime::action_stats::snapshot();
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert_eq!(cached.text_input_epoch, expected_epoch);
    assert_eq!(
        before,
        (
            cached.computed.scene.texts.len(),
            cached.computed.scene.text_decorations.len(),
            cached.computed.scene.overlay_text_decorations.len(),
        ),
        "selection-only textarea update should keep primitive counts stable"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "text_input_slot_write" && *count == 1),
        "textarea selection-only update should be written through retained slots: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "text_input_scene_patch"),
        "textarea selection-only update should not use subtree scene patch: {snapshot:?}"
    );
}

#[test]
fn clicking_text_input_renders_caret_on_first_focused_frame() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Input::<TestVm>::new("hello"));
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + frame.width - dp(4.0),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let computed = handler.computed_scene();
    assert!(
        computed.ime_cursor_area.is_some(),
        "focused input should expose a caret rect on the first focused frame"
    );
    assert!(
        !computed.scene.overlay_text_decorations.is_empty(),
        "focused input should render the caret immediately after click"
    );
}

#[test]
fn non_text_focus_does_not_schedule_caret_blink_deadline() {
    let invalidation = InvalidationSignal::new();
    let tree: WidgetTree<TestVm> = WidgetTree::new(Button::new("click"));
    let mut handler = test_handler(Some(tree), invalidation);

    handler.update_focus(
        Some(FocusedWidget {
            widget_id: WidgetId::from_raw(999),
            scope_path: Vec::new(),
            on_blur: None,
        }),
        None,
        false,
    );

    let deadline = handler.next_deadline(Instant::now());
    assert!(deadline.is_none());
}
