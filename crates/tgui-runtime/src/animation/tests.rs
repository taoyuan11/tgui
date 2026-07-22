use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::ui::unit::dp;
use crate::ui::widget::{Point, WidgetId};

use super::controller::sample_timeline;
use super::{
    AnimatedValue, AnimationControllerBuilder, AnimationCoordinator, AnimationCurve,
    AnimationEngine, AnimationKey, AnimationSpec, AnimationStatus, FillMode, Keyframes, Playback,
    PlaybackDirection, Transition, WidgetProperty, WindowProperty,
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
fn reversed_controllers_use_directional_delay_and_completion_endpoints() {
    for direction in [
        PlaybackDirection::Reverse,
        PlaybackDirection::AlternateReverse,
    ] {
        let invalidation = InvalidationSignal::new();
        let coordinator = AnimationCoordinator::default();
        let value = AnimatedValue::new(-1.0_f32, invalidation.clone());
        let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
            .playback(
                Playback::default()
                    .direction(direction)
                    .delay(Duration::from_millis(10))
                    .fill_mode(FillMode::Both),
            )
            .track(
                value.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(100))
                        .at(Duration::ZERO, 0.0)
                        .at(Duration::from_millis(100), 1.0),
                ),
            )
            .build();
        let start = Instant::now();

        handle.play_at(start);
        coordinator.refresh(start + Duration::from_millis(5), true);
        assert_eq!(value.get(), 1.0, "{direction:?} backwards fill");
        coordinator.refresh(start + Duration::from_millis(10), true);
        assert_eq!(value.get(), 1.0, "{direction:?} active start");
        coordinator.refresh(start + Duration::from_millis(110), true);
        assert_eq!(handle.status(), AnimationStatus::Completed);
        assert_eq!(value.get(), 0.0, "{direction:?} completion");
    }
}

#[test]
fn reversed_declarative_forwards_fill_does_not_restart_after_completion() {
    for direction in [
        PlaybackDirection::Reverse,
        PlaybackDirection::AlternateReverse,
    ] {
        let mut engine = AnimationEngine::default();
        let key = key(WidgetProperty::Opacity);
        let start = Instant::now();
        let transition = Transition::linear(Duration::from_millis(100)).direction(direction);

        engine.resolve_f32(key, 0.0, None, start);
        assert_eq!(
            engine.resolve_f32(key, 1.0, Some(transition), start),
            1.0,
            "{direction:?} active start"
        );

        let completed = engine.refresh(start + Duration::from_millis(101));
        assert!(completed.changed);
        assert!(!engine.has_active_animations());

        assert_eq!(
            engine.resolve_f32(
                key,
                1.0,
                Some(transition),
                start + Duration::from_millis(200),
            ),
            0.0,
            "{direction:?} forwards fill"
        );
        assert!(!engine.has_active_animations());

        let refresh_again = engine.refresh(start + Duration::from_millis(300));
        assert!(!refresh_again.changed);
        assert!(!engine.has_active_animations());
    }
}

#[test]
fn timeline_sampling_applies_two_and_half_speed_once() {
    let duration = Duration::from_millis(100);
    let twice = sample_timeline(
        duration,
        Playback::default().speed(2.0),
        Duration::from_millis(25),
    )
    .expect("2x sample should exist");
    let half = sample_timeline(
        duration,
        Playback::default().speed(0.5),
        Duration::from_millis(50),
    )
    .expect("0.5x sample should exist");

    assert_eq!(twice.local_time, Duration::from_millis(50));
    assert_eq!(half.local_time, Duration::from_millis(25));
}

