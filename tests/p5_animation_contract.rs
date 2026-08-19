use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use tgui::animation::{
    Animated, AnimationConflictPolicy, AnimationImpact, AnimationKey, AnimationSpec,
    AnimationStatus, Timeline,
};
use tgui::application::{Application, WindowSpec};
use tgui::core::{ElementId, PropertyId, WidgetKey};
use tgui::state::State;
use tgui::test_support::{FakeClock, WidgetHarness};
use tgui::widget::{Widget, WidgetNode};
use tgui::widgets::Button;

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn key(element: ElementId, property: u64) -> AnimationKey {
    AnimationKey::new(element, PropertyId::new(property))
}

fn new_timeline() -> (Rc<FakeClock>, Timeline) {
    let clock = Rc::new(FakeClock::new());
    let timeline = Timeline::new(clock.clone());
    (clock, timeline)
}

#[test]
fn fake_clock_is_deterministic_and_invalidates_only_changed_properties() {
    let (clock, mut timeline) = new_timeline();
    let base = State::new(17.0_f32);
    let opacity = Animated::new(0.0_f32);
    let width = Animated::new(20.0_f32);
    let element = ElementId::from_parts(4, 2);
    let opacity_key = key(element, 1);
    let width_key = key(element, 2);

    timeline.animate(
        opacity_key,
        &opacity,
        1.0,
        AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
    );
    timeline.animate(
        width_key,
        &width,
        40.0,
        AnimationSpec::new(Duration::from_millis(200), AnimationImpact::Layout),
    );

    let initial = timeline.tick();
    assert!(initial.invalidations().is_empty());
    assert_eq!(initial.sampled(), 2);
    assert_eq!(initial.active(), 2);
    assert!(initial.needs_next_frame());

    clock.advance(Duration::from_millis(50)).unwrap();
    let frame = timeline.tick();
    close(opacity.value(), 0.5);
    close(width.value(), 25.0);
    assert_eq!(frame.invalidations().len(), 2);
    assert_eq!(frame.invalidations()[0].key(), opacity_key);
    assert_eq!(frame.invalidations()[0].impact(), AnimationImpact::Paint);
    assert_eq!(frame.invalidations()[1].key(), width_key);
    assert_eq!(frame.invalidations()[1].impact(), AnimationImpact::Layout);
    assert_eq!(base.get().unwrap(), 17.0);

    clock.advance(Duration::from_millis(150)).unwrap();
    let completed = timeline.tick();
    close(opacity.value(), 1.0);
    close(width.value(), 40.0);
    assert_eq!(completed.completed().len(), 2);
    assert!(!completed.needs_next_frame());
    assert_eq!(timeline.presentation::<f32>(opacity_key), Some(1.0));
    assert_eq!(timeline.presentation::<f32>(width_key), Some(40.0));

    assert!(timeline.clear_presentation(opacity_key));
    assert_eq!(timeline.presentation::<f32>(opacity_key), None);
    let cleared = timeline.tick();
    assert_eq!(cleared.invalidations().len(), 1);
    assert_eq!(cleared.invalidations()[0].key(), opacity_key);
    let idle = timeline.tick();
    assert_eq!(idle.sampled(), 0);
    assert!(idle.invalidations().is_empty());
    assert!(idle.completed().is_empty());
    assert!(idle.cancelled().is_empty());
    assert!(!idle.needs_next_frame());
}

