use super::*;
use crate::ui::widget::{ComputedScene, RenderCommand, SceneCounts, ShapePrimitiveSlot};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct CacheStats {
    builds: u64,
    validations: u64,
    hits: u64,
}

fn cache_stats(handler: &BoundRuntimeHandler<TestVm>) -> CacheStats {
    let (builds, validations, hits) = handler.focus_navigation_cache_stats();
    CacheStats {
        builds,
        validations,
        hits,
    }
}

fn dense_focus_tree(
    count: usize,
    clicks: &Arc<AtomicUsize>,
    last_click: &Arc<AtomicUsize>,
) -> (WidgetTree<TestVm>, Vec<WidgetId>) {
    let mut ids = Vec::with_capacity(count);
    let mut children = Vec::with_capacity(count);
    for index in 0..count {
        let clicks = Arc::clone(clicks);
        let last_click = Arc::clone(last_click);
        let button: Element<TestVm> = Button::new("")
            .size(dp(8.0), dp(8.0))
            .on_click(
                Command::new(move |_vm: &mut TestVm| {
                    clicks.fetch_add(1, Ordering::SeqCst);
                    last_click.store(index, Ordering::SeqCst);
                })
                .effect(crate::foundation::view_model::CommandEffect::NoUiChange),
            )
            .into();
        ids.push(button.id);
        children.push(button);
    }
    (WidgetTree::new(Stack::new().child(children)), ids)
}

#[test]
fn ten_thousand_focus_candidates_build_once_and_keep_tab_order() {
    const CANDIDATES: usize = 10_000;
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, ids) = dense_focus_tree(CANDIDATES, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = handler.focusable_widgets_in_tab_order();
    assert_eq!(initial.len(), CANDIDATES);
    let initial_stats = cache_stats(&handler);
    assert_eq!(initial_stats.builds, 1);
    assert_eq!(initial_stats.validations, 0);
    handler.request_redraw_if_dirty(Instant::now());
    crate::runtime::scene_patch::focus_ring_overlay_patch_probe::reset();

    // A few real keyboard/render cycles exercise the retained scene hand-off without making this
    // unit test spend the time required to walk all 10k controls once per possible Tab position.
    for expected_index in 0..8 {
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
        assert_eq!(handler.focused_widget_id(), Some(ids[expected_index]));
        let _ = handler.computed_scene();
    }
    let stats = cache_stats(&handler);
    assert_eq!(
        stats.builds, 1,
        "Tab must not rebuild the navigation snapshot"
    );
    assert_eq!(
        stats.validations, 0,
        "focus-only scene updates should retain the snapshot key"
    );
    assert!(stats.hits >= 8);
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::hits(),
        8,
        "each Tab paint should update only the retained focus-ring overlay"
    );
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::rejects(),
        0
    );
}

#[test]
fn non_focus_overlay_rejects_bounded_patch_and_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Switch::new(false).size(dp(52.0), dp(30.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    handler.request_redraw_if_dirty(Instant::now());

    crate::runtime::scene_patch::focus_ring_overlay_patch_probe::reset();
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::hits(),
        0
    );
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::rejects(),
        1,
        "the Switch thumb is a second overlay shape and must force the general fallback"
    );

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    super::table_tests::assert_data_grid_scene_equivalent(&fallback, &full);
}

#[test]
fn focus_ring_overlay_patch_matches_full_recollect() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, ids) = dense_focus_tree(3, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    handler.request_redraw_if_dirty(Instant::now());

    crate::runtime::scene_patch::focus_ring_overlay_patch_probe::reset();
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let retained_first = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::hits(),
        1
    );
    handler.invalidate_computed_scene();
    let full_first = handler.computed_scene().clone();
    super::table_tests::assert_data_grid_scene_equivalent(&retained_first, &full_first);

    crate::runtime::scene_patch::focus_ring_overlay_patch_probe::reset();
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let retained_second = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::hits(),
        1
    );
    handler.invalidate_computed_scene();
    let full_second = handler.computed_scene().clone();
    super::table_tests::assert_data_grid_scene_equivalent(&retained_second, &full_second);
    assert_eq!(handler.focused_widget_id(), Some(ids[1]));
}