#[test]
fn controller_applies_two_and_half_speed_once() {
    fn controller_at_speed(
        speed: f32,
    ) -> (
        AnimationCoordinator,
        AnimatedValue<f32>,
        super::AnimationControllerHandle,
    ) {
        let invalidation = InvalidationSignal::new();
        let coordinator = AnimationCoordinator::default();
        let value = AnimatedValue::new(0.0f32, invalidation.clone());
        let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
            .playback(Playback::default().speed(speed))
            .track(
                value.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(100))
                        .at(Duration::ZERO, 0.0)
                        .at(Duration::from_millis(100), 100.0),
                ),
            )
            .build();
        (coordinator, value, handle)
    }

    let start = Instant::now();
    let (twice_coordinator, twice_value, twice) = controller_at_speed(2.0);
    twice.play_at(start);
    twice_coordinator.refresh(start + Duration::from_millis(25), true);
    assert_eq!(twice_value.get(), 50.0);

    let (half_coordinator, half_value, half) = controller_at_speed(0.5);
    half.play_at(start);
    half_coordinator.refresh(start + Duration::from_millis(50), true);
    assert_eq!(half_value.get(), 25.0);
}

#[test]
fn zero_speed_controller_does_not_arm_frame_clock_and_resumes_when_enabled() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .playback(Playback::default().speed(0.0))
        .track(
            value.clone(),
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 1.0),
            ),
        )
        .build();
    let start = Instant::now();

    handle.play_at(start);
    let frozen = coordinator.refresh_and_next_frame_deadline(start, true);
    assert!(!frozen.active);
    assert_eq!(frozen.next_deadline, None);
    assert_eq!(frozen.visited_controllers, 0);
    assert!(!coordinator.has_active_controllers());
    assert_eq!(value.get(), 0.0);

    handle.set_speed_at(start + Duration::from_millis(500), 1.0);
    let resumed = coordinator.refresh_and_next_frame_deadline(start, false);
    assert!(resumed.active);
    assert_eq!(resumed.visited_controllers, 1);
    coordinator.refresh(start + Duration::from_millis(550), true);
    assert_eq!(value.get(), 0.5);
}

#[test]
fn running_controller_samples_the_freeze_instant_before_zero_speed() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .track(
            value.clone(),
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 1.0),
            ),
        )
        .build();
    let start = Instant::now();

    handle.play_at(start);
    coordinator.refresh(start + Duration::from_millis(25), true);
    assert_eq!(value.get(), 0.25);

    handle.set_speed_at(start + Duration::from_millis(50), 0.0);
    assert_eq!(value.get(), 0.5);
    assert_eq!(handle.progress_at(start + Duration::from_secs(1)), 0.5);
    let frozen = coordinator.refresh_and_next_frame_deadline(start + Duration::from_secs(1), true);
    assert!(!frozen.active);
    assert_eq!(frozen.next_deadline, None);

    handle.set_speed_at(start + Duration::from_secs(1), 1.0);
    coordinator.refresh(start + Duration::from_millis(1050), true);
    assert_eq!(value.get(), 1.0);
    assert_eq!(handle.status(), AnimationStatus::Completed);
}

#[test]
fn zero_speed_declarative_transition_settles_without_frame_loop() {
    let mut engine = AnimationEngine::default();
    let animation_key = key(WidgetProperty::Opacity);
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_millis(100)).speed(0.0);

    engine.resolve_f32(animation_key, 0.0, None, start);
    assert_eq!(
        engine.resolve_f32(animation_key, 1.0, Some(transition), start),
        1.0
    );
    assert!(!engine.has_active_animations());
    let refresh = engine.refresh(start + Duration::from_secs(10));
    assert!(!refresh.changed);
    assert!(!engine.has_active_animations());
}

#[test]
fn zero_duration_infinite_controller_completes_without_frame_loop() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0_f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .playback(Playback::default().repeat_forever().speed(0.0))
        .track(
            value.clone(),
            AnimationSpec::from(Keyframes::timed(Duration::ZERO).at(Duration::ZERO, 1.0_f32)),
        )
        .build();
    let start = Instant::now();

    handle.play_at(start);
    let frame = coordinator.refresh_and_next_frame_deadline(start, true);

    assert!(frame.changed);
    assert!(!frame.active);
    assert_eq!(frame.next_deadline, None);
    assert_eq!(handle.status(), AnimationStatus::Completed);
    assert_eq!(value.get(), 1.0);
}