#[test]
fn pause_resume_cancel_and_stale_handles_are_isolated() {
    let (clock, mut timeline) = new_timeline();
    let value = Animated::new(0.0_f32);
    let animation_key = key(ElementId::from_parts(1, 1), 7);
    let spec = AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint);
    let first = timeline.animate(animation_key, &value, 100.0, spec);

    clock.advance(Duration::from_millis(25)).unwrap();
    assert!(timeline.pause(&first));
    close(value.value(), 25.0);
    assert_eq!(first.status(), AnimationStatus::Paused);
    assert!(!timeline.needs_frame());

    clock.advance(Duration::from_millis(50)).unwrap();
    let paused = timeline.tick();
    assert_eq!(paused.sampled(), 0);
    close(value.value(), 25.0);

    assert!(timeline.resume(&first));
    clock.advance(Duration::from_millis(25)).unwrap();
    timeline.tick();
    close(value.value(), 50.0);

    assert!(timeline.cancel(&first));
    assert_eq!(first.status(), AnimationStatus::Cancelled);
    assert!(!timeline.cancel(&first));
    assert_eq!(timeline.presentation::<f32>(animation_key), None);

    let second = timeline.animate(animation_key, &value, 200.0, spec);
    assert_ne!(first.id(), second.id());
    assert!(!timeline.cancel(&first));
    assert!(timeline.contains(&second));

    let (_other_clock, mut other_timeline) = new_timeline();
    let other_value = Animated::new(0.0_f32);
    let other = other_timeline.animate(animation_key, &other_value, 1.0, spec);
    assert_eq!(other.id(), first.id());
    assert_ne!(other, first);
    assert!(!other_timeline.cancel(&first));
    assert!(other_timeline.contains(&other));

    let report = timeline.tick();
    assert_eq!(report.cancelled(), &[first]);
}

#[test]
fn same_key_continue_samples_the_old_track_before_retargeting() {
    let (clock, mut timeline) = new_timeline();
    let value = Animated::new(0.0_f32);
    let animation_key = key(ElementId::from_parts(2, 1), 8);
    let first = timeline.animate_between(
        animation_key,
        &value,
        0.0,
        100.0,
        AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
    );

    clock.advance(Duration::from_millis(40)).unwrap();
    let rebuilt_value = Animated::new(-999.0_f32);
    let second = timeline.animate_between(
        animation_key,
        &rebuilt_value,
        -500.0,
        200.0,
        AnimationSpec::new(Duration::from_millis(60), AnimationImpact::Paint)
            .with_conflict(AnimationConflictPolicy::Continue),
    );
    close(rebuilt_value.value(), 40.0);
    assert_eq!(first.status(), AnimationStatus::Replaced);
    assert_eq!(second.status(), AnimationStatus::Running);

    clock.advance(Duration::from_millis(30)).unwrap();
    timeline.tick();
    close(rebuilt_value.value(), 120.0);
    assert!(!timeline.cancel(&first));
    assert!(timeline.contains(&second));
}

#[test]
fn reduced_motion_completes_without_scheduling_and_callbacks_are_deferred() {
    let (clock, mut timeline) = new_timeline();
    let value = Animated::new(0.0_f32);
    let animation_key = key(ElementId::from_parts(3, 1), 9);
    let completions = Rc::new(Cell::new(0));
    let callback_count = completions.clone();
    let handle = timeline.animate_with_completion(
        animation_key,
        &value,
        1.0,
        AnimationSpec::new(Duration::from_secs(1), AnimationImpact::Paint),
        move || callback_count.set(callback_count.get() + 1),
    );

    clock.advance(Duration::from_millis(250)).unwrap();
    timeline.set_reduced_motion(true);
    assert_eq!(handle.status(), AnimationStatus::Completed);
    close(value.value(), 1.0);
    assert!(!timeline.needs_frame());
    assert_eq!(completions.get(), 0);

    let frame = timeline.tick();
    assert_eq!(frame.completed(), &[handle]);
    assert_eq!(frame.invalidations().len(), 1);
    assert!(!frame.needs_next_frame());
    assert_eq!(timeline.dispatch_completion_callbacks(), 1);
    assert_eq!(completions.get(), 1);
    assert_eq!(timeline.dispatch_completion_callbacks(), 0);

    let immediate = Animated::new(5.0_f32);
    let immediate_handle = timeline.animate_with_completion(
        key(ElementId::from_parts(3, 1), 10),
        &immediate,
        9.0,
        AnimationSpec::new(Duration::from_secs(5), AnimationImpact::Layout),
        || {},
    );
    assert_eq!(immediate_handle.status(), AnimationStatus::Completed);
    close(immediate.value(), 9.0);
    assert!(!timeline.needs_frame());
}

