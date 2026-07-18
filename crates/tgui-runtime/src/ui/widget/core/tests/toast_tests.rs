pub(super) use super::*;

use std::time::{Duration, Instant};

use crate::animation::FrameClockSnapshot;
use crate::foundation::binding::{Toast, ToastKind, ToastPlacement, ToastQueue};
#[cfg(feature = "bench-support")]
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::style::ToastStyle;
#[cfg(feature = "bench-support")]
use crate::ui::widget::{
    with_prepared_toast_card_cache, with_toast_base_scene_replay, CollectedSceneCache, HitGeometry,
    ResolvedSceneLayout,
};
use crate::ui::widget::{Button, ComputedScene, Popover, Stack, ToastHost};

#[test]
fn toast_density_geometry_reaches_real_overlay_primitives() {
    run_with_large_stack(toast_density_geometry_reaches_real_overlay_primitives_impl);
}

fn toast_density_geometry_reaches_real_overlay_primitives_impl() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 480.0);

    for mut theme in [Theme::light(), Theme::dark()] {
        for (density, expected_width, expected_icon) in [
            (Density::Compact, dp(280.0), dp(16.0)),
            (Density::Spacious, dp(360.0), dp(20.0)),
        ] {
            theme.density = density;
            let context = test_context();
            let queue = ToastQueue::<()>::new(&context);
            let now = Instant::now();
            queue.push_at(
                Toast::new("Density-aware toast")
                    .title("Saved")
                    .kind(ToastKind::Success)
                    .duration(Duration::from_secs(10)),
                now - Duration::from_secs(1),
            );
            let tree = WidgetTree::new(Stack::new().child(ToastHost::new(queue)));
            let mut animations = AnimationEngine::default();
            let computed = compute_scene_at(
                &tree,
                &font_manager,
                &theme,
                &media,
                &mut animations,
                viewport,
                now,
            );

            let card = computed
                .scene
                .overlay_shapes
                .iter()
                .find(|shape| {
                    shape.color == theme.colors.outline_muted
                        && (shape.rect.width - expected_width).abs() <= dp(0.1)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "toast card should render with density width {expected_width:?}; target={:?}, shapes={:?}",
                        theme.colors.outline_muted,
                        computed
                            .scene
                            .overlay_shapes
                            .iter()
                            .map(|shape| (shape.rect, shape.color, shape.corner_radius))
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(card.corner_radius, theme.radius.lg.get());

            let icon = computed
                .scene
                .overlay_shapes
                .iter()
                .find(|shape| {
                    shape.color == theme.colors.success
                        && (shape.rect.width - expected_icon).abs() <= dp(0.1)
                        && (shape.rect.height - expected_icon).abs() <= dp(0.1)
                })
                .expect("toast kind icon should render with its density size");
            assert_eq!(icon.corner_radius, expected_icon.get() * 0.5);

            let title = computed
                .scene
                .overlay_texts
                .iter()
                .find(|text| text.content.as_ref() == "Saved")
                .expect("toast title primitive");
            let body = computed
                .scene
                .overlay_texts
                .iter()
                .find(|text| text.content.as_ref() == "Density-aware toast")
                .expect("toast body primitive");
            assert_eq!(title.font_size, theme.typography.label.size.get());
            assert_eq!(title.font_weight, crate::theme::FontWeight::Medium);
            assert_eq!(
                title.line_height,
                theme
                    .typography
                    .label
                    .line_height
                    .expect("default label line height")
                    .get()
            );
            assert_eq!(body.font_size, theme.typography.body_small.size.get());
            assert_eq!(body.font_weight, crate::theme::FontWeight::Regular);
            assert_eq!(
                body.line_height,
                theme
                    .typography
                    .body_small
                    .line_height
                    .expect("default body-small line height")
                    .get()
            );
            assert!(title.frame.y < body.frame.y);
            assert!(body.frame.height.get() >= body.line_height);
        }
    }
}

#[test]
fn toast_host_emits_overlay_content_in_stack_order_and_tracks_wakeup() {
    run_with_large_stack(toast_host_emits_overlay_content_in_stack_order_and_tracks_wakeup_impl);
}

fn toast_host_emits_overlay_content_in_stack_order_and_tracks_wakeup_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    // 用过去的时间创建Toast，确保入场动画已完成
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("first").duration(Duration::from_secs(10)),
        created_at,
    );
    queue.push_at(
        Toast::new("second").duration(Duration::from_secs(9)),
        created_at,
    );
    queue.push_at(
        Toast::new("third").duration(Duration::from_secs(8)),
        created_at,
    );

    let tree = WidgetTree::new(
        Stack::new().child(Text::new("content")).child(
            ToastHost::new(queue.clone())
                .placement(ToastPlacement::BottomEnd)
                .max_visible(2),
        ),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let layout = tree.build_scene_layout_at(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
    );
    let default_style_sheet = crate::ui::widget::StyleSheet::default();
    let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        now,
        &HashMap::new(),
        None,
        None,
        &default_style_sheet,
    );

    // Toast在1秒前创建，入场动画已完成（400ms），唤醒时间应该是最早的deadline
    assert!(
        collected.next_toast_wakeup.is_some(),
        "should have a toast wakeup time"
    );
    assert!(
        !collected.computed.scene.overlay_shapes.is_empty(),
        "toast host should emit overlay card shapes"
    );

    let labels: Vec<_> = collected
        .computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(
        !labels.iter().any(|text| *text == "first"),
        "max_visible(2) should clip the oldest toast, got {labels:?}"
    );

    let third_index = labels
        .iter()
        .position(|text| *text == "third")
        .expect("latest toast should be rendered");
    let second_index = labels
        .iter()
        .position(|text| *text == "second")
        .expect("second toast should be rendered");
    assert!(
        third_index < second_index,
        "bottom placement should keep the newest toast closest to the anchor, got {labels:?}"
    );
}

