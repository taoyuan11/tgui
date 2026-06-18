use crate::foundation::task::Tasks;
use crate::foundation::view_model::{Command, CommandContext, ValueCommand, ViewModel};
use crate::foundation::window_control::{WindowControl, WindowRequest};
use crate::log::{log_text_profile, text_profile_enabled, Log};
use crate::notification::Notifications;
use crate::platform::backend::event_loop::ActiveEventLoop;
use crate::ui::widget::{
    CanvasDragEvent, CanvasMouseButton, CanvasMouseEvent, CanvasWheelEvent, Point,
};
use crate::{application::WindowClosePolicy, dialog::Dialogs};
use std::time::Instant;

use super::{
    BoundRuntimeHandler, CanvasPointerContext, ClickHandler, HoverMoveHandler,
    HoverTransitionHandler, MultiWindowHandler,
};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn command_context(&self) -> CommandContext<VM> {
        let window = self.window.clone();
        CommandContext::new(
            Dialogs::from_runtime(
                self.window_key.clone(),
                self.window_instance_id,
                self.window.as_ref(),
                self.dialog_dispatcher.clone(),
            ),
            Notifications::from_runtime(
                self.window_key.clone(),
                self.window_instance_id,
                self.config.app_id.clone(),
                self.notification_dispatcher.clone(),
            ),
            Tasks::from_runtime(
                self.window_key.clone(),
                self.window_instance_id,
                self.task_dispatcher.clone(),
            ),
            WindowControl::new(self.window_requests.clone(), move || {
                window
                    .as_ref()
                    .map(|window| window.is_maximized())
                    .unwrap_or(false)
            }),
            Log::default(),
            self.rebuild_requested.clone(),
        )
    }

    pub(super) fn set_dialog_proxy(&self, event_loop: &dyn ActiveEventLoop) {
        self.dialog_dispatcher.set_proxy(event_loop.create_proxy());
        self.notification_dispatcher
            .set_proxy(event_loop.create_proxy());
        self.task_dispatcher.set_proxy(event_loop.create_proxy());
        self.invalidation.set_proxy(event_loop.create_proxy());
    }

    fn execute_command_internal(&mut self, command: &Command<VM>, invalidate_scene: bool) {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let context = self.command_context();
        let _wake_guard = if invalidate_scene {
            None
        } else {
            Some(self.invalidation.suppress_wakeups())
        };
        self.with_view_model(|view_model| command.execute_with_context(view_model, &context));
        let rebuild_requested = context.take_rebuild_request();
        let rebuilt_tree = rebuild_requested && self.rebuild_widget_tree_from_root_view();
        if invalidate_scene || rebuild_requested {
            if !rebuilt_tree {
                self.invalidate_scene_with_reason(if rebuild_requested {
                    "request_rebuild"
                } else {
                    "execute_command"
                });
            }
            self.invalidation.mark_dirty();
        }
        if let Some(started_at) = started_at {
            log_text_profile(
                "execute_command",
                started_at.elapsed(),
                format!(
                    "invalidated_scene={invalidate_scene} rebuild_requested={rebuild_requested}"
                ),
            );
        }
    }

    fn execute_value_command_internal<V>(
        &mut self,
        command: &ValueCommand<VM, V>,
        value: V,
        invalidate_scene: bool,
    ) {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let context = self.command_context();
        let _wake_guard = if invalidate_scene {
            None
        } else {
            Some(self.invalidation.suppress_wakeups())
        };
        self.with_view_model(|view_model| {
            command.execute_with_context(view_model, value, &context)
        });
        let rebuild_requested = context.take_rebuild_request();
        let rebuilt_tree = rebuild_requested && self.rebuild_widget_tree_from_root_view();
        if invalidate_scene || rebuild_requested {
            if !rebuilt_tree {
                self.invalidate_scene_with_reason(if rebuild_requested {
                    "request_rebuild"
                } else {
                    "execute_value_command"
                });
            }
            self.invalidation.mark_dirty();
        }
        if let Some(started_at) = started_at {
            log_text_profile(
                "execute_value_command",
                started_at.elapsed(),
                format!(
                    "invalidated_scene={invalidate_scene} rebuild_requested={rebuild_requested}"
                ),
            );
        }
    }

    pub(super) fn execute_command(&mut self, command: &Command<VM>) {
        self.execute_command_internal(command, true);
    }

    pub(super) fn execute_command_without_invalidation(&mut self, command: &Command<VM>) {
        self.execute_command_internal(command, false);
    }

    pub(super) fn execute_value_command<V>(&mut self, command: &ValueCommand<VM, V>, value: V) {
        self.execute_value_command_internal(command, value, true);
    }

    pub(super) fn execute_value_command_without_invalidation<V>(
        &mut self,
        command: &ValueCommand<VM, V>,
        value: V,
    ) {
        self.execute_value_command_internal(command, value, false);
    }

    pub(super) fn drain_window_requests(&mut self) -> bool {
        let requests = self.window_requests.drain();
        if requests.is_empty() {
            return false;
        }

        let mut close_requested = false;
        for request in requests {
            match request {
                WindowRequest::Drag => {
                    if let Some(window) = self.window.as_ref() {
                        if let Err(error) = window.drag_window() {
                            Log::with_tag("tgui-runtime")
                                .warn(format_args!("window drag request failed: {error}"));
                        }
                    }
                }
                WindowRequest::DragResize(direction) => {
                    if let Some(window) = self.window.as_ref() {
                        if let Err(error) = window.drag_resize_window(direction.into()) {
                            Log::with_tag("tgui-runtime")
                                .warn(format_args!("window resize request failed: {error}"));
                        }
                    }
                }
                WindowRequest::Minimize => {
                    if let Some(window) = self.window.as_ref() {
                        window.set_minimized(true);
                    }
                }
                WindowRequest::Maximize => {
                    if let Some(window) = self.window.as_ref() {
                        window.set_maximized(true);
                    }
                }
                WindowRequest::Restore => {
                    if let Some(window) = self.window.as_ref() {
                        window.set_maximized(false);
                    }
                }
                WindowRequest::ToggleMaximize => {
                    if let Some(window) = self.window.as_ref() {
                        window.set_maximized(!window.is_maximized());
                    }
                }
                WindowRequest::Close => {
                    close_requested |= self.close_policy() == WindowClosePolicy::Close;
                }
            }
        }

        close_requested
    }

    pub(super) fn execute_hover_transition_handler(
        &mut self,
        handler: &HoverTransitionHandler<VM>,
        position: Option<Point>,
    ) {
        match handler {
            HoverTransitionHandler::Command(command) => self.execute_command(command),
            HoverTransitionHandler::Canvas(command, context) => {
                if let Some(position) = position {
                    self.execute_value_command(command, context.pointer_event(position));
                }
            }
        }
    }

    pub(super) fn execute_hover_move_handler(
        &mut self,
        handler: &HoverMoveHandler<VM>,
        position: Point,
    ) {
        match handler {
            HoverMoveHandler::Point(command) => self.execute_value_command(command, position),
            HoverMoveHandler::Canvas(command, context) => {
                self.execute_value_command(command, context.pointer_event(position));
            }
        }
    }

    pub(super) fn execute_click_handler(
        &mut self,
        handler: &ClickHandler<VM>,
        position: Option<Point>,
    ) {
        match handler {
            ClickHandler::Command(command) => self.execute_command(command),
            ClickHandler::Toggle(command, next) => self.execute_value_command(command, *next),
            ClickHandler::SelectOption {
                widget_id,
                command,
                on_open_change,
            } => {
                if let Some(command) = command {
                    self.execute_command(command);
                }
                if self.close_context_menu(*widget_id) {
                    return;
                }
                let is_menu = self
                    .cached_scene
                    .as_ref()
                    .and_then(|cached| cached.layout.as_ref())
                    .and_then(|layout| layout.resolved_widget(*widget_id))
                    .and_then(|resolved| resolved.menu.as_ref())
                    .is_some();
                if is_menu {
                    let _ = self.set_menu_open_state(*widget_id, false);
                } else {
                    let _ = self.set_select_open_state(*widget_id, false, on_open_change.as_ref());
                }
            }
            ClickHandler::Canvas(command, context, button) => {
                if let Some(position) = position {
                    self.execute_value_command(command, context.mouse_event(position, *button));
                }
            }
        }
    }

    pub(super) fn execute_canvas_mouse_command(
        &mut self,
        command: &ValueCommand<VM, CanvasMouseEvent>,
        context: CanvasPointerContext,
        position: Point,
        button: Option<CanvasMouseButton>,
    ) {
        self.execute_value_command(command, context.mouse_event(position, button));
    }

    pub(super) fn execute_canvas_wheel_command(
        &mut self,
        command: &ValueCommand<VM, CanvasWheelEvent>,
        context: CanvasPointerContext,
        position: Point,
        delta: Point,
    ) {
        self.execute_value_command(command, context.wheel_event(position, delta));
    }

    pub(super) fn execute_canvas_drag_command(
        &mut self,
        command: &ValueCommand<VM, CanvasDragEvent>,
        context: CanvasPointerContext,
        start_position: Point,
        position: Point,
        button: CanvasMouseButton,
    ) {
        self.execute_value_command(
            command,
            context.drag_event(start_position, position, button),
        );
    }

    pub(super) fn drain_dialog_completions(&mut self) -> bool {
        let completions: Vec<_> = self
            .dialog_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();

        if completions.is_empty() {
            return false;
        }

        for completion in completions {
            if completion.window_instance_id != self.window_instance_id {
                continue;
            }
            let context = self.command_context();
            self.with_view_model(|view_model| (completion.callback)(view_model, &context));
            self.invalidate_scene_with_reason("dialog_completion");
            self.invalidation.mark_dirty();
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }

        true
    }

    pub(super) fn drain_notification_completions(&mut self) -> bool {
        let completions: Vec<_> = self
            .notification_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();

        if completions.is_empty() {
            return false;
        }

        for completion in completions {
            if completion.window_instance_id != self.window_instance_id {
                continue;
            }
            let context = self.command_context();
            self.with_view_model(|view_model| (completion.callback)(view_model, &context));
            self.invalidate_scene_with_reason("notification_completion");
            self.invalidation.mark_dirty();
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }

        true
    }

    pub(super) fn drain_task_completions(&mut self) -> bool {
        let completions: Vec<_> = self
            .task_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();

        if completions.is_empty() {
            return false;
        }

        for completion in completions {
            if completion.window_instance_id != self.window_instance_id {
                continue;
            }
            let context = self.command_context();
            self.with_view_model(|view_model| (completion.callback)(view_model, &context));
            self.invalidate_scene_with_reason("task_completion");
            self.invalidation.mark_dirty();
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }

        true
    }
}

