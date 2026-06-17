use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crate::animation::{AnimationCoordinator, Transition};

use super::{
    track_dependency_scope, track_property_scope, with_dependency_collection, DependencyOwner,
    DependencyPhase, DirtyDependencySet, InvalidationSignal, PropertySlot, ReactiveTarget, Signal,
    State, TextController, Toast, ToastQueue, ViewModelContext,
};

fn context() -> ViewModelContext {
    ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
}

#[test]
fn state_set_same_value_does_not_advance_revision() {
    let invalidation = InvalidationSignal::new();
    let state = State::new(1, invalidation.clone());
    let before = invalidation.revision();

    state.set(1);

    assert_eq!(invalidation.revision(), before);
}

#[test]
fn state_update_only_invalidates_when_value_changes() {
    let invalidation = InvalidationSignal::new();
    let state = State::new(String::from("hello"), invalidation.clone());
    let before = invalidation.revision();

    state.update(|value| value.push_str(""));
    assert_eq!(invalidation.revision(), before);

    state.update(|value| value.push('!'));
    assert!(invalidation.revision() > before);
}

#[test]
fn state_mutate_invalidates_without_cloning_value() {
    struct CloneTracked {
        value: usize,
        clone_count: Arc<AtomicUsize>,
    }

    impl PartialEq for CloneTracked {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }

    impl Clone for CloneTracked {
        fn clone(&self) -> Self {
            self.clone_count.fetch_add(1, Ordering::SeqCst);
            Self {
                value: self.value,
                clone_count: self.clone_count.clone(),
            }
        }
    }

    let invalidation = InvalidationSignal::new();
    let clone_count = Arc::new(AtomicUsize::new(0));
    let state = State::new(
        CloneTracked {
            value: 1,
            clone_count: clone_count.clone(),
        },
        invalidation.clone(),
    );
    let before = invalidation.revision();

    state.mutate(|value| value.value += 1);

    assert_eq!(clone_count.load(Ordering::SeqCst), 0);
    assert!(invalidation.revision() > before);
    assert_eq!(state.read(|value| value.value), 2);
}

#[test]
fn signal_get_caches_within_revision() {
    let ctx = context();
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_signal = calls.clone();
    let signal = ctx.signal(move || {
        calls_for_signal.fetch_add(1, Ordering::SeqCst);
        42
    });

    assert_eq!(signal.get(), 42);
    assert_eq!(signal.get(), 42);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn signal_recomputes_after_state_changes() {
    let ctx = context();
    let state = ctx.state(1);
    let signal = state.signal().map(|value| value * 2);

    assert_eq!(signal.get(), 2);
    state.set(4);
    assert_eq!(signal.get(), 8);
}

#[test]
fn state_set_queues_subscribed_reactive_target() {
    let ctx = context();
    let state = ctx.state(1);
    let signal = state.signal();
    let target = ReactiveTarget::Custom(7);
    signal.subscribe_target(target);

    state.set(2);

    let targets = ctx.invalidation().drain_reactive_targets();
    assert_eq!(targets, vec![target]);
}

#[test]
fn tracked_signal_read_subscribes_current_owner_as_reactive_target() {
    let ctx = context();
    let state = ctx.state(1);
    let signal = state.signal();
    let owner = DependencyOwner {
        widget_id: 42,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Background),
    };

    let _ = with_dependency_collection(|| track_dependency_scope(owner, || signal.get()));
    state.set(2);

    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(targets.contains(&ReactiveTarget::Owner(owner)));
}

