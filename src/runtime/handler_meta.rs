use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn with_view_model<R>(&self, f: impl FnOnce(&mut VM) -> R) -> R {
        let mut view_model = self.view_model.lock().expect("view model lock poisoned");
        f(&mut view_model)
    }

    pub(in crate::runtime) fn set_definition(
        &mut self,
        role: WindowRole,
        config: ApplicationConfig,
        window_bindings: WindowBindings,
        commands: Vec<WindowCommand<VM>>,
        close_policy: WindowClosePolicy,
    ) {
        self.role = role;
        let font_manager = FontManager::new(&config.fonts);
        if let Some(window) = self.window.as_ref() {
            if window.is_decorated() != config.decorations {
                window.set_decorations(config.decorations);
            }
        }
        self.config = config;
        self.font_manager = font_manager;
        self.window_bindings = window_bindings;
        self.commands = commands;
        self.close_policy = close_policy;
    }

    pub(in crate::runtime) fn close_policy(&self) -> WindowClosePolicy {
        self.close_policy
    }

    pub(in crate::runtime) fn is_main_window(&self) -> bool {
        matches!(self.role, WindowRole::Main)
    }

    pub(in crate::runtime) fn blocks_main_window(&self) -> bool {
        matches!(
            self.role,
            WindowRole::Child {
                blocks_main_window: true
            }
        )
    }

    pub(in crate::runtime) fn fail(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        error: TguiError,
    ) {
        Log::with_tag("tgui-runtime").error(format_args!("bound runtime failed: {error}"));
        self.error = Some(error);
        event_loop.exit();
    }
}
