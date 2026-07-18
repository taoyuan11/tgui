use std::time::{Duration, Instant};

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::ui::unit::dp;
use crate::ui::widget::{Point, WidgetId};

use super::controller::sample_timeline;
use super::{
    AnimatedValue, AnimationControllerBuilder, AnimationCoordinator, AnimationCurve,
    AnimationEngine, AnimationKey, AnimationSpec, AnimationStatus, FillMode, Keyframes, Playback,
    PlaybackDirection, Transition, WidgetProperty,
};

fn key(property: WidgetProperty) -> AnimationKey {
    AnimationKey::Widget { id: 1, property }
}

#[test]
fn unchanged_target_does_not_restart_animation() {
    let mut engine = AnimationEngine::default();
    let transition = Transition::ease_out(Duration::from_millis(100));
    let start = Instant::now();

    assert_eq!(
        engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start),
        0.0
    );
    let mid = start + Duration::from_millis(50);
    let animated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);
    let repeated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);

    assert_eq!(animated, repeated);
    assert!(engine.has_active_animations());
}

#[test]
fn target_change_continues_from_current_value() {
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();

    engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start);
    engine.resolve_f32(
        key(WidgetProperty::Opacity),
        10.0,
        Some(transition),
        start + Duration::from_millis(1),
    );
    let mid = start + Duration::from_millis(51);
    let current = engine.resolve_f32(key(WidgetProperty::Opacity), 10.0, Some(transition), mid);
    let redirected = engine.resolve_f32(key(WidgetProperty::Opacity), 20.0, Some(transition), mid);

    assert_eq!(current, redirected);
}

#[test]
fn finished_animation_lands_exactly_on_target() {
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();

    engine.resolve_color(
        key(WidgetProperty::Background),
        Color::BLACK,
        Some(transition),
        start,
    );
    engine.resolve_color(
        key(WidgetProperty::Background),
        Color::WHITE,
        Some(transition),
        start + Duration::from_millis(1),
    );

    let end = start + Duration::from_millis(200);
    assert_eq!(
        engine.resolve_color(
            key(WidgetProperty::Background),
            Color::WHITE,
            Some(transition),
            end,
        ),
        Color::WHITE
    );
    assert!(!engine.has_active_animations());
}

#[test]
fn transparent_color_animation_preserves_visible_target_hue() {
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();
    let target = Color::hexa(0xEEF2F8FF);

    engine.resolve_color(
        key(WidgetProperty::Background),
        Color::TRANSPARENT,
        Some(transition),
        start,
    );
    engine.resolve_color(
        key(WidgetProperty::Background),
        target,
        Some(transition),
        start + Duration::from_millis(1),
    );

    let mid = engine.resolve_color(
        key(WidgetProperty::Background),
        target,
        Some(transition),
        start + Duration::from_millis(51),
    );

    assert_eq!((mid.r, mid.g, mid.b), (target.r, target.g, target.b));
    assert!(mid.a > 0 && mid.a < target.a);
}

#[test]
fn refresh_reports_when_animated_values_advance() {
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();

    engine.resolve_point(
        key(WidgetProperty::Offset),
        Point::new(dp(0.0), dp(0.0)),
        Some(transition),
        start,
    );
    engine.resolve_point(
        key(WidgetProperty::Offset),
        Point::new(dp(20.0), dp(0.0)),
        Some(transition),
        start + Duration::from_millis(1),
    );

    let refresh = engine.refresh(start + Duration::from_millis(50));
    assert!(refresh.changed);
    assert!(!refresh.layout_changed);
}

#[test]
fn timed_and_percent_keyframes_land_on_same_value() {
    let timed = Keyframes::timed(Duration::from_millis(200))
        .at(Duration::ZERO, 0.0)
        .at(Duration::from_millis(100), 50.0)
        .at(Duration::from_millis(200), 100.0);
    let percent = Keyframes::percent(Duration::from_millis(200))
        .at_percent(0.0, 0.0)
        .at_percent(0.5, 50.0)
        .at_percent(1.0, 100.0);

    assert_eq!(
        timed.sample_at(Duration::from_millis(100)),
        percent.sample_at(Duration::from_millis(100))
    );
}

