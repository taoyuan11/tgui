use super::*;

use crate::foundation::binding::{Toast, ToastQueue};
use crate::ui::widget::{ComputedScene, ToastHost};

struct ToastRuntimeVm {
    queue: ToastQueue<Self>,
}

impl crate::foundation::view_model::ViewModel for ToastRuntimeVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            queue: ToastQueue::new(context),
        }
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new()
            .child(ToastHost::new(self.queue.clone()))
            .into()
    }
}

fn retained_toast_host_ids(handler: &BoundRuntimeHandler<ToastRuntimeVm>) -> Vec<WidgetId> {
    let layout = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .expect("toast test should warm the retained scene layout");
    let mut ids = layout
        .all_widget_ids()
        .filter(|widget_id| {
            layout.resolved_widget(*widget_id).is_some_and(|widget| {
                matches!(
                    widget.kind,
                    crate::ui::widget::ResolvedWidgetKind::ToastHost { .. }
                )
            })
        })
        .collect::<Vec<_>>();
    ids.sort_by_key(|widget_id| widget_id.raw());
    ids
}

fn toast_overlay_widget_ids(scene: &ComputedScene<ToastRuntimeVm>) -> Vec<WidgetId> {
    scene
        .overlay_hit_regions
        .iter()
        .filter_map(|hit| match &hit.interaction {
            HitInteraction::Widget { id, .. } => Some(id),
            _ => None,
        })
        .copied()
        .collect()
}

#[test]
fn sole_toast_tick_reuses_prepared_card_tree() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    let created_at = Instant::now() - Duration::from_secs(1);
    queue.push_at(Toast::new("prepared").persistent(true), created_at);
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);

    let before_ids = toast_overlay_widget_ids(handler.computed_scene());
    assert!(
        !before_ids.is_empty(),
        "Toast card should expose its close hit"
    );
    let tick = Instant::now();
    handler.next_toast_wakeup_deadline = Some(tick);
    crate::runtime::scene_runtime::frame_path_probe::begin();
    assert!(handler.drive_animations(&TestEventLoop, tick));
    let after_ids = toast_overlay_widget_ids(handler.computed_scene());
    let path = crate::runtime::scene_runtime::frame_path_probe::finish();

    assert_eq!(after_ids, before_ids);
    assert_eq!(path.scene_recollects, 0);
    assert_eq!(path.layout_builds, 0);
    assert!(handler
        .cached_scene
        .as_ref()
        .is_some_and(|cached| cached.layout_valid && cached.computed_valid));
}

#[test]
fn toast_prepared_card_path_rejects_coincident_widget_state_change() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    queue.push_at(
        Toast::new("fallback").persistent(true),
        Instant::now() - Duration::from_secs(1),
    );
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);

    let before_ids = toast_overlay_widget_ids(handler.computed_scene());
    handler.invalidate_computed_scene_for_toast_motion();
    handler.hover_epoch = handler.hover_epoch.wrapping_add(1);
    crate::runtime::scene_runtime::frame_path_probe::begin();
    let after_ids = toast_overlay_widget_ids(handler.computed_scene());
    let path = crate::runtime::scene_runtime::frame_path_probe::finish();

    assert_ne!(after_ids, before_ids, "fallback must rebuild the card tree");
    assert_eq!(path.scene_recollects, 1);
    assert_eq!(path.layout_reuses, 1);
    assert_eq!(path.layout_builds, 0);
}

#[test]
fn toast_prepared_card_path_rejects_coincident_signal_change() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    let message = context.state("before".to_string());
    queue.push_at(
        Toast::new(message.signal()).persistent(true),
        Instant::now() - Duration::from_secs(1),
    );
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);

    let before_ids = toast_overlay_widget_ids(handler.computed_scene());
    handler.invalidate_computed_scene_for_toast_motion();
    message.set("after".to_string());
    handler.request_redraw_if_dirty(Instant::now());
    assert!(!handler.toast_motion_patch_pending);
    let scene = handler.computed_scene();
    let after_ids = toast_overlay_widget_ids(scene);

    assert_ne!(
        after_ids, before_ids,
        "signal change must rebuild card layout"
    );
    assert!(scene
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "after"));
    assert!(scene
        .scene
        .overlay_texts
        .iter()
        .all(|text| text.content.as_ref() != "before"));
}

#[test]
fn retained_toast_push_propagates_next_frame_wakeup() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);
    let clock_origin = Instant::now();
    handler.frame_clock = crate::animation::AdaptiveFrameClock::new(clock_origin);
    let _ = handler.computed_scene();
    assert_eq!(handler.next_toast_wakeup_deadline, None);
    let host_ids = retained_toast_host_ids(&handler);
    let [host_id] = host_ids.as_slice() else {
        panic!("expected exactly one retained ToastHost");
    };

    let now = clock_origin + Duration::from_millis(5);
    queue.push_at(Toast::new("entering").persistent(true), now);
    assert!(handler.patch_cached_scene_for_roots(&[*host_id], now, false));

    assert_eq!(
        handler.next_toast_wakeup_deadline,
        Some(handler.frame_clock.snapshot().next_deadline_after(now)),
        "an entering toast collected by a retained subtree patch must wake on the next frame"
    );
}