#[test]
fn empty_toast_host_does_not_cover_underlying_hits() {
    run_with_large_stack(empty_toast_host_does_not_cover_underlying_hits_impl);
}

fn empty_toast_host_does_not_cover_underlying_hits_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);

    let clickable: Element<()> = Stack::new()
        .size(dp(120.0), dp(48.0))
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let clickable_id = clickable.id;
    let tree = WidgetTree::new(
        Stack::new()
            .child(clickable)
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );

    let computed = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        Instant::now(),
    );
    let hit = WidgetTree::hit_path_from_computed(&computed, Point::new(24.0, 24.0)).pop();

    assert!(
        matches!(hit, Some(HitInteraction::Widget { id, .. }) if id == clickable_id),
        "empty ToastHost should not block the clickable below"
    );
}

#[test]
fn stable_toast_host_collection_does_not_invalidate_queue() {
    run_with_large_stack(stable_toast_host_collection_does_not_invalidate_queue_impl);
}

fn stable_toast_host_collection_does_not_invalidate_queue_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let now = Instant::now();

    for has_live_entry in [false, true] {
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        if has_live_entry {
            queue.push_at(
                Toast::new("live").duration(Duration::from_secs(10)),
                now - Duration::from_secs(1),
            );
        }
        let tree = WidgetTree::new(Stack::new().child(ToastHost::new(queue)));
        let revision = context.invalidation().revision();
        let mut animations = AnimationEngine::default();

        let _ = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        );

        assert_eq!(
            context.invalidation().revision(),
            revision,
            "stable ToastHost collection must not self-invalidate; has_live_entry={has_live_entry}"
        );
    }
}

#[test]
fn toast_stack_auto_collapses_after_three_and_expands_from_queue_state() {
    run_with_large_stack(toast_stack_auto_collapses_after_three_and_expands_from_queue_state_impl);
}

fn toast_stack_auto_collapses_after_three_and_expands_from_queue_state_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let collapsed = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let collapsed_messages = toast_message_labels(&collapsed);
    assert_eq!(
        collapsed_messages,
        vec!["toast 4"],
        "collapsed stack should show only the front toast content"
    );
    let (front, back_layers) = toast_stack_card_layers(&collapsed);
    assert!(
        back_layers.len() >= 2,
        "collapsed stack should reveal two back card layers, got {back_layers:?}"
    );
    assert!(
        (back_layers[0].x - (front.x + dp(12.0))).abs() <= dp(1.0)
            && (back_layers[0].width - (front.width - dp(24.0))).abs() <= dp(1.0)
            && (back_layers[0].y - (front.y + dp(16.0))).abs() <= dp(1.0),
        "first back layer should be inset and shifted: front={front:?}, layer={:?}",
        back_layers[0]
    );
    assert!(
        (back_layers[1].x - (front.x + dp(24.0))).abs() <= dp(1.0)
            && (back_layers[1].width - (front.width - dp(48.0))).abs() <= dp(1.0)
            && (back_layers[1].y - (front.y + dp(32.0))).abs() <= dp(1.0),
        "second back layer should be inset and shifted: front={front:?}, layer={:?}",
        back_layers[1]
    );

    queue.set_stack_expanded(true);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let expanded = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(220),
    );
    let expanded_messages = toast_message_labels(&expanded);
    assert_eq!(
        expanded_messages,
        vec!["toast 4", "toast 3", "toast 2", "toast 1"],
        "expanded stack should render all queued toasts"
    );

    queue.set_stack_expanded(false);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(220),
    );
    let collapsed_again = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(440),
    );
    let collapsed_again_messages = toast_message_labels(&collapsed_again);
    assert_eq!(
        collapsed_again_messages,
        vec!["toast 4"],
        "leaving the stack should collapse it back to front toast content"
    );
}

#[test]
fn toast_stack_expand_animation_interpolates_layers() {
    run_with_large_stack(toast_stack_expand_animation_interpolates_layers_impl);
}

fn toast_stack_expand_animation_interpolates_layers_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let collapsed = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let (collapsed_front, collapsed_back_layers) = toast_stack_card_layers(&collapsed);
    let collapsed_first_back = collapsed_back_layers[0];

    queue.set_stack_expanded(true);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let mid = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(90),
    );
    let mid_messages = toast_message_labels(&mid);
    assert!(
        mid_messages.iter().any(|label| label == "toast 3"),
        "second toast content should fade in during expansion, got {mid_messages:?}"
    );
    let (_, mid_back_layers) = toast_stack_card_layers(&mid);
    let mid_first_back = mid_back_layers
        .iter()
        .find(|rect| rect.width < collapsed_front.width && rect.width > collapsed_first_back.width)
        .copied()
        .expect("first animated back layer should have intermediate width");
    let mid_faded_card = mid
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| {
            shape.stroke_width <= 0.1
                && shape.rect.width == mid_first_back.width
                && shape.rect.height == mid_first_back.height
                && shape.color.a < 255
        })
        .expect("intermediate card layer should fade while expanding");
    assert!(
        (mid_first_back.y - collapsed_first_back.y).abs() > dp(4.0)
            && mid_first_back.width > collapsed_first_back.width
            && mid_first_back.width < collapsed_front.width,
        "back layer should interpolate y and width: collapsed={collapsed_first_back:?}, mid={mid_first_back:?}, front={collapsed_front:?}, faded_alpha={}",
        mid_faded_card.color.a
    );
}

