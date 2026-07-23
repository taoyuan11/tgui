use super::*;

impl<VM: ViewModel> ApplicationHandler for BoundRuntimeHandler<VM> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.create_or_resume_surface(event_loop, None);
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, _event: ()) {
        self.invalidation.acknowledge_wake();
        self.drain_dialog_completions();
        self.drain_notification_completions();
        self.drain_task_completions();
        if self.invalidation.take_redraw_request() {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        if self.drain_window_requests() {
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        // `Occluded` is intentionally kept out of the public platform event enum, but it
        // controls whether the internal idle redraw heartbeat should remain armed.
        if let winit::event::WindowEvent::Occluded(occluded) = &event {
            self.set_window_occluded(*occluded);
        }

        if matches!(
            &event,
            winit::event::WindowEvent::Moved(_)
                | winit::event::WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.refresh_frame_clock_from_monitor(Instant::now(), true);
        }

        for event in WindowEvent::from_winit(event, self.physical_cursor_position()) {
            if self.handle_bound_window_event(event_loop, event) {
                event_loop.exit();
                break;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.drain_dialog_completions();
        self.drain_notification_completions();
        self.drain_task_completions();
        if self.handle_bound_about_to_wait(event_loop) {
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.suspend();
    }
}
