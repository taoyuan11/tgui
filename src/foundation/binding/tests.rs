use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crate::animation::{AnimationCoordinator, Transition};

use super::{
    track_dependency_scope, with_dependency_collection, DependencyOwner, DependencyPhase,
    DirtyDependencySet, InvalidationSignal, Signal, State, Toast, ToastQueue, ViewModelContext,
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