#[test]
fn toast_stack_top_placements_animate_new_front_toast_while_collapsed() {
    run_with_large_stack(toast_stack_top_placements_animate_new_front_toast_while_collapsed_impl);
}

fn toast_stack_top_placements_animate_new_front_toast_while_collapsed_impl() {
    for placement in [
        ToastPlacement::TopStart,
        ToastPlacement::TopCenter,
        ToastPlacement::TopEnd,
    ] {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        let now = Instant::now();
        let old_created_at = now - Duration::from_secs(1);

        for index in 1..=4 {
            queue.push_at(
                Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
                old_created_at,
            );
        }

        let tree = WidgetTree::new(
            Stack::new()
                .child(Text::new("content"))
                .child(ToastHost::new(queue.clone()).placement(placement)),
        );
        let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
        let _ = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        );

        queue.push_at(Toast::new("toast 5").duration(Duration::from_secs(10)), now);

        let mid = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now + Duration::from_millis(60),
        );
        let settled = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now + Duration::from_millis(500),
        );

        let mid_frame = toast_label_frame(&mid, "toast 5");
        let settled_frame = toast_label_frame(&settled, "toast 5");
        match placement {
            ToastPlacement::TopStart => assert!(
                mid_frame.x < settled_frame.x - dp(10.0),
                "new collapsed top-start toast should slide in from the leading edge: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            ToastPlacement::TopCenter => assert!(
                mid_frame.y < settled_frame.y - dp(5.0),
                "new collapsed top-center toast should slide in from above: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            ToastPlacement::TopEnd => assert!(
                mid_frame.x > settled_frame.x + dp(10.0),
                "new collapsed top-end toast should slide in from the trailing edge: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn toast_mid_enter_dismiss_preserves_card_and_sibling_continuity() {
    run_with_large_stack(toast_mid_enter_dismiss_preserves_card_and_sibling_continuity_impl);
}

fn toast_mid_enter_dismiss_preserves_card_and_sibling_continuity_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let created_at = Instant::now();
    queue.push_at(
        Toast::new("older toast").duration(Duration::from_secs(10)),
        created_at - Duration::from_secs(1),
    );
    let tree = WidgetTree::new(
        Stack::new().child(ToastHost::new(queue.clone()).placement(ToastPlacement::TopEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let baseline = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        created_at,
    );
    let baseline_older = toast_label_frame(&baseline, "older toast");
    let baseline_hits = baseline.overlay_hit_regions.len();

    let entering_id = queue.push_at(
        Toast::new("entering toast").duration(Duration::from_secs(10)),
        created_at,
    );
    let dismiss_at = created_at + Duration::from_millis(80);
    let immediately_after_insert = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        created_at,
    );
    assert_rect_near(
        toast_label_frame(&immediately_after_insert, "older toast"),
        baseline_older,
        dp(0.01),
        "a zero-occupancy entering toast must not jump its sibling",
    );

    let just_before_dismiss = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        dismiss_at,
    );
    let entering_before = toast_label_frame(&just_before_dismiss, "entering toast");
    let older_before = toast_label_frame(&just_before_dismiss, "older toast");
    assert!(
        older_before.y > baseline_older.y + dp(1.0),
        "the older sibling should be part-way through insertion reflow: baseline={baseline_older:?}, mid={older_before:?}"
    );
    assert!(queue.dismiss_at(entering_id, dismiss_at));
    let at_dismiss = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        dismiss_at,
    );
    assert_rect_near(
        toast_label_frame(&at_dismiss, "entering toast"),
        entering_before,
        dp(0.01),
        "dismiss must start from the exact in-flight card transform",
    );
    assert_rect_near(
        toast_label_frame(&at_dismiss, "older toast"),
        older_before,
        dp(0.01),
        "dismiss must preserve the sibling flow position at the reversal instant",
    );
    assert_eq!(
        at_dismiss.overlay_hit_regions.len(),
        baseline_hits,
        "the closing card must release every hit on the first exit frame"
    );
    assert!(
        just_before_dismiss.overlay_hit_regions.len() > at_dismiss.overlay_hit_regions.len(),
        "the entering card should have owned interactions before dismiss"
    );

    let exit_mid = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        dismiss_at + Duration::from_millis(70),
    );
    let older_exit_mid = toast_label_frame(&exit_mid, "older toast");
    assert!(
        older_exit_mid.y > baseline_older.y && older_exit_mid.y < older_before.y,
        "the sibling should flow continuously back toward its old slot: baseline={baseline_older:?}, dismiss={older_before:?}, mid={older_exit_mid:?}"
    );

    let exit_done = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        dismiss_at + Duration::from_secs(1),
    );
    assert_rect_near(
        toast_label_frame(&exit_done, "older toast"),
        baseline_older,
        dp(0.01),
        "removing the zero-occupancy card must not leave a final reflow jump",
    );
    assert!(
        !exit_done
            .scene
            .overlay_texts
            .iter()
            .any(|text| text.content.as_ref() == "entering toast"),
        "the exiting card should be removed after its motion window"
    );
}

#[test]
fn toast_reduced_motion_insertion_and_dismiss_land_immediately() {
    run_with_large_stack(toast_reduced_motion_insertion_and_dismiss_land_immediately_impl);
}