#[test]
fn replacing_owner_subscription_drops_previous_signal_sources() {
    let ctx = context();
    let flag = ctx.state(true);
    let first = ctx.state(1);
    let second = ctx.state(10);
    let flag_signal = flag.signal();
    let first_signal = first.signal();
    let second_signal = second.signal();
    let owner = DependencyOwner {
        widget_id: 91,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Background),
    };
    let target = ReactiveTarget::Owner(owner);

    ctx.invalidation().replace_reactive_target(target, || {
        let _ = with_dependency_collection(|| {
            track_dependency_scope(owner, || {
                if flag_signal.get() {
                    first_signal.get()
                } else {
                    second_signal.get()
                }
            })
        });
    });
    assert_eq!(
        ctx.invalidation().reactive_target_source_count(target),
        2,
        "owner should subscribe to flag + first branch"
    );

    flag.set(false);
    let targets = ctx.invalidation().drain_reactive_targets();
    assert_eq!(targets, vec![target]);

    ctx.invalidation().replace_reactive_target(target, || {
        let _ = with_dependency_collection(|| {
            track_dependency_scope(owner, || {
                if flag_signal.get() {
                    first_signal.get()
                } else {
                    second_signal.get()
                }
            })
        });
    });
    assert_eq!(
        ctx.invalidation().reactive_target_source_count(target),
        2,
        "owner should subscribe to flag + second branch after replacement"
    );

    first.set(2);
    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(
        targets.is_empty(),
        "old branch signal must not dirty the replaced owner"
    );

    second.set(11);
    let targets = ctx.invalidation().drain_reactive_targets();
    assert_eq!(targets, vec![target]);
}

#[test]
fn text_controller_set_queues_subscribed_reactive_target() {
    let ctx = context();
    let controller = TextController::new("hello", ctx.invalidation().clone());
    let owner = DependencyOwner {
        widget_id: 7,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::TextContent),
    };

    let _ = with_dependency_collection(|| track_dependency_scope(owner, || controller.text()));
    controller.set_text("world");

    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(targets.contains(&ReactiveTarget::Owner(owner)));
}

#[test]
fn memo_signal_stops_propagation_when_value_is_unchanged() {
    let ctx = context();
    let state = ctx.state(1);
    let parity = state.signal().map_memo(|value| value % 2);
    let target = ReactiveTarget::Custom(11);
    parity.subscribe_target(target);
    assert_eq!(parity.get(), 1);

    state.set(3);

    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(
        targets.is_empty(),
        "unchanged memo value must not dirty target"
    );
}

#[test]
fn mapped_signal_stops_propagation_when_value_is_unchanged() {
    let ctx = context();
    let state = ctx.state(1);
    let parity = state.signal().map(|value| value % 2);
    let target = ReactiveTarget::Custom(13);
    parity.subscribe_target(target);
    assert_eq!(parity.get(), 1);

    state.set(3);

    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(
        targets.is_empty(),
        "unchanged map value must not dirty target"
    );
}

#[test]
fn projected_signal_stops_propagation_when_value_is_unchanged() {
    let ctx = context();
    let state = ctx.state(String::from("aa"));
    let len = state.signal().project(|value| value.len());
    let target = ReactiveTarget::Custom(14);
    len.subscribe_target(target);
    assert_eq!(len.get(), 2);

    state.set(String::from("bb"));

    let targets = ctx.invalidation().drain_reactive_targets();
    assert!(
        targets.is_empty(),
        "unchanged project value must not dirty target"
    );
}

#[test]
fn memo_signal_propagates_when_value_changes() {
    let ctx = context();
    let state = ctx.state(1);
    let parity = state.signal().map_memo(|value| value % 2);
    let target = ReactiveTarget::Custom(12);
    parity.subscribe_target(target);
    assert_eq!(parity.get(), 1);

    state.set(2);

    let targets = ctx.invalidation().drain_reactive_targets();
    assert_eq!(targets, vec![target]);
}

#[test]
fn mapped_signal_preserves_transition() {
    let ctx = context();
    let state = ctx.state(1);
    let transition = Transition::linear(Duration::from_millis(10));
    let signal = state.signal().animated(transition).map(|value| value + 1);

    assert_eq!(signal.get(), 2);
    assert_eq!(signal.transition(), Some(transition));
}

#[test]
fn state_change_records_specific_dirty_dependency() {
    let invalidation = InvalidationSignal::new();
    let state = State::new(1, invalidation.clone());
    let baseline = invalidation.revision();

    state.set(2);

    let (dirty, deps) = invalidation.dirty_dependencies_since(baseline);
    assert!(matches!(dirty, DirtyDependencySet::Dependencies { .. }));
    assert_eq!(deps.len(), 1);
}

