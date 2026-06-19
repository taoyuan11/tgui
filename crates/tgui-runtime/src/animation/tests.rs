use std::time::{Duration, Instant};

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::ui::unit::dp;
use crate::ui::widget::Point;

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
    assert!(coordinator.refresh(Instant::now() + Duration::from_millis(50)));
    assert!(value.get() > 0.0);
    coordinator.refresh(Instant::now() + Duration::from_millis(150));
    assert_eq!(handle.status(), AnimationStatus::Completed);
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