#[test]
fn unsorted_keyframes_are_sampled_by_offset_order() {
    let unsorted = Keyframes::timed(Duration::from_millis(200))
        .at(Duration::from_millis(100), 10.0)
        .at(Duration::ZERO, 0.0)
        .at(Duration::from_millis(200), 20.0);
    let sorted = Keyframes::timed(Duration::from_millis(200))
        .at(Duration::ZERO, 0.0)
        .at(Duration::from_millis(100), 10.0)
        .at(Duration::from_millis(200), 20.0);

    assert_eq!(
        unsorted.sample_at(Duration::from_millis(50)),
        sorted.sample_at(Duration::from_millis(50))
    );
    assert_eq!(
        unsorted.sample_at(Duration::from_millis(150)),
        sorted.sample_at(Duration::from_millis(150))
    );
}

#[test]
fn duplicate_keyframe_offsets_keep_first_boundary_value() {
    let keyframes = Keyframes::timed(Duration::from_millis(200))
        .at(Duration::ZERO, 0.0)
        .at(Duration::from_millis(100), 10.0)
        .at(Duration::from_millis(100), 99.0)
        .at(Duration::from_millis(200), 20.0);

    assert_eq!(keyframes.sample_at(Duration::from_millis(100)), Some(10.0));
    assert_eq!(keyframes.sample_at(Duration::from_millis(150)), Some(59.5));
}

#[test]
fn timeline_sampling_respects_alternate_direction() {
    let sample = sample_timeline(
        Duration::from_millis(100),
        Playback::default()
            .repeat(2)
            .direction(PlaybackDirection::Alternate),
        Duration::from_millis(150),
    )
    .expect("sample should exist");

    assert_eq!(sample.cycle_index, 1);
    assert!(sample.reversed);
}

#[test]
fn timeline_sampling_exact_cycle_boundary_stays_on_previous_cycle() {
    let sample = sample_timeline(
        Duration::from_millis(100),
        Playback::default().repeat(3),
        Duration::from_millis(100),
    )
    .expect("boundary sample should exist");

    assert_eq!(sample.cycle_index, 0);
    assert_eq!(sample.local_time, Duration::from_millis(100));
    assert!(!sample.completed);
}

#[test]
fn controller_updates_animated_value_and_completes() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation.clone())
        .track(
            value.clone(),
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .curve(AnimationCurve::Linear)
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 10.0),
            ),
        )
        .build();

    handle.play();
    assert!(
        coordinator
            .refresh_and_next_frame_deadline(Instant::now() + Duration::from_millis(50), true)
            .changed
    );
    assert!(value.get() > 0.0);
    coordinator.refresh_and_next_frame_deadline(Instant::now() + Duration::from_millis(150), true);
    assert_eq!(handle.status(), AnimationStatus::Completed);
}

#[test]
fn controller_frame_visits_only_queued_running_controllers() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let handles = (0..1024)
        .map(|_| AnimationControllerBuilder::new(coordinator.clone(), invalidation.clone()).build())
        .collect::<Vec<_>>();

    let idle_frame = coordinator.refresh_and_next_frame_deadline(Instant::now(), false);
    assert_eq!(idle_frame.visited_controllers, 0);
    assert_eq!(idle_frame.next_deadline, None);

    handles[517].play();
    let running_frame = coordinator.refresh_and_next_frame_deadline(Instant::now(), false);

    assert_eq!(running_frame.visited_controllers, 1);
    assert!(!running_frame.changed);
    assert!(running_frame.next_deadline.is_some());
}

#[test]
fn paused_then_resumed_controller_keeps_one_coordinator_entry() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation).build();

    handle.play();
    handle.pause();
    handle.resume();
    let frame = coordinator.refresh_and_next_frame_deadline(Instant::now(), false);

    assert_eq!(frame.visited_controllers, 1);
    assert!(frame.next_deadline.is_some());
}

