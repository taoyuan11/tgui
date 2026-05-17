use super::*;

impl<VM: ViewModel> ApplicationHandler for BoundRuntimeHandler<VM> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.create_or_resume_surface(event_loop, None);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
        self.drain_notification_completions();
        if self.drain_window_requests() {
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        if self.handle_bound_window_event(event_loop, event) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_dialog_completions();
        self.drain_notification_completions();
        if self.handle_bound_about_to_wait(event_loop) {
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        self.suspend();
    }
}