#[test]
fn unchanged_state_keeps_dirty_dependencies_clean() {
    let invalidation = InvalidationSignal::new();
    let state = State::new(5, invalidation.clone());
    let baseline = invalidation.revision();

    state.set(5);

    let (dirty, deps) = invalidation.dirty_dependencies_since(baseline);
    assert!(matches!(dirty, DirtyDependencySet::Clean));
    assert!(deps.is_empty());
}

#[test]
fn mapped_signal_reads_are_tracked_without_global_fallback() {
    let ctx = context();
    let state = ctx.state(7);
    let mapped = state.signal().map(|value| value + 1);
    let owner = DependencyOwner {
        widget_id: 1,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| track_dependency_scope(owner, || mapped.get()));

    assert!(!graph.has_global_dependency());
    assert_eq!(graph.dependency_count(), 1);
}

#[test]
fn mapped_signal_does_not_recompute_for_unrelated_state_changes() {
    let ctx = context();
    let tracked = ctx.state(7);
    let unrelated = ctx.state(3);
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_signal = calls.clone();
    let mapped = tracked.signal().map(move |value| {
        calls_for_signal.fetch_add(1, Ordering::SeqCst);
        value + 1
    });

    assert_eq!(mapped.get(), 8);
    assert_eq!(mapped.get(), 8);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    unrelated.set(4);

    assert_eq!(mapped.get(), 8);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn opaque_signal_reads_fall_back_to_global_dependency() {
    let invalidation = InvalidationSignal::new();
    let signal = Signal::new(|| 9, invalidation);
    let owner = DependencyOwner {
        widget_id: 2,
        phase: DependencyPhase::Layout,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| track_dependency_scope(owner, || signal.get()));

    assert!(graph.has_global_dependency());
}

#[test]
fn state_project_reads_without_cloning_source_value() {
    struct CloneTrackedText {
        text: String,
        clone_count: Arc<AtomicUsize>,
    }

    impl PartialEq for CloneTrackedText {
        fn eq(&self, other: &Self) -> bool {
            self.text == other.text
        }
    }

    impl Clone for CloneTrackedText {
        fn clone(&self) -> Self {
            self.clone_count.fetch_add(1, Ordering::SeqCst);
            Self {
                text: self.text.clone(),
                clone_count: self.clone_count.clone(),
            }
        }
    }

    let ctx = context();
    let clone_count = Arc::new(AtomicUsize::new(0));
    let state = ctx.state(CloneTrackedText {
        text: "tracked text".to_string(),
        clone_count: clone_count.clone(),
    });
    let projected = state.project(|value| value.text.len());

    assert_eq!(projected.get(), "tracked text".len());
    assert_eq!(clone_count.load(Ordering::SeqCst), 0);
}

#[test]
fn toast_queue_push_dismiss_and_clear_work() {
    let queue = ToastQueue::<()>::new_detached();
    let now = Instant::now();
    let first = queue.push_at(Toast::new("first"), now - Duration::from_secs(1));
    let second = queue.push_at(Toast::new("second"), now - Duration::from_secs(1));

    assert_eq!(queue.snapshot().len(), 2);
    assert!(queue.dismiss_at(first, now));
    let entries = queue.snapshot();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.id == first)
            .and_then(|entry| entry.deadline),
        Some(now),
        "dismiss should mark a toast for exit animation before removal"
    );
    assert!(
        !queue.pause_at(first, now + Duration::from_millis(50)),
        "hover pause should not hold a toast that is already exiting"
    );

    assert!(!queue.flush_expired(now + Duration::from_millis(299)));
    assert_eq!(queue.snapshot().len(), 2);
    assert!(queue.flush_expired(now + Duration::from_millis(301)));
    let entries = queue.snapshot();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, second);

    let clear_at = now + Duration::from_secs(1);
    queue.clear_at(clear_at);
    assert_eq!(
        queue.snapshot()[0].deadline,
        Some(clear_at),
        "clear should mark remaining toasts for exit animation before removal"
    );
    assert!(!queue.snapshot().is_empty());
    assert!(queue.flush_expired(clear_at + Duration::from_millis(301)));
    assert!(queue.snapshot().is_empty());
}