#[test]
fn rebuild_and_keyed_reorder_keep_identity_while_unmount_cancels_exact_generation() {
    struct Root;
    struct Item;

    fn root(order: &[&str]) -> WidgetNode {
        WidgetNode::new::<Root>().with_children(
            order
                .iter()
                .map(|key| WidgetNode::new::<Item>().with_key(*key)),
        )
    }

    let mut harness = WidgetHarness::new();
    harness.mount(root(&["a", "b", "c"])).unwrap();
    let root_id = harness.root().unwrap();
    let b = harness
        .child_for_key(root_id, &WidgetKey::from("b"))
        .unwrap();
    let (clock, mut timeline) = new_timeline();
    let value = Animated::new(0.0_f32);
    let animation_key = key(b, 11);
    let handle = timeline.animate(
        animation_key,
        &value,
        1.0,
        AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
    );

    harness.reconcile(root(&["c", "a", "b"])).unwrap();
    let reordered_b = harness
        .child_for_key(root_id, &WidgetKey::from("b"))
        .unwrap();
    assert_eq!(reordered_b, b);
    assert_eq!(timeline.retain_elements(|id| harness.contains(id)), 0);
    assert!(timeline.contains(&handle));

    harness.reconcile(root(&["c", "a"])).unwrap();
    assert!(!harness.contains(b));
    assert_eq!(timeline.retain_elements(|id| harness.contains(id)), 1);
    assert_eq!(handle.status(), AnimationStatus::Cancelled);
    assert_eq!(timeline.presentation::<f32>(animation_key), None);

    harness.reconcile(root(&["c", "a", "b"])).unwrap();
    let replacement_b = harness
        .child_for_key(root_id, &WidgetKey::from("b"))
        .unwrap();
    assert_ne!(replacement_b, b);
    if replacement_b.slot() == b.slot() {
        assert_ne!(replacement_b.generation(), b.generation());
    }

    clock.advance(Duration::from_secs(1)).unwrap();
    let frame = timeline.tick();
    assert_eq!(frame.cancelled(), &[handle]);
    assert!(frame.invalidations().is_empty());
}

#[test]
fn application_routes_animation_overlay_through_dirty_layout_and_scene() {
    let clock = Rc::new(FakeClock::new());
    let mut application = Application::with_frame_clock(clock.clone());
    let window = application
        .create_window(WindowSpec::new("animation-contract"))
        .unwrap();
    let mut context = tgui::widget::BuildContext::new();
    let button = Button::new("fade").build(&mut context).unwrap();
    let report = application.mount_widget(window, button).unwrap();
    let element = report.invalidations().next().unwrap().element();
    let initial = application.render_window(window).unwrap();
    let initial_fingerprint = initial.scene.fingerprint();
    let presentation = Animated::new(1.0_f32);
    assert!(
        application
            .animate(
                window,
                AnimationKey::new(element, PropertyId::new(1)),
                &presentation,
                0.25,
                AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
            )
            .is_err()
    );
    application
        .animate(
            window,
            AnimationKey::new(element, tgui::widget::OPACITY),
            &presentation,
            0.25,
            AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
        )
        .unwrap();
    clock.advance(Duration::from_millis(50)).unwrap();
    let tick = application.tick_animations(window).unwrap();
    assert_eq!(tick.metrics.sampled, 1);
    assert!(application.window_info(window).unwrap().frame_requested);
    let next = application.render_window(window).unwrap();
    assert_ne!(next.scene.fingerprint(), initial_fingerprint);
    assert_eq!(
        application.frame_metrics(window).unwrap().animation.sampled,
        1
    );

    let width = Animated::new(800.0_f32);
    application
        .animate_between(
            window,
            AnimationKey::new(element, tgui::widget::LAYOUT_WIDTH),
            &width,
            800.0,
            400.0,
            AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Layout),
        )
        .unwrap();
    clock.advance(Duration::from_millis(25)).unwrap();
    let layout_tick = application.tick_animations(window).unwrap();
    assert!(
        layout_tick
            .frame
            .invalidations()
            .iter()
            .any(|item| item.impact() == AnimationImpact::Layout)
    );
    let layout = application.layout_window(window).unwrap();
    assert!(layout.layout_performed);
    close(
        layout.snapshot.node(element).unwrap().rect().size.width,
        700.0,
    );
}
