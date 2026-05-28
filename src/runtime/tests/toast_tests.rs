use super::*;

use crate::foundation::binding::{Toast, ToastQueue};
use crate::ui::widget::ToastHost;

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
        Stack::new().child(ToastHost::new(self.queue.clone())).into()
    }
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
            .any(|text| text.content == "ephemeral"),
        "toast should be visible before its deadline"
    );
    assert!(
        handler.next_toast_wakeup_deadline.is_some(),
        "runtime should track the next toast wakeup deadline"
    );

    std::thread::sleep(Duration::from_millis(2200));
    let event_loop = TestEventLoop;
    assert!(
        handler.drive_animations(&event_loop, Instant::now()),
        "toast wakeup should invalidate the cached scene"
    );
    assert!(
        handler.next_toast_wakeup_deadline.is_none(),
        "expired wakeup should be consumed before the next collect"
    );

    let after = handler.computed_scene().clone();
    assert!(
        after
            .scene
            .overlay_texts
            .iter()
            .all(|text| text.content != "ephemeral"),
        "expired toast should be removed on the next collect"
    );
    assert!(
        handler.next_toast_wakeup_deadline.is_none(),
        "once the queue is empty there should be no next toast deadline"
    );
}