#[test]
fn completed_controller_is_pruned_and_restart_requeues_it() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .track(
            value,
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 1.0),
            ),
        )
        .build();

    handle.play();
    let completed =
        coordinator.refresh_and_next_frame_deadline(Instant::now() + Duration::from_secs(1), true);
    assert_eq!(completed.visited_controllers, 1);
    assert_eq!(completed.next_deadline, None);
    assert_eq!(handle.status(), AnimationStatus::Completed);

    let idle = coordinator.refresh_and_next_frame_deadline(Instant::now(), false);
    assert_eq!(idle.visited_controllers, 0);

    handle.restart();
    let restarted = coordinator.refresh_and_next_frame_deadline(Instant::now(), false);
    assert_eq!(restarted.visited_controllers, 1);
    assert!(restarted.next_deadline.is_some());
}

#[test]
fn fill_mode_none_hides_values_outside_range() {
    assert!(sample_timeline(
        Duration::from_millis(100),
        Playback::default()
            .delay(Duration::from_millis(10))
            .fill_mode(FillMode::None),
        Duration::ZERO,
    )
    .is_none());
}

#[test]
fn refresh_skips_settled_slots_without_reporting_change() {
    // 已稳定(无活动动画)的槽位在 refresh 中必须被跳过,且永不报告变化。
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();
    engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start);
    // 槽位已稳定:任意未来时刻 refresh 都不应报告变化,也无活动动画。
    let refresh = engine.refresh(start + Duration::from_secs(10));
    assert!(!refresh.changed);
    assert!(!engine.has_active_animations());
}

#[test]
fn sparse_refresh_visits_only_running_slots() {
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    for id in 0..4096 {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            0.0,
            None,
            start,
        );
    }

    let active_key = AnimationKey::Widget {
        id: 2048,
        property: WidgetProperty::Opacity,
    };
    engine.resolve_f32(
        active_key,
        1.0,
        Some(Transition::linear(Duration::from_secs(1))),
        start + Duration::from_millis(1),
    );

    let refresh = engine.refresh(start + Duration::from_millis(501));
    assert!(refresh.changed);
    assert_eq!(refresh.visited_slots, 1);
    assert_eq!(refresh.scene_widget_ids.as_slice(), &[2048]);
}

#[test]
fn refresh_canonicalizes_mixed_property_targets_without_losing_fallback_scope() {
    use crate::foundation::binding::PropertySlot;

    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let animate_at = start + Duration::from_millis(1);
    let sample_at = animate_at + Duration::from_millis(50);
    let transition = Transition::linear(Duration::from_millis(100));
    let id = 77;

    let opacity = AnimationKey::Widget {
        id,
        property: WidgetProperty::Opacity,
    };
    let background = AnimationKey::Widget {
        id,
        property: WidgetProperty::Background,
    };
    let offset = AnimationKey::Widget {
        id,
        property: WidgetProperty::Offset,
    };
    let unsupported = AnimationKey::Widget {
        id,
        property: WidgetProperty::SpinnerPhase,
    };

    engine.resolve_f32(opacity, 0.0, None, start);
    engine.resolve_color(background, Color::BLACK, None, start);
    engine.resolve_point(offset, Point::ZERO, None, start);
    engine.resolve_f32(unsupported, 0.0, None, start);
    engine.resolve_f32(opacity, 1.0, Some(transition), animate_at);
    engine.resolve_color(background, Color::WHITE, Some(transition), animate_at);
    engine.resolve_point(
        offset,
        Point::new(dp(8.0), dp(4.0)),
        Some(transition),
        animate_at,
    );
    engine.resolve_f32(unsupported, 1.0, Some(transition), animate_at);

    let refresh = engine.refresh(sample_at);
    assert_eq!(refresh.scene_widget_ids.as_slice(), &[id]);
    assert_eq!(
        refresh.scene_property_targets.as_slice(),
        &[
            (WidgetId::from_raw(id), PropertySlot::Background),
            (WidgetId::from_raw(id), PropertySlot::Opacity),
            (WidgetId::from_raw(id), PropertySlot::Offset),
        ]
    );
    assert!(refresh.has_unscoped_scene_changes);
}

