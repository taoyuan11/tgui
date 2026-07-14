use super::{
    window_sync_priority, AnimationCoordinator, ApplicationConfig, BoundRuntimeHandler,
    InvalidationSignal, Log, RootViewFactory, TguiError, WindowBindings, WindowClosePolicy,
    WindowCommand, WindowRole, WindowSetFactory,
};
use crate::dialog::{async_dialog_channel, AsyncDialogDispatcher, AsyncDialogReceiver};
use crate::foundation::task::{async_task_channel, AsyncTaskDispatcher, AsyncTaskReceiver};
use crate::foundation::view_model::ViewModel;
use crate::notification::{
    async_notification_channel, AsyncNotificationDispatcher, AsyncNotificationReceiver,
};
use crate::platform::backend::application::ApplicationHandler;
use crate::platform::backend::event_loop::ActiveEventLoop;
use crate::platform::backend::window::Window;
use crate::platform::event::WindowEvent;
use crate::platform::window::WindowId;
use crate::runtime::portal::PortalRegistry;
use crate::ui::widget::WidgetTree;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;

pub(super) struct ResolvedWindowSpec<VM> {
    key: String,
    role: WindowRole,
    config: ApplicationConfig,
    window_bindings: WindowBindings,
    widget_tree: Option<WidgetTree<VM>>,
    root_view: Option<RootViewFactory<VM>>,
    commands: Vec<WindowCommand<VM>>,
    close_policy: WindowClosePolicy,
}

pub(super) struct MultiWindowHandler<VM> {
    config: ApplicationConfig,
    view_model: Arc<Mutex<VM>>,
    windows: WindowSetFactory<VM>,
    pub(super) invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
    pub(super) dialog_dispatcher: AsyncDialogDispatcher<VM>,
    pub(super) dialog_receiver: AsyncDialogReceiver<VM>,
    pub(super) notification_dispatcher: AsyncNotificationDispatcher<VM>,
    pub(super) notification_receiver: AsyncNotificationReceiver<VM>,
    pub(super) task_dispatcher: AsyncTaskDispatcher<VM>,
    pub(super) task_receiver: AsyncTaskReceiver<VM>,
    next_window_instance_id: u64,
    pub(super) windows_by_key: HashMap<String, BoundRuntimeHandler<VM>>,
    window_keys_by_id: HashMap<WindowId, String>,
    closed_window_keys: HashSet<String>,
    last_window_sync_revision: u64,
    windows_need_sync: bool,
    portal_registry: PortalRegistry<VM>,
    shutting_down: bool,
    #[cfg(target_os = "windows")]
    main_window_disabled_for_modal: bool,
    pub(super) error: Option<TguiError>,
}