#[test]
fn retained_settled_toast_propagates_expiry_wakeup() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);
    let created_at = Instant::now();
    handler.frame_clock = crate::animation::AdaptiveFrameClock::new(created_at);
    let _ = handler.computed_scene();
    let host_ids = retained_toast_host_ids(&handler);
    let [host_id] = host_ids.as_slice() else {
        panic!("expected exactly one retained ToastHost");
    };

    queue.push_at(
        Toast::new("settled").duration(Duration::from_secs(60)),
        created_at,
    );
    let now = created_at + Duration::from_secs(5);
    let expiry = queue.earliest_deadline().expect("non-persistent expiry");
    assert!(handler.patch_cached_scene_for_roots(&[*host_id], now, false));

    assert_eq!(
        handler.next_toast_wakeup_deadline,
        Some(expiry),
        "a settled toast must retain its exact expiry wakeup after subtree patching"
    );
}

#[test]
fn retained_multi_root_toast_patch_merges_earliest_wakeup() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let first_queue = view_model.queue.clone();
    let second_queue = ToastQueue::new(&context);
    let tree = WidgetTree::new(
        Stack::new()
            .child(ToastHost::new(first_queue.clone()))
            .child(ToastHost::new(second_queue.clone())),
    );
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);
    let created_at = Instant::now();
    handler.frame_clock = crate::animation::AdaptiveFrameClock::new(created_at);
    let _ = handler.computed_scene();
    let host_ids = retained_toast_host_ids(&handler);
    assert_eq!(host_ids.len(), 2, "expected two retained ToastHost roots");

    first_queue.push_at(
        Toast::new("later").duration(Duration::from_secs(60)),
        created_at,
    );
    second_queue.push_at(
        Toast::new("earlier").duration(Duration::from_secs(30)),
        created_at,
    );
    let now = created_at + Duration::from_secs(5);
    let expected = second_queue
        .earliest_deadline()
        .expect("second queue expiry");
    assert!(handler.patch_cached_scene_for_roots(&host_ids, now, false));

    assert_eq!(
        handler.next_toast_wakeup_deadline,
        Some(expected),
        "multi-root retained collection must merge the earliest toast side-channel deadline"
    );
}

#[test]
fn strict_toast_queue_insert_uses_retained_scene_patch() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);

    assert!(handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .all(|text| text.content.as_ref() != "strict retained toast"));

    crate::runtime::scene_runtime::frame_path_probe::begin();
    queue.push(Toast::new("strict retained toast").persistent(true));
    handler.request_redraw_if_dirty(Instant::now());
    let retained = handler.computed_scene().clone();
    let path = crate::runtime::scene_runtime::frame_path_probe::finish();

    assert!(retained
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "strict retained toast"));
    assert_eq!(path.layout_builds, 0);
    assert_eq!(path.scene_recollects, 0);
    assert!(path.cache_hits > 0);
    assert!(handler
        .cached_scene
        .as_ref()
        .is_some_and(|cached| cached.layout_valid && cached.computed_valid));

    handler.invalidate_scene_with_reason("strict_toast_full_layout_control");
    assert!(handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .any(|text| text.content.as_ref() == "strict retained toast"));
}

#[test]
fn toast_deadline_invalidates_scene_and_recollect_clears_expired_entry() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let view_model = ToastRuntimeVm::new(&context);
    let queue = view_model.queue.clone();
    queue.push(Toast::new("ephemeral").duration(Duration::from_secs(2)));
    let tree = WidgetTree::new(ToastRuntimeVm::view(&view_model));
    let mut handler = test_handler_with_vm(view_model, Some(tree), invalidation);

    let initial = handler.computed_scene().clone();
    assert!(
        initial
            .scene
            .overlay_texts
            .iter()
            .any(|text| text.content.as_ref() == "ephemeral"),
        "toast should be visible before its deadline"
    );
    assert!(
        handler.next_toast_wakeup_deadline.is_some(),
        "runtime should track the next toast wakeup deadline"
    );

    std::thread::sleep(Duration::from_millis(2200));
    let event_loop = TestEventLoop;

    // 第一次drive_animations会触发Toast进入退场状态
    handler.drive_animations(&event_loop, Instant::now());

    // 等待退场动画完成（300ms + buffer）
    std::thread::sleep(Duration::from_millis(400));

    // 第二次drive_animations应该清理完成退场的Toast
    let _did_invalidate = handler.drive_animations(&event_loop, Instant::now());

    let final_scene = handler.computed_scene().clone();
    assert!(
        final_scene
            .scene
            .overlay_texts
            .iter()
            .all(|text| text.content.as_ref() != "ephemeral"),
        "expired toast should be removed after exit animation completes"
    );
}