#[test]
fn refresh_sorts_and_deduplicates_widget_ids_across_typed_stores() {
    use crate::foundation::binding::PropertySlot;

    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let animate_at = start + Duration::from_millis(1);
    let sample_at = animate_at + Duration::from_millis(50);
    let transition = Transition::linear(Duration::from_millis(100));

    let background = AnimationKey::Widget {
        id: 42,
        property: WidgetProperty::Background,
    };
    let opacity = AnimationKey::Widget {
        id: 7,
        property: WidgetProperty::Opacity,
    };
    let scale = AnimationKey::Widget {
        id: 42,
        property: WidgetProperty::Scale,
    };
    let offset = AnimationKey::Widget {
        id: 19,
        property: WidgetProperty::Offset,
    };
    let width = AnimationKey::Widget {
        id: 42,
        property: WidgetProperty::Width,
    };
    let height = AnimationKey::Widget {
        id: 7,
        property: WidgetProperty::Height,
    };
    let gap = AnimationKey::Widget {
        id: 19,
        property: WidgetProperty::Gap,
    };

    engine.resolve_color(background, Color::BLACK, None, start);
    engine.resolve_f32(opacity, 0.25, None, start);
    engine.resolve_f32(scale, 0.9, None, start);
    engine.resolve_point(offset, Point::ZERO, None, start);
    engine.resolve_dp(width, dp(20.0), None, start);
    engine.resolve_dp(height, dp(18.0), None, start);
    engine.resolve_dp(gap, dp(2.0), None, start);

    engine.resolve_color(background, Color::WHITE, Some(transition), animate_at);
    engine.resolve_f32(opacity, 0.75, Some(transition), animate_at);
    engine.resolve_f32(scale, 1.1, Some(transition), animate_at);
    engine.resolve_point(
        offset,
        Point::new(dp(6.0), dp(3.0)),
        Some(transition),
        animate_at,
    );
    engine.resolve_dp(width, dp(40.0), Some(transition), animate_at);
    engine.resolve_dp(height, dp(30.0), Some(transition), animate_at);
    engine.resolve_dp(gap, dp(8.0), Some(transition), animate_at);

    let refresh = engine.refresh(sample_at);
    assert_eq!(refresh.scene_widget_ids.as_slice(), &[7, 19, 42]);
    assert_eq!(
        refresh.scene_property_targets.as_slice(),
        &[
            (WidgetId::from_raw(7), PropertySlot::Opacity),
            (WidgetId::from_raw(19), PropertySlot::Offset),
            (WidgetId::from_raw(42), PropertySlot::Background),
            (WidgetId::from_raw(42), PropertySlot::Scale),
        ]
    );
    assert!(!refresh.has_unscoped_scene_changes);

    assert_eq!(refresh.layout_widget_ids.as_slice(), &[7, 19, 42]);
    assert_eq!(
        refresh.layout_property_targets.as_slice(),
        &[
            (WidgetId::from_raw(7), PropertySlot::Height),
            (WidgetId::from_raw(42), PropertySlot::Width),
        ]
    );
    assert!(
        refresh.has_unscoped_layout_changes,
        "Gap has no retained layout slot and must preserve the fallback scope"
    );
}

#[test]
fn refresh_marks_only_accessibility_geometry_animations() {
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_secs(1));

    for (id, property) in [
        (1, WidgetProperty::Background),
        (2, WidgetProperty::Opacity),
        (3, WidgetProperty::Offset),
    ] {
        let key = AnimationKey::Widget { id, property };
        if property == WidgetProperty::Offset {
            engine.resolve_point(key, Point::ZERO, None, start);
            engine.resolve_point(
                key,
                Point::new(dp(20.0), dp(10.0)),
                Some(transition),
                start + Duration::from_millis(1),
            );
        } else if property == WidgetProperty::Background {
            engine.resolve_color(key, Color::BLACK, None, start);
            engine.resolve_color(
                key,
                Color::WHITE,
                Some(transition),
                start + Duration::from_millis(1),
            );
        } else {
            engine.resolve_f32(key, 0.0, None, start);
            engine.resolve_f32(key, 1.0, Some(transition), start + Duration::from_millis(1));
        }
    }

    let refresh = engine.refresh(start + Duration::from_millis(501));
    assert!(refresh.changed);
    assert!(refresh.accessibility_geometry_changed);

    let mut paint_only = AnimationEngine::default();
    paint_only.resolve_f32(key(WidgetProperty::Opacity), 0.0, None, start);
    paint_only.resolve_f32(
        key(WidgetProperty::Opacity),
        1.0,
        Some(transition),
        start + Duration::from_millis(1),
    );
    let refresh = paint_only.refresh(start + Duration::from_millis(501));
    assert!(refresh.changed);
    assert!(!refresh.accessibility_geometry_changed);
}