fn toast_reduced_motion_insertion_and_dismiss_land_immediately_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    queue.push_at(
        Toast::new("older toast").duration(Duration::from_secs(10)),
        now - Duration::from_secs(1),
    );
    let tree = WidgetTree::new(
        Stack::new().child(ToastHost::new(queue.clone()).placement(ToastPlacement::TopEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let baseline = compute_scene_at_with_reduced_motion(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
        true,
    );
    let baseline_older = toast_label_frame(&baseline, "older toast");
    let baseline_hits = baseline.overlay_hit_regions.len();

    let inserted = queue.push_at(
        Toast::new("instant toast").duration(Duration::from_secs(10)),
        now,
    );
    let inserted_scene = compute_scene_at_with_reduced_motion(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
        true,
    );
    assert!(
        toast_label_frame(&inserted_scene, "older toast").y > baseline_older.y + dp(20.0),
        "reduced motion should place the sibling directly in its final inserted slot"
    );

    assert!(queue.dismiss_at(inserted, now));
    let dismissed_scene = compute_scene_at_with_reduced_motion(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
        true,
    );
    assert_rect_near(
        toast_label_frame(&dismissed_scene, "older toast"),
        baseline_older,
        dp(0.01),
        "reduced motion should remove the flow slot without an intermediate frame",
    );
    assert_eq!(dismissed_scene.overlay_hit_regions.len(), baseline_hits);
    assert!(
        !dismissed_scene
            .scene
            .overlay_texts
            .iter()
            .any(|text| text.content.as_ref() == "instant toast"),
        "reduced motion dismiss should remove the card immediately"
    );
}

#[test]
fn toast_manual_dismiss_and_clear_use_exit_animation() {
    run_with_large_stack(|| {
        toast_manual_dismiss_and_clear_use_exit_animation_impl();
    });
}

fn toast_manual_dismiss_and_clear_use_exit_animation_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    let first = queue.push_at(
        Toast::new("toast 1").duration(Duration::from_secs(10)),
        created_at,
    );
    queue.push_at(
        Toast::new("toast 2").duration(Duration::from_secs(10)),
        created_at,
    );

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);

    assert!(queue.dismiss_at(first, now));
    assert!(
        !queue.pause_at(first, now + Duration::from_millis(50)),
        "hover pause should not hold a toast that is already exiting"
    );
    let dismiss_mid = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(150),
    );
    let dismiss_mid_labels = toast_message_labels(&dismiss_mid);
    assert!(
        dismiss_mid_labels.iter().any(|label| label == "toast 1"),
        "manual dismiss should keep toast visible during exit animation, got {dismiss_mid_labels:?}"
    );
    let dismiss_done = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(301),
    );
    let dismiss_done_labels = toast_message_labels(&dismiss_done);
    assert!(
        !dismiss_done_labels.iter().any(|label| label == "toast 1")
            && dismiss_done_labels.iter().any(|label| label == "toast 2"),
        "manual dismiss should remove only after exit completes, got {dismiss_done_labels:?}"
    );
    assert_eq!(
        (
            dismiss_mid.hit_regions.len(),
            dismiss_mid.overlay_hit_regions.len()
        ),
        (
            dismiss_done.hit_regions.len(),
            dismiss_done.overlay_hit_regions.len()
        ),
        "an exiting toast may remain visible but must add no interactions beyond the live toast"
    );

    let clear_at = now + Duration::from_secs(1);
    queue.clear_at(clear_at);
    let clear_mid = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        clear_at + Duration::from_millis(150),
    );
    let clear_mid_labels = toast_message_labels(&clear_mid);
    assert!(
        clear_mid_labels.iter().any(|label| label == "toast 2"),
        "clear should keep toasts visible during exit animation, got {clear_mid_labels:?}"
    );

    let clear_done = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        clear_at + Duration::from_millis(301),
    );
    let clear_done_labels = toast_message_labels(&clear_done);
    assert!(
        clear_done_labels.iter().all(|label| label != "toast 2"),
        "clear should remove toasts after exit completes, got {clear_done_labels:?}"
    );
}

fn run_with_large_stack(f: impl FnOnce() + Send + 'static) {
    let handle = std::thread::Builder::new()
        .name("toast-exit-animation-test".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .expect("spawn large-stack toast test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn toast_stack_hover_region_toggles_auto_collapse_state() {
    run_with_large_stack(toast_stack_hover_region_toggles_auto_collapse_state_impl);
}

fn toast_stack_hover_region_toggles_auto_collapse_state_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 480.0, 360.0),
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let hover_regions: Vec<_> = computed
        .overlay_hit_regions
        .iter()
        .filter_map(|hit| match &hit.interaction {
            HitInteraction::Widget { interactions, .. }
                if interactions.on_mouse_enter.is_some()
                    && interactions.on_mouse_leave.is_some() =>
            {
                Some(interactions.clone())
            }
            _ => None,
        })
        .collect();

    assert!(
        !hover_regions.is_empty(),
        "collapsible toast stack should emit hover enter/leave hit regions"
    );
    assert!(
        !queue.stack_expanded(),
        "toast stack should start collapsed when more than three toasts are visible"
    );

    let mut vm = ();
    for interactions in hover_regions.iter() {
        if let Some(command) = interactions.on_mouse_enter.as_ref() {
            command.execute(&mut vm);
        }
    }
    assert!(
        queue.stack_expanded(),
        "hover enter should expand the toast stack"
    );

    for interactions in hover_regions.iter().rev() {
        if let Some(command) = interactions.on_mouse_leave.as_ref() {
            command.execute(&mut vm);
        }
    }
    assert!(
        !queue.stack_expanded(),
        "hover leave should collapse the toast stack"
    );
}

#[test]
fn toast_close_button_aligns_to_card_trailing_edge() {
    run_with_large_stack(toast_close_button_aligns_to_card_trailing_edge_impl);
}

