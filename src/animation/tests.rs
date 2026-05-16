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