#[test]
fn running_controller_speed_change_preserves_progress() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .track(
            value,
            AnimationSpec::from(
                Keyframes::timed(Duration::from_secs(1))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_secs(1), 1.0),
            ),
        )
        .build();
    let start = Instant::now();
    let changed_at = start + Duration::from_millis(200);

    handle.play_at(start);
    coordinator.refresh(changed_at, true);
    assert!((handle.progress_at(changed_at) - 0.2).abs() < f32::EPSILON);

    handle.set_speed_at(changed_at, 2.0);
    assert!((handle.progress_at(changed_at) - 0.2).abs() < f32::EPSILON);
    assert!(
        (handle.progress_at(changed_at + Duration::from_millis(100)) - 0.4).abs() < f32::EPSILON
    );
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
fn stopping_controller_does_not_replay_start_callback() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let starts = Arc::new(AtomicUsize::new(0));
    let stops = Arc::new(AtomicUsize::new(0));
    let starts_callback = starts.clone();
    let stops_callback = stops.clone();
    let value = AnimatedValue::new(0.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator, invalidation)
        .track(
            value,
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 1.0),
            ),
        )
        .on_start(move || {
            starts_callback.fetch_add(1, Ordering::Relaxed);
        })
        .on_stop(move || {
            stops_callback.fetch_add(1, Ordering::Relaxed);
        })
        .build();

    handle.stop();

    assert_eq!(starts.load(Ordering::Relaxed), 0);
    assert_eq!(stops.load(Ordering::Relaxed), 1);
}

#[test]
fn completed_controller_keeps_terminal_progress() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(0.0_f32, invalidation.clone());
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
    let start = Instant::now();

    handle.play_at(start);
    coordinator.refresh(start + Duration::from_millis(101), true);

    assert_eq!(handle.status(), AnimationStatus::Completed);
    assert_eq!(handle.progress_at(start + Duration::from_millis(200)), 1.0);
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
fn fill_mode_none_stays_hidden_during_delay_then_completes_controller() {
    let invalidation = InvalidationSignal::new();
    let coordinator = AnimationCoordinator::default();
    let value = AnimatedValue::new(-1.0f32, invalidation.clone());
    let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
        .playback(
            Playback::default()
                .delay(Duration::from_millis(50))
                .fill_mode(FillMode::None),
        )
        .track(
            value.clone(),
            AnimationSpec::from(
                Keyframes::timed(Duration::from_millis(100))
                    .at(Duration::ZERO, 0.0)
                    .at(Duration::from_millis(100), 1.0),
            ),
        )
        .build();
    let start = Instant::now();

    handle.play_at(start);
    let delayed =
        coordinator.refresh_and_next_frame_deadline(start + Duration::from_millis(25), true);
    assert!(delayed.active);
    assert!(delayed.next_deadline.is_some());
    assert_eq!(handle.status(), AnimationStatus::Running);
    assert_eq!(value.get(), -1.0, "hidden delay must not write a keyframe");

    let completed =
        coordinator.refresh_and_next_frame_deadline(start + Duration::from_millis(151), true);
    assert!(!completed.active);
    assert_eq!(completed.next_deadline, None);
    assert_eq!(handle.status(), AnimationStatus::Completed);
    assert_eq!(
        value.get(),
        -1.0,
        "hidden completion restores the play baseline"
    );
    assert_eq!(handle.progress_at(start + Duration::from_millis(151)), 1.0);
}