fn toast_close_button_aligns_to_card_trailing_edge_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("body")
            .title("Saved")
            .duration(Duration::from_secs(10)),
        created_at,
    );

    let tree = WidgetTree::new(Stack::new().child(Text::new("content")).child(
        ToastHost::new(queue.clone()).style_full(|ctx| {
            let mut style = ToastStyle::default_for_theme(ctx.theme);
            style.max_width = dp(320.0);
            style.padding = Insets::all(dp(12.0));
            style
        }),
    ));
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let card_rect = computed
        .scene
        .overlay_shapes
        .iter()
        .max_by(|left, right| {
            let left_area = left.rect.width.get() * left.rect.height.get();
            let right_area = right.rect.width.get() * right.rect.height.get();
            left_area
                .partial_cmp(&right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|shape| shape.rect)
        .expect("toast card background should render");
    let close_icon = computed
        .scene
        .overlay_textures
        .iter()
        .max_by(|left, right| {
            left.frame
                .x
                .partial_cmp(&right.frame.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("toast close icon should render");
    assert!(!computed
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "\u{e5cd}"));

    assert!(
        close_icon.frame.x > card_rect.right() - dp(48.0),
        "close button should sit near the card trailing edge: card={card_rect:?}, close={:?}",
        close_icon.frame
    );
}

#[test]
fn toast_kind_icon_is_centered_inside_circle() {
    run_with_large_stack(toast_kind_icon_is_centered_inside_circle_impl);
}

fn toast_kind_icon_is_centered_inside_circle_impl() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("body")
            .title("Info")
            .kind(ToastKind::Info)
            .duration(Duration::from_secs(10)),
        created_at,
    );

    let tree = WidgetTree::new(Stack::new().child(Text::new("content")).child(
        ToastHost::new(queue.clone()).style_full(|ctx| {
            let mut style = ToastStyle::default_for_theme(ctx.theme);
            style.max_width = dp(320.0);
            style.padding = Insets::all(dp(12.0));
            style
        }),
    ));
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let circle = computed
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| {
            (shape.rect.width - dp(18.0)).abs() <= dp(0.1)
                && (shape.rect.height - dp(18.0)).abs() <= dp(0.1)
        })
        .expect("toast kind icon circle should render");
    let icon = computed
        .scene
        .overlay_texts
        .iter()
        .find(|text| text.content.as_ref() == "i")
        .expect("toast info icon should render");

    let circle_center_x = circle.rect.x + circle.rect.width * 0.5;
    let circle_center_y = circle.rect.y + circle.rect.height * 0.5;
    let icon_center_x = icon.frame.x + icon.frame.width * 0.5;
    let icon_center_y = icon.frame.y + icon.frame.height * 0.5;

    assert!(
        icon.frame.width < circle.rect.width,
        "toast icon should be laid out by glyph width, not by the full circle frame: circle={:?}, icon={:?}",
        circle.rect,
        icon.frame
    );
    assert!(
        (icon_center_x - circle_center_x).abs() <= dp(1.0)
            && (icon_center_y - circle_center_y).abs() <= dp(1.0),
        "toast icon should be centered in its circle: circle={:?}, icon={:?}",
        circle.rect,
        icon.frame
    );
}

#[test]
fn toast_animation_deadline_uses_window_60_120_and_144_hz_clock() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let created_at = Instant::now();
    let sample_at = created_at + Duration::from_millis(70);
    let mut reference_frame = None;

    for (refresh_rate, expected_interval) in [
        (60_000, Duration::from_nanos(16_666_667)),
        (120_000, Duration::from_nanos(8_333_333)),
        (144_000, Duration::from_nanos(6_944_444)),
    ] {
        let theme = Theme::light();
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        queue.push_at(
            Toast::new("Refresh-aware toast").duration(Duration::from_secs(10)),
            created_at,
        );
        let tree = WidgetTree::new(
            Popover::new(Button::new("Toast anchor").size(dp(120.0), dp(36.0)))
                .content(Stack::new().child(ToastHost::new(queue)))
                .open(true),
        );
        let mut animations = AnimationEngine::default();
        let mut layout = tree.build_scene_layout_at(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            sample_at,
        );
        layout.set_frame_clock(FrameClockSnapshot::for_refresh_rate(
            sample_at,
            Some(refresh_rate),
        ));
        let style_sheet = crate::ui::widget::StyleSheet::default();
        let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
            &font_manager,
            &layout,
            &theme,
            &media,
            &mut animations,
            false,
            None,
            None,
            &WidgetStateMap::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            sample_at,
            &HashMap::new(),
            None,
            None,
            &style_sheet,
        );

        assert_eq!(
            collected.next_toast_wakeup,
            Some(sample_at + expected_interval),
            "toast cadence should follow {refresh_rate}mHz"
        );
        let frame = toast_label_frame(&collected.computed, "Refresh-aware toast");
        if let Some(reference) = reference_frame {
            assert_rect_near(
                frame,
                reference,
                dp(0.01),
                "refresh cadence must not change animation geometry at one absolute instant",
            );
        } else {
            reference_frame = Some(frame);
        }
    }
}

#[test]
fn toast_reduced_or_zero_motion_has_no_animation_frame_deadline() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);

    for (reduced_motion, zero_motion) in [(true, false), (false, true)] {
        let mut theme = Theme::light();
        if zero_motion {
            theme.motion.fast_ms = 0;
            theme.motion.normal_ms = 0;
            theme.motion.slow_ms = 0;
        }
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        let now = Instant::now();
        queue.push_at(
            Toast::new("Motion-safe toast").duration(Duration::from_secs(10)),
            now,
        );
        let tree = WidgetTree::new(Stack::new().child(ToastHost::new(queue)));
        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout_at(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            now,
        );
        let style_sheet = crate::ui::widget::StyleSheet::default();
        let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
            &font_manager,
            &layout,
            &theme,
            &media,
            &mut animations,
            reduced_motion,
            None,
            None,
            &WidgetStateMap::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            now,
            &HashMap::new(),
            None,
            None,
            &style_sheet,
        );

        assert!(
            !animations.has_active_animations(),
            "reduced/zero motion must not retain an animation slot"
        );
        assert!(
            collected
                .next_toast_wakeup
                .is_some_and(|deadline| deadline >= now + Duration::from_secs(9)),
            "only the toast expiry deadline should remain; got {:?}",
            collected.next_toast_wakeup
        );
    }
}