#[test]
fn focus_ring_patch_then_general_recompose_matches_full_recollect() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let mut ids = Vec::new();
    let mut children = Vec::new();
    for index in 0..3 {
        let clicks = Arc::clone(&clicks);
        let last_click = Arc::clone(&last_click);
        let button: Element<TestVm> = Button::new("")
            .size(dp(8.0), dp(8.0))
            .on_click(
                Command::new(move |_vm: &mut TestVm| {
                    clicks.fetch_add(1, Ordering::SeqCst);
                    last_click.store(index, Ordering::SeqCst);
                })
                .effect(crate::foundation::view_model::CommandEffect::NoUiChange),
            )
            .into();
        ids.push(button.id);
        children.push(button);
    }
    let marker: Element<TestVm> = Text::new("marker").into();
    let marker_id = marker.id;
    children.push(marker);
    let tree = WidgetTree::new(Stack::new().focusable(true).tab_index(-1).child(children));
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    handler.request_redraw_if_dirty(Instant::now());

    crate::runtime::scene_patch::focus_ring_overlay_patch_probe::reset();
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_patch::focus_ring_overlay_patch_probe::hits(),
        1
    );

    assert!(handler.patch_cached_scene_for_roots(&[marker_id], Instant::now(), true));
    let retained = handler.computed_scene().clone();
    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    super::table_tests::assert_data_grid_scene_equivalent(&retained, &full);
    let focus_orders = |scene: &ComputedScene<TestVm>| {
        scene
            .hit_regions
            .iter()
            .filter_map(|region| {
                region
                    .focus
                    .as_ref()
                    .map(|focus| (focus.widget_id, focus.order))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(focus_orders(&retained), focus_orders(&full));
    assert_eq!(handler.focused_widget_id(), Some(ids[0]));
}

#[test]
fn cached_activation_dispatches_enter_and_space_once_from_current_scene() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, ids) = dense_focus_tree(4, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.focusable_widgets_in_tab_order();

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    assert_eq!(handler.focused_widget_id(), Some(ids[0]));
    let _ = handler.computed_scene();

    let replacement_clicks = Arc::new(AtomicUsize::new(0));
    let replacement_clicks_for_command = Arc::clone(&replacement_clicks);
    let computed = &mut handler
        .cached_scene
        .as_mut()
        .expect("scene should be cached")
        .computed;
    let interaction = computed
        .hit_regions
        .iter_mut()
        .find(|region| {
            region
                .interaction
                .keyboard_activation()
                .is_some_and(|hit| hit.0 == ids[0])
        })
        .expect("focused Button should have an activation interaction");
    if let HitInteraction::Widget { interactions, .. } = &mut interaction.interaction {
        interactions.on_click = Some(
            Command::new(move |_vm: &mut TestVm| {
                replacement_clicks_for_command.fetch_add(1, Ordering::SeqCst);
            })
            .effect(crate::foundation::view_model::CommandEffect::NoUiChange),
        );
    } else {
        panic!("expected Button widget interaction");
    }

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter)));
    handler.handle_keyboard_input(&released_key_event(PhysicalKey::Code(KeyCode::Enter)));
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_clicks.load(Ordering::SeqCst), 1);
    assert_eq!(last_click.load(Ordering::SeqCst), usize::MAX);

    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space)));
    handler.handle_keyboard_input(&released_key_event(PhysicalKey::Code(KeyCode::Space)));
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_clicks.load(Ordering::SeqCst), 2);
    assert_eq!(last_click.load(Ordering::SeqCst), usize::MAX);
    assert_eq!(cache_stats(&handler).builds, 1);
}

#[test]
fn paint_slots_and_transform_records_keep_focus_navigation_key() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, _) = dense_focus_tree(2, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.focusable_widgets_in_tab_order();
    let before_stats = cache_stats(&handler);
    let before_key = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .focus_navigation_cache_key();

    {
        let computed = &mut handler
            .cached_scene
            .as_mut()
            .expect("scene should be cached")
            .computed;
        let mut shape_index = 0;
        let mut shape_slot = None;
        for (command_index, command) in computed.scene.commands.iter().enumerate() {
            if matches!(command, RenderCommand::Shape(_)) {
                shape_slot = Some(ShapePrimitiveSlot {
                    shape_index,
                    command_index,
                });
                break;
            }
            shape_index += usize::from(matches!(command, RenderCommand::Shape(_)));
        }
        let slot = shape_slot.expect("Button scene should contain a shape");
        assert!(computed
            .scene
            .write_shape_color_slot(&SceneCounts::default(), slot, Color::RED));
        computed.transform_records.insert(
            WidgetId::from_raw(0xfeed),
            crate::ui::widget::TransformRecord {
                id: WidgetId::from_raw(0xfeed),
                base_offset: Point::ZERO,
                current_offset: Point::new(dp(1.0), dp(2.0)),
            },
        );
        assert_eq!(computed.focus_navigation_cache_key(), before_key);
    }

    let _ = handler.focusable_widgets_in_tab_order();
    let after_stats = cache_stats(&handler);
    assert_eq!(after_stats.builds, before_stats.builds);
    assert_eq!(after_stats.validations, before_stats.validations);
    assert!(after_stats.hits > before_stats.hits);
}