#[test]
fn non_forward_fill_modes_prune_completed_controllers() {
    for fill_mode in [FillMode::None, FillMode::Backwards] {
        let invalidation = InvalidationSignal::new();
        let coordinator = AnimationCoordinator::default();
        let value = AnimatedValue::new(-1.0f32, invalidation.clone());
        let handle = AnimationControllerBuilder::new(coordinator.clone(), invalidation)
            .playback(Playback::default().fill_mode(fill_mode))
            .track(
                value.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(100))
                        .at(Duration::ZERO, 0.0)
                        .at(Duration::from_millis(100), 1.0),
                ),
            )
            .build();
        let start = Instant::now();

        handle.play_at(start);
        coordinator.refresh(start + Duration::from_millis(50), true);
        assert_eq!(value.get(), 0.5, "fill mode {fill_mode:?} active sample");
        let completed =
            coordinator.refresh_and_next_frame_deadline(start + Duration::from_millis(101), true);

        assert!(!completed.active, "fill mode {fill_mode:?}");
        assert_eq!(completed.next_deadline, None, "fill mode {fill_mode:?}");
        assert_eq!(
            handle.status(),
            AnimationStatus::Completed,
            "fill mode {fill_mode:?}"
        );
        assert_eq!(
            value.get(),
            -1.0,
            "fill mode {fill_mode:?} restores the play baseline"
        );
        assert_eq!(
            handle.progress_at(start + Duration::from_millis(101)),
            1.0,
            "fill mode {fill_mode:?} keeps terminal progress"
        );
        assert!(!coordinator.has_active_controllers());
    }
}

#[test]
fn non_forward_fill_modes_prune_completed_declarative_transitions() {
    for fill_mode in [FillMode::None, FillMode::Backwards] {
        let mut engine = AnimationEngine::default();
        let animation_key = key(WidgetProperty::Opacity);
        let start = Instant::now();
        let transition = Transition::linear(Duration::from_millis(100))
            .delay(Duration::from_millis(20))
            .fill_mode(fill_mode);

        assert_eq!(
            engine.resolve_f32(animation_key, 0.0, Some(transition), start),
            0.0
        );
        assert_eq!(
            engine.resolve_f32(animation_key, 1.0, Some(transition), start),
            0.0
        );
        assert!(engine.has_active_animations());

        let delayed = engine.refresh(start + Duration::from_millis(10));
        assert!(!delayed.changed);
        assert!(engine.has_active_animations());

        let completed = engine.refresh(start + Duration::from_millis(121));
        assert!(completed.changed);
        assert!(!engine.has_active_animations());
        assert_eq!(
            engine.resolve_f32(
                animation_key,
                1.0,
                Some(transition),
                start + Duration::from_millis(121),
            ),
            1.0,
            "a completed hidden-fill transition must remain settled at its target"
        );
        assert!(!engine.has_active_animations());
    }
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
fn window_color_animation_refresh_is_scene_only() {
    let mut engine = AnimationEngine::default();
    let key = AnimationKey::Window(WindowProperty::ThemeBackground);
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_millis(100));

    engine.resolve_color(key, Color::BLACK, None, start);
    engine.resolve_color(key, Color::WHITE, Some(transition), start);
    let refresh = engine.refresh(start + Duration::from_millis(50));

    assert!(refresh.changed);
    assert!(!refresh.layout_changed);
    assert!(refresh.layout_widget_ids.is_empty());
    assert!(refresh.scene_widget_ids.is_empty());
    assert_eq!(
        refresh.window_properties.as_slice(),
        &[WindowProperty::ThemeBackground]
    );
    assert!(refresh.scene_changed());
}

#[test]
fn clear_color_animation_is_renderer_only() {
    let mut engine = AnimationEngine::default();
    let key = AnimationKey::Window(WindowProperty::ClearColor);
    let start = Instant::now();
    let transition = Transition::linear(Duration::from_millis(100));

    engine.resolve_color(key, Color::BLACK, None, start);
    engine.resolve_color(key, Color::WHITE, Some(transition), start);
    let refresh = engine.refresh(start + Duration::from_millis(50));

    assert!(refresh.changed);
    assert_eq!(
        refresh.window_properties.as_slice(),
        &[WindowProperty::ClearColor]
    );
    assert!(!refresh.scene_changed());
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