#[cfg(feature = "bench-support")]
#[test]
fn toast_base_scene_replay_matches_full_collect_for_motion_and_closing_hits() {
    run_with_large_stack(|| {
        for placement in [ToastPlacement::TopEnd, ToastPlacement::BottomEnd] {
            let theme = Theme::default();
            let font_manager = FontManager::new(&FontCatalog::default());
            let media = test_media();
            let context = test_context();
            let queue = ToastQueue::<()>::new(&context);
            let created_at = Instant::now();
            queue.push_at(
                Toast::new("first moving toast")
                    .title("First")
                    .persistent(true),
                created_at,
            );
            queue.push_at(
                Toast::new("second moving toast")
                    .title("Second")
                    .persistent(true),
                created_at,
            );
            let tree =
                WidgetTree::new(Stack::new().child(ToastHost::new(queue).placement(placement)));
            let sample_at = created_at + Duration::from_millis(93);
            assert_toast_base_scene_replay_matches(
                &tree,
                &font_manager,
                &theme,
                &media,
                Rect::new(0.0, 0.0, 640.0, 480.0),
                sample_at,
                true,
            );
        }

        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        let now = Instant::now();
        let closing = queue.push_at(
            Toast::new("closing toast")
                .title("Closing")
                .persistent(true),
            now - Duration::from_secs(1),
        );
        queue.push_at(
            Toast::new("live toast").title("Live").persistent(true),
            now - Duration::from_secs(1),
        );
        assert!(queue.dismiss_at(closing, now));
        let tree = WidgetTree::new(
            Stack::new().child(ToastHost::new(queue).placement(ToastPlacement::BottomEnd)),
        );
        assert_toast_base_scene_replay_matches(
            &tree,
            &font_manager,
            &theme,
            &media,
            Rect::new(0.0, 0.0, 640.0, 480.0),
            now + Duration::from_millis(71),
            true,
        );

        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        let now = Instant::now();
        queue.push_at(
            Toast::new("unsupported nested shadow")
                .title("Fallback")
                .persistent(true),
            now - Duration::from_secs(1),
        );
        let tree = WidgetTree::new(Stack::new().child(ToastHost::new(queue).style_full(
            |context| {
                let mut style = ToastStyle::default_for_theme(context.theme);
                style.close_button.surface.shadow =
                    Some(Value::Static(context.theme.elevation.sm.clone()));
                style
            },
        )));
        assert_toast_base_scene_replay_matches(
            &tree,
            &font_manager,
            &theme,
            &media,
            Rect::new(0.0, 0.0, 640.0, 480.0),
            now,
            false,
        );
    });
}

#[cfg(feature = "bench-support")]
fn assert_toast_base_scene_replay_matches(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    viewport: Rect,
    now: Instant,
    expect_replay: bool,
) {
    let mut animations = AnimationEngine::default();
    let layout = tree.build_scene_layout_at(
        font_manager,
        theme,
        media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
    );
    let _ = with_prepared_toast_card_cache(|| {
        collect_toast_layout_scene(
            tree,
            &layout,
            font_manager,
            theme,
            media,
            &mut animations,
            viewport,
            now,
        )
    });
    crate::ui::widget::toast_scene_bench_profile::reset();
    let replay = with_prepared_toast_card_cache(|| {
        with_toast_base_scene_replay(|| {
            collect_toast_layout_scene(
                tree,
                &layout,
                font_manager,
                theme,
                media,
                &mut animations,
                viewport,
                now,
            )
        })
    });
    let replay_profile = crate::ui::widget::toast_scene_bench_profile::snapshot();
    if expect_replay {
        assert!(replay_profile.base_scene_replay_hits > 0);
        assert_eq!(replay_profile.base_scene_replay_fallbacks, 0);
    } else {
        assert_eq!(replay_profile.base_scene_replay_hits, 0);
        assert!(replay_profile.base_scene_replay_fallbacks > 0);
    }
    let control = with_prepared_toast_card_cache(|| {
        collect_toast_layout_scene(
            tree,
            &layout,
            font_manager,
            theme,
            media,
            &mut animations,
            viewport,
            now,
        )
    });
    assert_eq!(replay.next_toast_wakeup, control.next_toast_wakeup);
    assert_toast_computed_scene_equivalent(&replay.computed, &control.computed);
}