#[test]
fn immediate_settle_keeps_dense_active_index_consistent() {
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_secs(1));
    for id in 0..128 {
        let key = AnimationKey::Widget {
            id,
            property: WidgetProperty::Opacity,
        };
        engine.resolve_f32(key, 0.0, None, start);
        engine.resolve_f32(key, 1.0, Some(transition), start + Duration::from_millis(1));
    }

    // Models reduced motion becoming active while a large group is running. Alternating keys force
    // the dense index through both direct removals and swap-moved entries.
    for id in (0..128).step_by(2) {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            1.0,
            None,
            start + Duration::from_millis(2),
        );
    }

    let refresh = engine.refresh(start + Duration::from_millis(502));
    assert_eq!(refresh.visited_slots, 64);
    assert_eq!(refresh.scene_widget_ids.len(), 64);

    for id in (1..128).step_by(2) {
        assert_eq!(
            engine.resolve_f32(
                AnimationKey::Widget {
                    id,
                    property: WidgetProperty::Opacity,
                },
                1.0,
                Some(Transition::linear(Duration::ZERO)),
                start + Duration::from_millis(503),
            ),
            1.0
        );
    }
    assert!(!engine.has_active_animations());
    assert_eq!(
        engine
            .refresh(start + Duration::from_millis(504))
            .visited_slots,
        0
    );
}

#[test]
fn refresh_completion_removes_active_slots_and_allows_reactivation() {
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_millis(100));
    for id in 0..64 {
        let key = AnimationKey::Widget {
            id,
            property: WidgetProperty::Opacity,
        };
        engine.resolve_f32(key, 0.0, None, start);
        engine.resolve_f32(key, 1.0, Some(transition), start + Duration::from_millis(1));
    }

    let completed = engine.refresh(start + Duration::from_millis(201));
    assert_eq!(completed.visited_slots, 64);
    assert!(!engine.has_active_animations());

    let reactivated_key = AnimationKey::Widget {
        id: 17,
        property: WidgetProperty::Opacity,
    };
    engine.resolve_f32(
        reactivated_key,
        0.0,
        Some(transition),
        start + Duration::from_millis(202),
    );
    let refresh = engine.refresh(start + Duration::from_millis(252));
    assert_eq!(refresh.visited_slots, 1);
    assert_eq!(refresh.scene_widget_ids.as_slice(), &[17]);
}

#[test]
fn idle_settled_slot_below_cap_is_retained_and_still_animates() {
    // 回归保护:软上限以下,长时间空闲的已稳定槽位绝不能被回收 —— 否则后续目标
    // 变化时会丢失过渡(直接跳变)。
    let mut engine = AnimationEngine::default();
    let transition = Transition::linear(Duration::from_millis(100));
    let start = Instant::now();
    engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start);

    // 长时间空闲后 refresh(远超 GC TTL),但槽位数远低于软上限 → 不应回收。
    let later = start + super::engine::SLOT_GC_TTL + Duration::from_secs(3600);
    let _ = engine.refresh(later);
    assert_eq!(engine.debug_total_slots(), 1);

    // 空闲后目标变化:必须从 0.0 平滑过渡而非跳变。
    engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), later);
    let mid = engine.resolve_f32(
        key(WidgetProperty::Opacity),
        1.0,
        Some(transition),
        later + Duration::from_millis(50),
    );
    assert!(
        mid > 0.0 && mid < 1.0,
        "expected mid-transition value, got {mid}"
    );
    assert!(engine.has_active_animations());
}