#[test]
fn toast_queue_flush_expired_filters_deadlines() {
    let queue = ToastQueue::<()>::new_detached();
    let now = Instant::now();
    queue.push_at(Toast::new("short").duration(Duration::from_secs(1)), now);
    queue.push_at(Toast::new("long").duration(Duration::from_secs(5)), now);
    queue.push_at(Toast::new("keep").persistent(true), now);

    assert!(queue.flush_expired(now + Duration::from_secs(2)));
    let entries = queue.snapshot();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| entry.toast.persistent));
    assert!(entries
        .iter()
        .any(|entry| entry.toast.duration == Duration::from_secs(5)));
}

#[test]
fn toast_queue_pause_and_resume_preserve_remaining_time() {
    let queue = ToastQueue::<()>::new_detached();
    let now = Instant::now();
    let id = queue.push_at(Toast::new("pause").duration(Duration::from_secs(5)), now);

    assert!(queue.pause_at(id, now + Duration::from_secs(2)));
    let paused = queue.snapshot().pop().expect("toast should exist");
    assert!(paused.paused);
    assert_eq!(paused.paused_remaining, Some(Duration::from_secs(3)));

    assert!(queue.resume_at(id, now + Duration::from_secs(4)));
    let resumed = queue.snapshot().pop().expect("toast should exist");
    assert!(!resumed.paused);
    assert_eq!(resumed.deadline, Some(now + Duration::from_secs(7)));
}

// 属性级依赖归因

#[test]
fn property_scope_attributes_signal_read_to_slot() {
    let ctx = context();
    let fill = ctx.state(7);
    let signal = fill.signal();
    let owner = DependencyOwner {
        widget_id: 11,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            track_property_scope(PropertySlot::Background, || signal.get())
        })
    });

    // 归因开启时，该信号读取被记到 Background 属性槽，而非裸 Scene owner。
    let owners = graph.all_owners();
    assert!(
        owners.contains(&DependencyOwner {
            widget_id: 11,
            phase: DependencyPhase::Scene,
            property: Some(PropertySlot::Background),
        }),
        "signal read inside Background scope should be attributed to Background slot: {owners:?}"
    );
    assert!(
        !owners.contains(&owner),
        "no bare (property: None) Scene owner should remain for the attributed read"
    );
}

#[test]
fn property_scope_restores_outer_owner_after_drop() {
    let ctx = context();
    let inside = ctx.state(1);
    let outside = ctx.state(2);
    let inside_signal = inside.signal();
    let outside_signal = outside.signal();
    let owner = DependencyOwner {
        widget_id: 22,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            track_property_scope(PropertySlot::Opacity, || inside_signal.get());
            // 退出属性作用域后，外层 owner 应恢复，读取归因回 property: None。
            outside_signal.get()
        })
    });

    let owners = graph.all_owners();
    assert!(owners.contains(&DependencyOwner {
        widget_id: 22,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Opacity),
    }));
    assert!(
        owners.contains(&owner),
        "read after the property scope drops should fall back to the bare Scene owner: {owners:?}"
    );
}

#[test]
fn property_scope_without_outer_scope_records_nothing() {
    let ctx = context();
    let value = ctx.state(1);
    let signal = value.signal();

    // 无外层 owner 时（栈空），属性作用域不引入任何 owner，与 record_dependency_read
    // 的「无作用域即不记录」语义一致。
    let (_, graph) =
        with_dependency_collection(|| track_property_scope(PropertySlot::Scale, || signal.get()));

    assert!(graph.all_owners().is_empty());
    assert!(!graph.has_global_dependency());
}

// 补充测试：属性级依赖归因的边界情况和复杂场景

#[test]
fn property_scope_nested_same_property_uses_inner() {
    // 嵌套相同属性作用域时，内层作用域生效
    let ctx = context();
    let state = ctx.state(42);
    let signal = state.signal();
    let owner = DependencyOwner {
        widget_id: 100,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            track_property_scope(PropertySlot::Background, || {
                track_property_scope(PropertySlot::Background, || signal.get())
            })
        })
    });

    let owners = graph.all_owners();
    assert_eq!(
        owners.len(),
        1,
        "nested same property should result in single owner"
    );
    assert!(owners.contains(&DependencyOwner {
        widget_id: 100,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Background),
    }));
}