impl<VM: ViewModel> MultiWindowHandler<VM> {
    pub(super) fn set_dialog_proxy(&self, event_loop: &dyn ActiveEventLoop) {
        self.dialog_dispatcher.set_proxy(event_loop.create_proxy());
        self.notification_dispatcher
            .set_proxy(event_loop.create_proxy());
        self.task_dispatcher.set_proxy(event_loop.create_proxy());
        self.invalidation.set_proxy(event_loop.create_proxy());
    }

    pub(super) fn drain_dialog_completions(&mut self) {
        let completions: Vec<_> = self.dialog_receiver.try_iter().collect();
        for completion in completions {
            let Some(window) = self.windows_by_key.get_mut(&completion.window_key) else {
                continue;
            };
            if completion.window_instance_id != window.window_instance_id {
                continue;
            }

            let context = window.command_context();
            window.with_view_model(|view_model| (completion.callback)(view_model, &context));
            window.invalidate_scene_with_reason("multi_dialog_completion");
            self.invalidation.mark_dirty();
            if let Some(native_window) = window.window.as_ref() {
                native_window.request_redraw();
            }
        }
    }

    pub(super) fn drain_notification_completions(&mut self) {
        let completions: Vec<_> = self.notification_receiver.try_iter().collect();
        for completion in completions {
            let Some(window) = self.windows_by_key.get_mut(&completion.window_key) else {
                continue;
            };
            if completion.window_instance_id != window.window_instance_id {
                continue;
            }

            let context = window.command_context();
            window.with_view_model(|view_model| (completion.callback)(view_model, &context));
            window.invalidate_scene_with_reason("multi_notification_completion");
            self.invalidation.mark_dirty();
            if let Some(native_window) = window.window.as_ref() {
                native_window.request_redraw();
            }
        }
    }

    pub(super) fn drain_task_completions(&mut self) {
        let completions: Vec<_> = self.task_receiver.try_iter().collect();
        for completion in completions {
            let Some(window) = self.windows_by_key.get_mut(&completion.window_key) else {
                continue;
            };
            if completion.window_instance_id != window.window_instance_id {
                continue;
            }

            let context = window.command_context();
            window.with_view_model(|view_model| (completion.callback)(view_model, &context));
            window.invalidate_scene_with_reason("multi_task_completion");
            self.invalidation.mark_dirty();
            if let Some(native_window) = window.window.as_ref() {
                native_window.request_redraw();
            }
        }
    }
}