#[test]
fn over_cap_reclaims_stale_settled_slots_but_keeps_recent() {
    use super::engine::{SLOT_GC_SOFT_CAP, SLOT_GC_TTL};
    let mut engine = AnimationEngine::default();
    let start = Instant::now();

    // 用「无过渡」即时稳定的槽位填到软上限以上,全部 last_touch=start。
    for id in 0..(SLOT_GC_SOFT_CAP as u64 + 4) {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            1.0,
            None,
            start,
        );
    }
    assert!(engine.debug_total_slots() > SLOT_GC_SOFT_CAP);

    // 一个「存活」槽位在更晚时刻被触达。
    let later = start + SLOT_GC_TTL + Duration::from_secs(1);
    engine.resolve_f32(
        AnimationKey::Widget {
            id: 9_999_999,
            property: WidgetProperty::Opacity,
        },
        1.0,
        None,
        later,
    );

    // 此刻 refresh 触发回收:陈旧的已稳定槽位被回收,仅保留刚触达的存活槽位。
    let _ = engine.refresh(later);
    assert_eq!(engine.debug_total_slots(), 1);
}

#[test]
fn over_cap_idle_refresh_throttles_gc_but_still_reclaims_after_ttl() {
    use super::engine::{SLOT_GC_SOFT_CAP, SLOT_GC_SWEEP_INTERVAL, SLOT_GC_TTL};
    let mut engine = AnimationEngine::default();
    let start = Instant::now();

    for id in 0..(SLOT_GC_SOFT_CAP as u64 + 4) {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            1.0,
            None,
            start,
        );
    }

    let _ = engine.refresh(start);
    assert_eq!(engine.debug_slot_gc_sweep_count(), 1);
    assert!(engine.debug_total_slots() > SLOT_GC_SOFT_CAP);

    for step in 1..10 {
        let _ = engine.refresh(start + SLOT_GC_SWEEP_INTERVAL / 10 * step);
    }
    assert_eq!(
        engine.debug_slot_gc_sweep_count(),
        1,
        "idle event refreshes before the deadline must not rescan the retained slot table"
    );

    let expired = start + SLOT_GC_TTL + SLOT_GC_SWEEP_INTERVAL;
    let _ = engine.refresh(expired);
    assert_eq!(engine.debug_slot_gc_sweep_count(), 2);
    assert_eq!(engine.debug_total_slots(), 0);
}

#[test]
fn over_cap_gc_preserves_active_and_window_slots() {
    use super::engine::{SLOT_GC_SOFT_CAP, SLOT_GC_TTL};
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let active_key = AnimationKey::Widget {
        id: 7,
        property: WidgetProperty::Opacity,
    };

    for id in 0..(SLOT_GC_SOFT_CAP as u64 + 4) {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            1.0,
            None,
            start,
        );
        engine.resolve_color(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Background,
            },
            Color::BLACK,
            None,
            start,
        );
    }
    let window_key = AnimationKey::Window(super::WindowProperty::ThemeBackground);
    engine.resolve_color(window_key, Color::WHITE, None, start);
    engine.resolve_f32(
        active_key,
        2.0,
        Some(Transition::linear(Duration::from_secs(60))),
        start + Duration::from_millis(1),
    );

    let expired = start + SLOT_GC_TTL + Duration::from_secs(1);
    let _ = engine.refresh(expired);

    assert!(engine.contains_key(active_key));
    assert!(engine.contains_key(window_key));
    assert!(engine.has_active_animations());
    assert_eq!(engine.debug_total_slots(), 2);
}

#[test]
fn over_cap_same_target_touch_refreshes_settled_slot_liveness() {
    use super::engine::{SLOT_GC_SOFT_CAP, SLOT_GC_TTL};
    let mut engine = AnimationEngine::default();
    let start = Instant::now();
    let live_key = AnimationKey::Widget {
        id: 7,
        property: WidgetProperty::Opacity,
    };

    for id in 0..(SLOT_GC_SOFT_CAP as u64 + 4) {
        engine.resolve_f32(
            AnimationKey::Widget {
                id,
                property: WidgetProperty::Opacity,
            },
            1.0,
            None,
            start,
        );
    }
    assert!(engine.debug_total_slots() > SLOT_GC_SOFT_CAP);

    let later = start + SLOT_GC_TTL + Duration::from_secs(1);
    engine.resolve_f32(live_key, 1.0, None, later);
    let _ = engine.refresh(later);

    assert_eq!(engine.debug_total_slots(), 1);
    assert_eq!(
        engine.resolve_f32(live_key, 1.0, None, later),
        1.0,
        "same-target settled resolve should keep the live slot reachable"
    );
}