#[cfg(feature = "bench-support")]
fn collect_toast_layout_scene(
    tree: &WidgetTree<()>,
    layout: &ResolvedSceneLayout<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> CollectedSceneCache<()> {
    let style_sheet = crate::ui::widget::StyleSheet::default();
    tree.collect_scene_cache_from_layout_with_focus_value_at(
        font_manager,
        layout,
        theme,
        media,
        animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        now,
        &HashMap::new(),
        None,
        None,
        &style_sheet,
    )
}

#[cfg(feature = "bench-support")]
fn assert_toast_computed_scene_equivalent(
    actual: &ComputedScene<()>,
    expected: &ComputedScene<()>,
) {
    assert_eq!(actual.scene.backdrop_blurs, expected.scene.backdrop_blurs);
    assert_eq!(actual.scene.brushes, expected.scene.brushes);
    assert_eq!(
        actual.scene.canvas_composites.len(),
        expected.scene.canvas_composites.len()
    );
    assert_eq!(actual.scene.meshes.len(), expected.scene.meshes.len());
    assert_eq!(
        actual.scene.overlay_shapes.len(),
        expected.scene.overlay_shapes.len()
    );
    for (actual, expected) in actual
        .scene
        .overlay_shapes
        .iter()
        .zip(expected.scene.overlay_shapes.iter())
    {
        assert_eq!(actual.rect, expected.rect);
        assert_eq!(actual.color, expected.color);
        assert_eq!(actual.corner_radius, expected.corner_radius);
        assert_eq!(actual.stroke_width, expected.stroke_width);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert_eq!(actual.clip_mask, expected.clip_mask);
    }
    assert_eq!(
        actual.scene.overlay_textures.len(),
        expected.scene.overlay_textures.len()
    );
    for (actual, expected) in actual
        .scene
        .overlay_textures
        .iter()
        .zip(expected.scene.overlay_textures.iter())
    {
        assert_eq!(actual.texture.id(), expected.texture.id());
        assert_eq!(actual.frame, expected.frame);
        assert_eq!(actual.quad, expected.quad);
        assert_eq!(actual.uv_rect, expected.uv_rect);
        assert_eq!(actual.corner_radius, expected.corner_radius);
        assert_eq!(actual.opacity, expected.opacity);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert_eq!(actual.clip_mask, expected.clip_mask);
        assert_eq!(actual.mask_tint, expected.mask_tint);
    }
    assert_eq!(actual.scene.overlay_texts, expected.scene.overlay_texts);
    assert_eq!(
        actual.scene.overlay_text_decorations,
        expected.scene.overlay_text_decorations,
    );
    assert_eq!(
        actual.scene.overlay_commands.len(),
        expected.scene.overlay_commands.len()
    );
    let command_kind = |command: &crate::ui::widget::common::RenderCommand| match command {
        crate::ui::widget::common::RenderCommand::Shape(_) => 0_u8,
        crate::ui::widget::common::RenderCommand::Texture(_) => 1,
        crate::ui::widget::common::RenderCommand::Text(_) => 2,
        crate::ui::widget::common::RenderCommand::TextDecoration(_) => 3,
        crate::ui::widget::common::RenderCommand::BackdropBlur(_) => 4,
        crate::ui::widget::common::RenderCommand::Brush(_) => 5,
        crate::ui::widget::common::RenderCommand::CanvasComposite(_) => 6,
        crate::ui::widget::common::RenderCommand::Mesh(_) => 7,
        #[cfg(feature = "video")]
        crate::ui::widget::common::RenderCommand::VideoTexture(_) => 8,
    };
    assert_eq!(
        actual
            .scene
            .overlay_commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>(),
        expected
            .scene
            .overlay_commands
            .iter()
            .map(command_kind)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        actual.overlay_hit_regions.len(),
        expected.overlay_hit_regions.len()
    );
    for (actual, expected) in actual
        .overlay_hit_regions
        .iter()
        .zip(expected.overlay_hit_regions.iter())
    {
        assert_eq!(actual.rect, expected.rect);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert!(matches!(actual.geometry, HitGeometry::Rect));
        assert!(matches!(expected.geometry, HitGeometry::Rect));
        assert_eq!(actual.transform_chain, expected.transform_chain);
        assert_eq!(actual.scope_path, expected.scope_path);
        assert_eq!(
            actual.interaction.target_id(),
            expected.interaction.target_id()
        );
        assert_eq!(actual.focus.is_some(), expected.focus.is_some());
        assert_eq!(actual.gpu_scroll_container, expected.gpu_scroll_container);
    }
    assert_eq!(actual.scroll_regions.len(), expected.scroll_regions.len());
    for (actual, expected) in actual
        .scroll_regions
        .iter()
        .zip(expected.scroll_regions.iter())
    {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.content_viewport, expected.content_viewport);
        assert_eq!(actual.visible_frame, expected.visible_frame);
        assert_eq!(actual.content_bounds, expected.content_bounds);
        assert_eq!(
            actual.gpu_base_scroll_offset,
            expected.gpu_base_scroll_offset
        );
        assert_eq!(actual.scroll_offset, expected.scroll_offset);
        assert_eq!(actual.overflow_x, expected.overflow_x);
        assert_eq!(actual.overflow_y, expected.overflow_y);
        assert_eq!(actual.horizontal_track, expected.horizontal_track);
        assert_eq!(actual.horizontal_thumb, expected.horizontal_thumb);
        assert_eq!(actual.vertical_track, expected.vertical_track);
        assert_eq!(actual.vertical_thumb, expected.vertical_thumb);
    }
    assert_eq!(actual.focus_scopes, expected.focus_scopes);
    assert_toast_accessibility_fragments_equivalent(
        &actual.accessibility_fragments,
        &expected.accessibility_fragments,
    );
    assert_eq!(actual.overlay_layers.len(), expected.overlay_layers.len());
    for (actual, expected) in actual.overlay_layers.iter().zip(&expected.overlay_layers) {
        assert_toast_accessibility_fragments_equivalent(
            &actual.accessibility_fragments,
            &expected.accessibility_fragments,
        );
    }
    assert_eq!(actual.overlay_layer_graph, expected.overlay_layer_graph);
    assert_eq!(actual.portal_entries.len(), expected.portal_entries.len());
    assert_eq!(
        actual.external_portal_requests.len(),
        expected.external_portal_requests.len()
    );
    assert_eq!(
        (
            actual.portal_overlay_counts.shapes,
            actual.portal_overlay_counts.textures,
            actual.portal_overlay_counts.meshes,
            actual.portal_overlay_counts.texts,
            actual.portal_overlay_counts.text_decorations,
            actual.portal_overlay_counts.commands,
            actual.portal_overlay_counts.hits,
            actual.portal_overlay_counts.close_handlers,
            actual.portal_overlay_counts.focus_scopes,
            actual.portal_overlay_counts.accessibility_fragments,
        ),
        (
            expected.portal_overlay_counts.shapes,
            expected.portal_overlay_counts.textures,
            expected.portal_overlay_counts.meshes,
            expected.portal_overlay_counts.texts,
            expected.portal_overlay_counts.text_decorations,
            expected.portal_overlay_counts.commands,
            expected.portal_overlay_counts.hits,
            expected.portal_overlay_counts.close_handlers,
            expected.portal_overlay_counts.focus_scopes,
            expected.portal_overlay_counts.accessibility_fragments,
        )
    );
    assert_eq!(actual.overlay_anchors, expected.overlay_anchors);
    assert_eq!(actual.ime_cursor_area, expected.ime_cursor_area);
    assert_eq!(
        actual.transform_records.len(),
        expected.transform_records.len()
    );
    assert_eq!(
        actual.virtual_state_updates.len(),
        expected.virtual_state_updates.len()
    );
    assert_eq!(
        actual.dependencies.dependency_count(),
        expected.dependencies.dependency_count(),
    );
    assert_eq!(
        actual.dependencies.all_owners(),
        expected.dependencies.all_owners()
    );
}