#[test]
fn property_scope_nested_different_properties() {
    // 嵌套不同属性作用域时，每层读取归因到各自的属性
    let ctx = context();
    let outer_state = ctx.state(1);
    let inner_state = ctx.state(2);
    let outer_signal = outer_state.signal();
    let inner_signal = inner_state.signal();
    let owner = DependencyOwner {
        widget_id: 200,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            let _outer = outer_signal.get();
            track_property_scope(PropertySlot::BorderColor, || {
                let _inner = inner_signal.get();
            });
        })
    });

    let owners = graph.all_owners();
    // 外层读取归因到 bare owner (property: None)
    // 内层读取归因到 BorderColor
    assert!(
        owners.contains(&owner),
        "outer read should use bare owner: {owners:?}"
    );
    assert!(
        owners.contains(&DependencyOwner {
            widget_id: 200,
            phase: DependencyPhase::Scene,
            property: Some(PropertySlot::BorderColor),
        }),
        "inner read should use BorderColor: {owners:?}"
    );
}

#[test]
fn property_scope_multiple_reads_same_slot() {
    // 同一属性作用域内多次读取不同信号，都归因到同一属性槽
    let ctx = context();
    let state1 = ctx.state(10);
    let state2 = ctx.state(20);
    let state3 = ctx.state(30);
    let signal1 = state1.signal();
    let signal2 = state2.signal();
    let signal3 = state3.signal();
    let owner = DependencyOwner {
        widget_id: 300,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            track_property_scope(PropertySlot::Opacity, || {
                let _a = signal1.get();
                let _b = signal2.get();
                let _c = signal3.get();
            })
        })
    });

    let owners = graph.all_owners();
    // 所有读取都归因到同一 Opacity 槽
    assert_eq!(
        owners.len(),
        1,
        "all reads should share the same property owner"
    );
    assert!(owners.contains(&DependencyOwner {
        widget_id: 300,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Opacity),
    }));
}

#[test]
fn property_scope_different_phases() {
    // 不同阶段的属性作用域独立工作
    let ctx = context();
    let state = ctx.state(99);
    let signal = state.signal();

    let (_, graph) = with_dependency_collection(|| {
        // Layout 阶段读取
        track_dependency_scope(
            DependencyOwner {
                widget_id: 400,
                phase: DependencyPhase::Layout,
                property: None,
            },
            || track_property_scope(PropertySlot::BorderWidth, || signal.get()),
        );
        // Scene 阶段读取
        track_dependency_scope(
            DependencyOwner {
                widget_id: 400,
                phase: DependencyPhase::Scene,
                property: None,
            },
            || track_property_scope(PropertySlot::Background, || signal.get()),
        );
    });

    let owners = graph.all_owners();
    assert_eq!(
        owners.len(),
        2,
        "different phases should create separate owners"
    );
    assert!(owners.contains(&DependencyOwner {
        widget_id: 400,
        phase: DependencyPhase::Layout,
        property: Some(PropertySlot::BorderWidth),
    }));
    assert!(owners.contains(&DependencyOwner {
        widget_id: 400,
        phase: DependencyPhase::Scene,
        property: Some(PropertySlot::Background),
    }));
}

#[test]
fn property_scope_mixed_with_and_without() {
    // 混合使用属性作用域和无作用域的读取
    let ctx = context();
    let state1 = ctx.state(1);
    let state2 = ctx.state(2);
    let signal1 = state1.signal();
    let signal2 = state2.signal();
    let owner = DependencyOwner {
        widget_id: 500,
        phase: DependencyPhase::Scene,
        property: None,
    };

    let (_, graph) = with_dependency_collection(|| {
        track_dependency_scope(owner, || {
            let _before = signal1.get();
            track_property_scope(PropertySlot::Scale, || {
                let _inside = signal2.get();
            });
        })
    });

    let owners = graph.all_owners();
    assert!(
        owners.contains(&owner),
        "read outside property scope uses bare owner"
    );
    assert!(
        owners.contains(&DependencyOwner {
            widget_id: 500,
            phase: DependencyPhase::Scene,
            property: Some(PropertySlot::Scale),
        }),
        "read inside property scope uses Scale slot"
    );
}