impl<VM: ViewModel> MultiWindowHandler<VM> {
    pub(super) fn new(
        config: ApplicationConfig,
        view_model: Arc<Mutex<VM>>,
        windows: WindowSetFactory<VM>,
        invalidation: InvalidationSignal,
        animations: AnimationCoordinator,
    ) -> Self {
        let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
        let (notification_dispatcher, notification_receiver) = async_notification_channel();
        let (task_dispatcher, task_receiver) = async_task_channel();
        Self {
            config,
            view_model,
            windows,
            invalidation,
            animations,
            dialog_dispatcher,
            dialog_receiver,
            notification_dispatcher,
            notification_receiver,
            task_dispatcher,
            task_receiver,
            next_window_instance_id: 1,
            windows_by_key: HashMap::new(),
            window_keys_by_id: HashMap::new(),
            closed_window_keys: HashSet::new(),
            last_window_sync_revision: 0,
            windows_need_sync: true,
            portal_registry: PortalRegistry::default(),
            shutting_down: false,
            #[cfg(target_os = "windows")]
            main_window_disabled_for_modal: false,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: TguiError) {
        Log::with_tag("tgui-runtime").error(format_args!("multi-window runtime failed: {error}"));
        self.error = Some(error);
        event_loop.exit();
    }

    fn next_window_instance_id(&mut self) -> u64 {
        let next = self.next_window_instance_id;
        self.next_window_instance_id = self.next_window_instance_id.wrapping_add(1);
        next
    }

    fn main_window_key(&self) -> Option<&str> {
        self.windows_by_key.iter().find_map(|(key, window)| {
            if window.is_main_window() {
                Some(key.as_str())
            } else {
                None
            }
        })
    }

    fn main_window_ref(&self) -> Option<&Arc<dyn Window>> {
        let key = self.main_window_key()?;
        self.windows_by_key.get(key)?.window.as_ref()
    }

    #[cfg(target_os = "windows")]
    fn sync_native_modal_state(&mut self) {
        let should_disable_main = self.main_window_is_blocked();
        if self.main_window_disabled_for_modal == should_disable_main {
            return;
        }

        if let Some(window) = self.main_window_ref() {
            window.set_enable(!should_disable_main);
            self.main_window_disabled_for_modal = should_disable_main;
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn sync_native_modal_state(&mut self) {}

    fn resolve_windows(&self) -> Result<Vec<ResolvedWindowSpec<VM>>, TguiError> {
        let view_model = self.view_model.lock().expect("view model lock poisoned");
        let specs = (self.windows.factory)(&view_model);
        let mut keys = HashSet::new();
        let mut main_window_count = 0usize;
        let mut resolved = Vec::with_capacity(specs.len());

        for spec in specs {
            let key = spec.key.clone();
            if !keys.insert(key.clone()) {
                return Err(TguiError::Unsupported(format!(
                    "window factory returned a duplicate window key: {key}"
                )));
            }

            if matches!(spec.role, WindowRole::Main) {
                main_window_count += 1;
            }

            let widget_tree = if self.windows_by_key.contains_key(&key) {
                None
            } else {
                spec.build_widget_tree(&view_model)
            };
            let root_view = spec.root_view.clone();

            resolved.push(ResolvedWindowSpec {
                key,
                role: spec.role,
                config: spec.resolved_config(&self.config),
                window_bindings: spec.build_window_bindings(&view_model),
                widget_tree,
                root_view,
                commands: spec.commands,
                close_policy: spec.close_policy,
            });
        }

        if resolved.is_empty() {
            return Ok(resolved);
        }

        if main_window_count != 1 {
            return Err(TguiError::Unsupported(format!(
                "multi-window applications must declare exactly one main window, found {main_window_count}"
            )));
        }

        Ok(resolved)
    }

    fn main_window_is_blocked(&self) -> bool {
        self.windows_by_key
            .values()
            .any(BoundRuntimeHandler::blocks_main_window)
    }

    fn should_gate_main_window_event(event: &WindowEvent) -> bool {
        matches!(
            event,
            WindowEvent::PointerMoved { .. }
                | WindowEvent::PointerLeft { .. }
                | WindowEvent::PointerButton { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::Ime(_)
                | WindowEvent::ModifiersChanged(_)
        )
    }

    fn sync_windows(&mut self, event_loop: &dyn ActiveEventLoop, force: bool) {
        if self.shutting_down {
            return;
        }

        let revision = self.invalidation.revision();
        if !force
            && !self.windows_need_sync
            && !self.windows_by_key.is_empty()
            && revision == self.last_window_sync_revision
        {
            return;
        }

        let mut resolved = match self.resolve_windows() {
            Ok(resolved) => resolved,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        resolved.sort_by_key(|window| window_sync_priority(window.role));

        let desired_keys: HashSet<String> =
            resolved.iter().map(|window| window.key.clone()).collect();
        self.closed_window_keys
            .retain(|key| desired_keys.contains(key));

        for resolved_window in resolved {
            if self.closed_window_keys.contains(&resolved_window.key) {
                continue;
            }

            let key = resolved_window.key.clone();
            let modal_parent = if matches!(
                resolved_window.role,
                WindowRole::Child {
                    blocks_main_window: true
                }
            ) {
                self.main_window_ref().cloned()
            } else {
                None
            };
            if let Some(window) = self.windows_by_key.get_mut(&key) {
                window.set_definition(
                    resolved_window.role,
                    resolved_window.config,
                    resolved_window.window_bindings,
                    resolved_window.root_view,
                    resolved_window.commands,
                    resolved_window.close_policy,
                );
                window.create_or_resume_surface(event_loop, modal_parent.as_ref());
                if let Some(error) = window.error.take() {
                    self.fail(event_loop, error);
                    return;
                }
                self.window_keys_by_id
                    .retain(|_, existing_key| existing_key != &key);
                if let Some(window_id) = window.window_id() {
                    self.window_keys_by_id.insert(window_id, key);
                }
            } else {
                let mut window = BoundRuntimeHandler::new(
                    key.clone(),
                    self.next_window_instance_id(),
                    resolved_window.role,
                    resolved_window.config,
                    self.view_model.clone(),
                    resolved_window.window_bindings,
                    resolved_window.widget_tree,
                    resolved_window.root_view,
                    resolved_window.commands,
                    self.invalidation.clone(),
                    self.animations.clone(),
                    self.dialog_dispatcher.clone(),
                    None,
                    self.notification_dispatcher.clone(),
                    None,
                    self.task_dispatcher.clone(),
                    None,
                );
                window.close_policy = resolved_window.close_policy;
                window.create_or_resume_surface(event_loop, modal_parent.as_ref());
                if let Some(error) = window.error.take() {
                    self.fail(event_loop, error);
                    return;
                }
                if let Some(window_id) = window.window_id() {
                    self.window_keys_by_id.insert(window_id, key.clone());
                }
                self.windows_by_key.insert(key, window);
            }
        }

        let stale_keys: Vec<String> = self
            .windows_by_key
            .keys()
            .filter(|key| {
                !desired_keys.contains(*key) || self.closed_window_keys.contains(key.as_str())
            })
            .cloned()
            .collect();

        for key in stale_keys {
            self.remove_window(&key);
        }

        self.sync_native_modal_state();

        if self.windows_by_key.is_empty() {
            event_loop.exit();
        }

        self.last_window_sync_revision = revision;
        self.windows_need_sync = false;
    }

    fn remove_window(&mut self, key: &str) {
        if let Some(window) = self.windows_by_key.remove(key) {
            if let Some(window_id) = window.window_id() {
                self.window_keys_by_id.remove(&window_id);
            }
        }
        let changed_targets = self.portal_registry.remove_source(key);
        self.apply_portal_target_updates(changed_targets);
    }

    fn sync_portal_registry(&mut self) {
        let keys = self.windows_by_key.keys().cloned().collect::<Vec<_>>();
        let mut targets_to_update = std::collections::BTreeSet::new();
        for key in &keys {
            let Some(window) = self.windows_by_key.get_mut(key) else {
                continue;
            };
            let requests = window.external_portal_requests_from_computed();
            targets_to_update.extend(self.portal_registry.publish_source(key, requests));
        }
        targets_to_update.extend(keys);
        self.apply_portal_target_updates(targets_to_update.into_iter().collect());
    }

    fn apply_portal_target_updates(&mut self, targets: Vec<String>) {
        for target in targets {
            let requests = self.portal_registry.requests_for_target(&target);
            let revision = self.portal_registry.target_revision(&target);
            if let Some(window) = self.windows_by_key.get_mut(&target) {
                window.set_external_portal_requests(requests, revision);
            }
        }
    }
}

impl<VM: ViewModel> ApplicationHandler for MultiWindowHandler<VM> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.set_dialog_proxy(event_loop);
        self.sync_windows(event_loop, true);
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, _event: ()) {
        self.invalidation.acknowledge_wake();
        self.drain_dialog_completions();
        self.drain_notification_completions();
        self.drain_task_completions();
        if self.invalidation.take_redraw_request() {
            for window in self.windows_by_key.values() {
                if let Some(native_window) = window.window.as_ref() {
                    native_window.request_redraw();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(key) = self.window_keys_by_id.get(&window_id).cloned() else {
            return;
        };

        let events = self
            .windows_by_key
            .get(&key)
            .map(|window| WindowEvent::from_winit(event, window.physical_cursor_position()))
            .unwrap_or_default();

        let is_main_window = self
            .windows_by_key
            .get(&key)
            .map(BoundRuntimeHandler::is_main_window)
            .unwrap_or(false);

        let mut close_requested = false;
        for event in events {
            if is_main_window
                && self.main_window_is_blocked()
                && Self::should_gate_main_window_event(&event)
            {
                continue;
            }

            close_requested |= self
                .windows_by_key
                .get_mut(&key)
                .map(|window| window.handle_bound_window_event(event_loop, event))
                .unwrap_or(false);
        }

        if let Some(window) = self.windows_by_key.get_mut(&key) {
            if let Some(error) = window.error.take() {
                self.fail(event_loop, error);
                return;
            }
        }

        if close_requested {
            if is_main_window && self.config.close_children_with_main {
                self.shutting_down = true;
                self.windows_by_key.clear();
                self.window_keys_by_id.clear();
                event_loop.exit();
                return;
            }

            self.closed_window_keys.insert(key.clone());
            self.remove_window(&key);
            self.sync_native_modal_state();
            if self.windows_by_key.is_empty() {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.shutting_down {
            event_loop.exit();
            return;
        }

        self.drain_dialog_completions();
        self.drain_notification_completions();
        self.drain_task_completions();
        self.sync_windows(event_loop, false);
        if self.error.is_some() {
            return;
        }
        self.sync_portal_registry();

        let keys: Vec<String> = self.windows_by_key.keys().cloned().collect();
        for key in keys {
            let (close_requested, is_main_window) =
                if let Some(window) = self.windows_by_key.get_mut(&key) {
                    let close_requested = window.handle_bound_about_to_wait(event_loop);
                    let is_main_window = window.is_main_window();
                    if let Some(error) = window.error.take() {
                        self.fail(event_loop, error);
                        return;
                    }
                    self.window_keys_by_id
                        .retain(|_, existing_key| existing_key != &key);
                    if let Some(window_id) = window.window_id() {
                        self.window_keys_by_id.insert(window_id, key.clone());
                    }
                    (close_requested, is_main_window)
                } else {
                    (false, false)
                };

            if close_requested {
                if is_main_window && self.config.close_children_with_main {
                    self.shutting_down = true;
                    self.windows_by_key.clear();
                    self.window_keys_by_id.clear();
                    event_loop.exit();
                    return;
                }

                self.closed_window_keys.insert(key.clone());
                self.remove_window(&key);
                self.sync_native_modal_state();
                if self.windows_by_key.is_empty() {
                    event_loop.exit();
                }
                return;
            }
        }
    }

    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        for window in self.windows_by_key.values_mut() {
            window.suspend();
        }
    }
}