#[test]
fn changing_tab_or_activation_metadata_rebuilds_snapshot() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, _) = dense_focus_tree(2, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.focusable_widgets_in_tab_order();
    let before = cache_stats(&handler);

    let focus_index = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions
        .iter()
        .position(|region| region.focus.is_some())
        .expect("Button hit should be focusable");
    let original = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions[focus_index]
        .clone();

    let mut replacement = ComputedScene::<TestVm>::default();
    let mut changed = original.clone();
    changed.focus.as_mut().expect("focus metadata").tab_index = Some(7);
    replacement.hit_regions.push(changed);
    {
        let computed = &mut handler
            .cached_scene
            .as_mut()
            .expect("scene should be cached")
            .computed;
        assert!(computed.splice_chunk_in_place(
            &SceneCounts::default(),
            focus_index,
            0,
            &replacement,
        ));
    }
    let _ = handler.focusable_widgets_in_tab_order();
    let after_tab = cache_stats(&handler);
    assert_eq!(after_tab.builds, before.builds + 1);

    let activation_index = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions
        .iter()
        .position(|region| region.interaction.keyboard_activation().is_some())
        .expect("Button hit should be keyboard activatable");
    let original = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions[activation_index]
        .clone();
    let mut replacement = ComputedScene::<TestVm>::default();
    let mut changed = original.clone();
    if let HitInteraction::Widget { interactions, .. } = &mut changed.interaction {
        interactions.on_click = None;
    } else {
        panic!("expected Button widget interaction");
    }
    replacement.hit_regions.push(changed);
    {
        let computed = &mut handler
            .cached_scene
            .as_mut()
            .expect("scene should be cached")
            .computed;
        assert!(computed.splice_chunk_in_place(
            &SceneCounts::default(),
            activation_index,
            0,
            &replacement,
        ));
    }
    let _ = handler.focusable_widgets_in_tab_order();
    let after_activation = cache_stats(&handler);
    assert_eq!(after_activation.builds, after_tab.builds + 1);
}

#[test]
fn duplicate_overlay_hit_reuses_snapshot_but_moving_first_hit_rebuilds() {
    let clicks = Arc::new(AtomicUsize::new(0));
    let last_click = Arc::new(AtomicUsize::new(usize::MAX));
    let (tree, ids) = dense_focus_tree(2, &clicks, &last_click);
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(Some(tree), invalidation);
    let initial = handler.focusable_widgets_in_tab_order();
    assert_eq!(
        initial
            .iter()
            .map(|item| item.widget_id)
            .collect::<Vec<_>>(),
        ids
    );
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab)));
    let _ = handler.computed_scene();

    let normal_index = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions
        .iter()
        .position(|region| {
            region
                .focus
                .as_ref()
                .is_some_and(|focus| focus.widget_id == ids[0])
                && region.interaction.keyboard_activation().is_some()
        })
        .expect("first Button should have one focus/activation hit");
    let duplicate = handler
        .cached_scene
        .as_ref()
        .expect("scene should be cached")
        .computed
        .hit_regions[normal_index]
        .clone();
    let before_duplicate = cache_stats(&handler);
    {
        let computed = &mut handler
            .cached_scene
            .as_mut()
            .expect("scene should be cached")
            .computed;
        computed.overlay_hit_regions.push(duplicate);
        computed.assign_new_prepare_cache_serial();
    }
    let duplicate_order = handler.focusable_widgets_in_tab_order();
    assert_eq!(
        duplicate_order
            .iter()
            .map(|item| item.widget_id)
            .collect::<Vec<_>>(),
        ids
    );
    let after_duplicate = cache_stats(&handler);
    assert_eq!(after_duplicate.builds, before_duplicate.builds);
    assert_eq!(
        after_duplicate.validations,
        before_duplicate.validations + 1
    );
    assert!(handler.activate_focused_widget(true, false));
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    assert_eq!(last_click.load(Ordering::SeqCst), 0);

    {
        let computed = &mut handler
            .cached_scene
            .as_mut()
            .expect("scene should be cached")
            .computed;
        computed.hit_regions.remove(normal_index);
        computed.assign_new_prepare_cache_serial();
    }
    let moved_order = handler.focusable_widgets_in_tab_order();
    assert_eq!(
        moved_order
            .iter()
            .map(|item| item.widget_id)
            .collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    let after_move = cache_stats(&handler);
    assert_eq!(after_move.builds, after_duplicate.builds + 1);
    assert!(handler.activate_focused_widget(true, false));
    assert_eq!(clicks.load(Ordering::SeqCst), 2);
    assert_eq!(last_click.load(Ordering::SeqCst), 0);
}