#[cfg(feature = "bench-support")]
fn assert_toast_accessibility_fragments_equivalent(
    actual: &[crate::ui::widget::AccessibilityFragment<()>],
    expected: &[crate::ui::widget::AccessibilityFragment<()>],
) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            actual.source_window_instance_id,
            expected.source_window_instance_id
        );
        assert_eq!(
            actual.source_publication_generation,
            expected.source_publication_generation
        );
        assert_eq!(
            actual
                .source_open
                .as_ref()
                .map(crate::ui::layout::Value::resolve_untracked),
            expected
                .source_open
                .as_ref()
                .map(crate::ui::layout::Value::resolve_untracked)
        );
        assert_eq!(actual.owner_path, expected.owner_path);
        assert_eq!(actual.scope_path, expected.scope_path);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert_eq!(
            actual.has_duplicate_widget_ids,
            expected.has_duplicate_widget_ids
        );
        assert_eq!(actual.resolved_root.id, expected.resolved_root.id);
        assert_eq!(actual.nodes.len(), expected.nodes.len());
        for (actual, expected) in actual.nodes.iter().zip(&expected.nodes) {
            assert_eq!(actual.widget_id, expected.widget_id);
            assert_eq!(actual.resolved_path, expected.resolved_path);
            assert_eq!(actual.bounds, expected.bounds);
            assert_eq!(actual.clip_rect, expected.clip_rect);
            assert_eq!(actual.children, expected.children);
            assert_eq!(actual.hits.len(), expected.hits.len());
            assert_eq!(actual.scroll_regions.len(), expected.scroll_regions.len());
        }
    }
}

fn compute_scene_at(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> ComputedScene<()> {
    compute_scene_at_with_reduced_motion(
        tree,
        font_manager,
        theme,
        media,
        animations,
        viewport,
        now,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn compute_scene_at_with_reduced_motion(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
    reduced_motion: bool,
) -> ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        font_manager,
        theme,
        media,
        UnitContext::default(),
        animations,
        reduced_motion,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    )
}

fn toast_message_labels(computed: &ComputedScene<()>) -> Vec<String> {
    computed
        .scene
        .overlay_texts
        .iter()
        .filter_map(|text| {
            text.content
                .starts_with("toast ")
                .then(|| text.content.to_string())
        })
        .collect()
}

fn toast_label_frame(computed: &ComputedScene<()>, label: &str) -> Rect {
    computed
        .scene
        .overlay_texts
        .iter()
        .find_map(|text| (text.content.as_ref() == label).then_some(text.frame))
        .unwrap_or_else(|| panic!("toast label {label:?} should render"))
}

fn assert_rect_near(actual: Rect, expected: Rect, tolerance: Dp, message: &str) {
    assert!(
        (actual.x - expected.x).abs() <= tolerance
            && (actual.y - expected.y).abs() <= tolerance
            && (actual.width - expected.width).abs() <= tolerance
            && (actual.height - expected.height).abs() <= tolerance,
        "{message}: actual={actual:?}, expected={expected:?}, tolerance={tolerance:?}"
    );
}

fn toast_stack_card_layers(computed: &ComputedScene<()>) -> (Rect, Vec<Rect>) {
    let mut cards: Vec<_> = computed
        .scene
        .overlay_shapes
        .iter()
        .filter_map(|shape| {
            (shape.stroke_width <= 0.1
                && shape.rect.width >= dp(180.0)
                && shape.rect.height >= dp(40.0))
            .then_some(shape.rect)
        })
        .collect();
    cards.sort_by(|left, right| {
        let left_area = left.width.get() * left.height.get();
        let right_area = right.width.get() * right.height.get();
        right_area
            .partial_cmp(&left_area)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let front = cards
        .first()
        .copied()
        .expect("toast stack should have a front card");
    let mut back_layers: Vec<_> = cards
        .into_iter()
        .filter(|rect| rect.width < front.width && rect.y > front.y)
        .collect();
    back_layers.sort_by(|left, right| {
        left.y
            .partial_cmp(&right.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (front, back_layers)
}
