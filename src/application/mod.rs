use std::sync::Arc;

#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
use crate::platform::dpi::LogicalSize;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::OhosApp;

use crate::animation::AnimationCoordinator;
use crate::foundation::binding::{Binding, InvalidationSignal, ViewModelContext};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::runtime::{BoundRuntime, WindowBindings, WindowCommand};
use crate::text::font::FontCatalog;
use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
use crate::ui::unit::Dp;
use crate::ui::widget::{Element, WidgetTree};

fn logical_window_size(width: Dp, height: Dp) -> LogicalSize<f64> {
    LogicalSize::new(
        width.max(Dp::new(1.0)).get() as f64,
        height.max(Dp::new(1.0)).get() as f64,
    )
}

fn max_logical_size(lhs: LogicalSize<f64>, rhs: LogicalSize<f64>) -> LogicalSize<f64> {
    LogicalSize::new(lhs.width.max(rhs.width), lhs.height.max(rhs.height))
}

fn min_logical_size(lhs: LogicalSize<f64>, rhs: LogicalSize<f64>) -> LogicalSize<f64> {
    LogicalSize::new(lhs.width.min(rhs.width), lhs.height.min(rhs.height))
}

#[derive(Debug, Clone)]
pub(crate) enum ThemeSelection {
    System,
    Mode(ThemeMode),
    Fixed(Theme),
}

impl ThemeSelection {
    pub(crate) fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::System => Self::System,
            ThemeMode::Light | ThemeMode::Dark => Self::Mode(mode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Application, WindowSpec};

    #[test]
    fn application_decorations_updates_config() {
        let config = Application::new().decorations(false).config();

        assert!(!config.decorations);
    }

    #[test]
    fn window_spec_decorations_override_application_default() {
        let app_config = Application::new().decorations(true).config();
        let window_config = WindowSpec::<()>::main("main")
            .decorations(false)
            .resolved_config(&app_config);

        assert!(!window_config.decorations);
    }

    #[test]
    fn window_spec_decorations_inherit_application_default() {
        let app_config = Application::new().decorations(false).config();
        let window_config = WindowSpec::<()>::main("main").resolved_config(&app_config);

        assert!(!window_config.decorations);
    }

    #[test]
    fn application_app_id_updates_config() {
        let config = Application::new().app_id("com.tgui.test").config();

        assert_eq!(config.app_id.as_deref(), Some("com.tgui.test"));
    }

    #[test]
    fn window_spec_inherits_application_app_id() {
        let app_config = Application::new().app_id("com.tgui.test").config();
        let window_config = WindowSpec::<()>::main("main").resolved_config(&app_config);

        assert_eq!(window_config.app_id.as_deref(), Some("com.tgui.test"));
    }
}

#[derive(Debug, Clone)]
/// Entry point for configuring and launching a `tgui` application.
///
/// `tgui` applications are MVVM-only: configure the window with `Application`,
/// then call [`Application::with_view_model`] to bind a view model and root view.
///
/// ```no_run
/// use tgui::prelude::*;
///
/// struct AppVm;
///
/// impl AppVm {
///     fn new(_: &ViewModelContext) -> Self {
///         Self
///     }
///
///     fn view(&self) -> Element<Self> {
///         Text::new("Hello tgui").into()
///     }
/// }
///
/// impl ViewModel for AppVm {
///     fn new(context: &ViewModelContext) -> Self {
///         AppVm::new(context)
///     }
///
///     fn view(&self) -> Element<Self> {
///         AppVm::view(self)
///     }
/// }
///
/// fn main() -> Result<(), TguiError> {
///     Application::new()
///         .title("Demo")
///         .window_size(dp(1024.0), dp(768.0))
///         .with_view_model(AppVm::new)
///         .root_view(AppVm::view)
///         .run()
/// }
/// ```
pub struct Application {
    app_id: Option<String>,
    title: String,
    width: Dp,
    height: Dp,
    min_size: Option<LogicalSize<f64>>,
    max_size: Option<LogicalSize<f64>>,
    clear_color: Color,
    clear_color_overridden: bool,
    close_children_with_main: bool,
    decorations: bool,
    fonts: FontCatalog,
    theme: ThemeSelection,
    theme_set: ThemeSet,
    window_icon: Option<&'static [u8]>,
}

impl Application {
    /// Creates an application with default title, window size, dark theme colors,
    /// and an empty font catalog.
    pub fn new() -> Self {
        Self {
            app_id: None,
            title: "tgui".to_string(),
            width: Dp::new(800.0),
            height: Dp::new(600.0),
            min_size: None,
            max_size: None,
            clear_color: Theme::default().colors.background,
            clear_color_overridden: false,
            close_children_with_main: true,
            decorations: true,
            fonts: FontCatalog::default(),
            theme: ThemeSelection::System,
            theme_set: ThemeSet::default(),
            window_icon: None,
        }
    }

    /// Sets the stable application identifier used by platform services such as notifications.
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Sets the initial window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the initial window size in logical pixels.
    ///
    /// Values are clamped to at least `1x1`.
    pub fn window_size(mut self, width: Dp, height: Dp) -> Self {
        self.width = width.max(Dp::new(1.0));
        self.height = height.max(Dp::new(1.0));
        self
    }

    /// Sets the minimum resizable window surface size in logical pixels.
    ///
    /// Values are clamped to at least `1x1`.
    pub fn min_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.min_size = Some(logical_window_size(width, height));
        self
    }

    /// Sets the maximum resizable window surface size in logical pixels.
    ///
    /// Values are clamped to at least `1x1`.
    pub fn max_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.max_size = Some(logical_window_size(width, height));
        self
    }

    /// Overrides the window clear color used by the renderer.
    pub fn clear_color(mut self, clear_color: Color) -> Self {
        self.clear_color = clear_color;
        self.clear_color_overridden = true;
        self
    }

    /// Controls whether closing the main window should also close all child windows.
    ///
    /// Defaults to `true`.
    pub fn close_children_with_main(mut self, close_children_with_main: bool) -> Self {
        self.close_children_with_main = close_children_with_main;
        self
    }

    /// Sets whether the native window should use system decorations.
    ///
    /// Defaults to `true`. Set this to `false` to draw a custom title bar.
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    /// Set window icon and status bar icon
    ///
    /// Defaults to `None`.
    pub fn window_icon(mut self, icon: &'static [u8]) -> Self {
        self.window_icon = Some(icon);
        self
    }

    /// Registers an in-memory font blob under a logical family name.
    pub fn font_bytes(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
        self.fonts.register_font(name, bytes);
        self
    }

    /// Registers a font file from disk under a logical family name.
    pub fn font_file(
        mut self,
        name: impl Into<String>,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        self.fonts.register_font_file(name, path);
        self
    }

    /// Sets the logical font family that widgets use when they do not specify one.
    pub fn default_font(mut self, name: impl Into<String>) -> Self {
        self.fonts.set_default_font(name);
        self
    }

    /// Applies a fixed theme for the whole application.
    ///
    /// If no clear color override has been set, the theme's window background is
    /// also used as the renderer clear color.
    pub fn theme(mut self, theme: Theme) -> Self {
        if !self.clear_color_overridden {
            self.clear_color = theme.colors.background;
        }
        self.theme = ThemeSelection::Fixed(theme);
        self
    }

    /// Sets the light and dark themes used by [`ThemeMode`] resolution.
    ///
    /// This does not force a fixed theme. Instead, [`ThemeMode::Light`],
    /// [`ThemeMode::Dark`], and [`ThemeMode::System`] resolve through this set.
    pub fn theme_set(mut self, theme_set: ThemeSet) -> Self {
        if !self.clear_color_overridden {
            self.clear_color = theme_set.dark.colors.background;
        }
        self.theme_set = theme_set;
        self
    }

    /// Starts the MVVM builder flow for applications backed by a view model.
    ///
    /// The factory receives a [`ViewModelContext`] that can create observables,
    /// animated values, and timeline controllers.
    pub fn with_view_model<VM, F>(self, factory: F) -> ApplicationBuilder<VM, F>
    where
        VM: ViewModel,
        F: FnOnce(&ViewModelContext) -> VM,
    {
        ApplicationBuilder::new(self, factory)
    }

    pub(crate) fn config(&self) -> ApplicationConfig {
        let mut config = ApplicationConfig {
            app_id: self.app_id.clone(),
            title: self.title.clone(),
            size: LogicalSize::new(self.width.get() as f64, self.height.get() as f64),
            min_size: self.min_size,
            max_size: self.max_size,
            clear_color: self.clear_color,
            clear_color_overridden: self.clear_color_overridden,
            close_children_with_main: self.close_children_with_main,
            decorations: self.decorations,
            fonts: self.fonts.clone(),
            theme: self.theme.clone(),
            theme_set: self.theme_set.clone(),
            window_icon: self.window_icon,
        };
        config.normalize_size_constraints();
        config
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ApplicationConfig {
    pub(crate) app_id: Option<String>,
    pub(crate) title: String,
    pub(crate) size: LogicalSize<f64>,
    pub(crate) min_size: Option<LogicalSize<f64>>,
    pub(crate) max_size: Option<LogicalSize<f64>>,
    pub(crate) clear_color: Color,
    pub(crate) clear_color_overridden: bool,
    pub(crate) close_children_with_main: bool,
    pub(crate) decorations: bool,
    pub(crate) fonts: FontCatalog,
    pub(crate) theme: ThemeSelection,
    pub(crate) theme_set: ThemeSet,
    pub(crate) window_icon: Option<&'static [u8]>,
}

impl ApplicationConfig {
    pub(crate) fn normalize_size_constraints(&mut self) {
        if let (Some(min_size), Some(max_size)) = (self.min_size, self.max_size) {
            self.max_size = Some(max_logical_size(max_size, min_size));
        }

        if let Some(min_size) = self.min_size {
            self.size = max_logical_size(self.size, min_size);
        }

        if let Some(max_size) = self.max_size {
            self.size = min_logical_size(self.size, max_size);
        }
    }
}

type TitleBinding<VM> = Arc<dyn Fn(&VM) -> Binding<String> + Send + Sync>;
type ClearColorBinding<VM> = Arc<dyn Fn(&VM) -> Binding<Color> + Send + Sync>;
type ThemeSetBinding<VM> = Arc<dyn Fn(&VM) -> Binding<ThemeSet> + Send + Sync>;
type ThemeModeBinding<VM> = Arc<dyn Fn(&VM) -> Binding<ThemeMode> + Send + Sync>;
type RootViewFactory<VM> = Arc<dyn Fn(&VM) -> Element<VM> + Send + Sync>;
type WindowsFactory<VM> = Box<dyn Fn(&VM) -> Vec<WindowSpec<VM>> + Send + Sync>;

fn build_root_element<VM>(root_view: &RootViewFactory<VM>, view_model: &VM) -> Element<VM> {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        const ROOT_VIEW_STACK_SIZE: usize = 8 * 1024 * 1024;
        const ROOT_VIEW_STACK_RED_ZONE: usize = ROOT_VIEW_STACK_SIZE;
        stacker::maybe_grow(ROOT_VIEW_STACK_RED_ZONE, ROOT_VIEW_STACK_SIZE, || {
            root_view(view_model)
        })
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        root_view(view_model)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowClosePolicy {
    /// Close the native window and keep the rest of the application running.
    #[default]
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRole {
    Main,
    Child { blocks_main_window: bool },
}

/// Declarative description of a runtime-managed window.
pub struct WindowSpec<VM> {
    pub(crate) key: String,
    pub(crate) role: WindowRole,
    pub(crate) title: Option<String>,
    pub(crate) size: Option<LogicalSize<f64>>,
    pub(crate) min_size: Option<LogicalSize<f64>>,
    pub(crate) max_size: Option<LogicalSize<f64>>,
    pub(crate) decorations: Option<bool>,
    pub(crate) title_binding: Option<TitleBinding<VM>>,
    pub(crate) clear_color_binding: Option<ClearColorBinding<VM>>,
    pub(crate) theme_set_binding: Option<ThemeSetBinding<VM>>,
    pub(crate) theme_mode_binding: Option<ThemeModeBinding<VM>>,
    pub(crate) root_view: Option<RootViewFactory<VM>>,
    pub(crate) commands: Vec<WindowCommand<VM>>,
    pub(crate) close_policy: WindowClosePolicy,
}

impl<VM> WindowSpec<VM> {
    /// Creates a window specification identified by a stable key.
    pub fn new(key: impl Into<String>) -> Self {
        Self::main(key)
    }

    /// Creates the main window specification.
    pub fn main(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            role: WindowRole::Main,
            title: None,
            size: None,
            min_size: None,
            max_size: None,
            decorations: None,
            title_binding: None,
            clear_color_binding: None,
            theme_set_binding: None,
            theme_mode_binding: None,
            root_view: None,
            commands: Vec::new(),
            close_policy: WindowClosePolicy::Close,
        }
    }

    /// Creates a child window specification.
    ///
    /// Child windows are non-blocking by default.
    pub fn child(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            role: WindowRole::Child {
                blocks_main_window: false,
            },
            title: None,
            size: None,
            min_size: None,
            max_size: None,
            decorations: None,
            title_binding: None,
            clear_color_binding: None,
            theme_set_binding: None,
            theme_mode_binding: None,
            root_view: None,
            commands: Vec::new(),
            close_policy: WindowClosePolicy::Close,
        }
    }

    /// Sets the initial native window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the initial native window size in logical pixels.
    pub fn window_size(mut self, width: Dp, height: Dp) -> Self {
        self.size = Some(logical_window_size(width, height));
        self
    }

    /// Sets the minimum resizable native window size in logical pixels.
    pub fn min_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.min_size = Some(logical_window_size(width, height));
        self
    }

    /// Sets the maximum resizable native window size in logical pixels.
    pub fn max_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.max_size = Some(logical_window_size(width, height));
        self
    }

    /// Sets whether this native window should use system decorations.
    ///
    /// If unset, the application-level setting is used.
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = Some(decorations);
        self
    }

    /// Binds the window title to shared view-model state.
    pub fn bind_title(
        mut self,
        binding: impl Fn(&VM) -> Binding<String> + Send + Sync + 'static,
    ) -> Self {
        self.title_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the renderer clear color to shared view-model state.
    pub fn bind_clear_color(
        mut self,
        binding: impl Fn(&VM) -> Binding<Color> + Send + Sync + 'static,
    ) -> Self {
        self.clear_color_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the light and dark themes used by theme mode resolution.
    pub fn bind_theme_set(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeSet> + Send + Sync + 'static,
    ) -> Self {
        self.theme_set_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the active theme mode to shared view-model state.
    pub fn bind_theme_mode(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeMode> + Send + Sync + 'static,
    ) -> Self {
        self.theme_mode_binding = Some(Arc::new(binding));
        self
    }

    /// Registers the root widget tree factory for this window.
    pub fn root_view(
        mut self,
        root_view: impl Fn(&VM) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.root_view = Some(Arc::new(root_view));
        self
    }

    /// Registers a window-scoped input binding.
    pub fn on_input(mut self, trigger: InputTrigger, command: Command<VM>) -> Self {
        self.commands.push(WindowCommand { trigger, command });
        self
    }

    /// Configures how the runtime should react when the native window requests closing.
    pub fn close_policy(mut self, close_policy: WindowClosePolicy) -> Self {
        self.close_policy = close_policy;
        self
    }

    /// Marks whether a child window should block interaction with the main window.
    ///
    /// The default is `false`.
    pub fn blocks_main_window(mut self, blocks_main_window: bool) -> Self {
        self.role = match self.role {
            WindowRole::Main => WindowRole::Main,
            WindowRole::Child { .. } => WindowRole::Child { blocks_main_window },
        };
        self
    }

    pub(crate) fn resolved_config(&self, app_config: &ApplicationConfig) -> ApplicationConfig {
        let mut config = app_config.clone();
        if let Some(title) = self.title.as_ref() {
            config.title = title.clone();
        }
        if let Some(size) = self.size {
            config.size = size;
        }
        if let Some(min_size) = self.min_size {
            config.min_size = Some(min_size);
        }
        if let Some(max_size) = self.max_size {
            config.max_size = Some(max_size);
        }
        if let Some(decorations) = self.decorations {
            config.decorations = decorations;
        }
        config.normalize_size_constraints();
        config
    }

    pub(crate) fn build_window_bindings(&self, view_model: &VM) -> WindowBindings {
        WindowBindings {
            title: self
                .title_binding
                .as_ref()
                .map(|binding| binding(view_model)),
            clear_color: self
                .clear_color_binding
                .as_ref()
                .map(|binding| binding(view_model)),
            theme_set: self
                .theme_set_binding
                .as_ref()
                .map(|binding| binding(view_model)),
            theme_mode: self
                .theme_mode_binding
                .as_ref()
                .map(|binding| binding(view_model)),
        }
    }

    pub(crate) fn build_widget_tree(&self, view_model: &VM) -> Option<WidgetTree<VM>> {
        self.root_view
            .as_ref()
            .map(|root_view| WidgetTree::new(build_root_element(root_view, view_model)))
    }
}

pub(crate) struct WindowSetFactory<VM> {
    pub(crate) factory: WindowsFactory<VM>,
    #[allow(dead_code)]
    pub(crate) explicit_windows: bool,
}

pub struct ApplicationBuilder<VM, F>
where
    VM: ViewModel,
    F: FnOnce(&ViewModelContext) -> VM,
{
    app: Application,
    factory: F,
    title_binding: Option<TitleBinding<VM>>,
    clear_color_binding: Option<ClearColorBinding<VM>>,
    theme_set_binding: Option<ThemeSetBinding<VM>>,
    theme_mode_binding: Option<ThemeModeBinding<VM>>,
    root_view: Option<RootViewFactory<VM>>,
    commands: Vec<WindowCommand<VM>>,
    windows_factory: Option<WindowsFactory<VM>>,
}

impl<VM, F> ApplicationBuilder<VM, F>
where
    VM: ViewModel,
    F: FnOnce(&ViewModelContext) -> VM,
{
    fn new(app: Application, factory: F) -> Self {
        Self {
            app,
            factory,
            title_binding: None,
            clear_color_binding: None,
            theme_set_binding: None,
            theme_mode_binding: None,
            root_view: None,
            commands: Vec::new(),
            windows_factory: None,
        }
    }

    /// Binds the window title to view-model state.
    pub fn bind_title(
        mut self,
        binding: impl Fn(&VM) -> Binding<String> + Send + Sync + 'static,
    ) -> Self {
        self.title_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the window clear color to view-model state.
    pub fn bind_clear_color(
        mut self,
        binding: impl Fn(&VM) -> Binding<Color> + Send + Sync + 'static,
    ) -> Self {
        self.clear_color_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the light and dark themes used by theme mode resolution.
    ///
    /// Runtime theme changes are animated automatically.
    pub fn bind_theme_set(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeSet> + Send + Sync + 'static,
    ) -> Self {
        self.theme_set_binding = Some(Arc::new(binding));
        self
    }

    /// Binds the active theme mode to view-model state.
    ///
    /// Runtime theme changes are animated automatically.
    pub fn bind_theme_mode(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeMode> + Send + Sync + 'static,
    ) -> Self {
        self.theme_mode_binding = Some(Arc::new(binding));
        self
    }

    /// Registers a global input binding on the application window.
    pub fn on_input(mut self, trigger: InputTrigger, command: Command<VM>) -> Self {
        self.commands.push(WindowCommand { trigger, command });
        self
    }

    /// Registers the root widget tree factory for the application.
    pub fn root_view(
        mut self,
        root_view: impl Fn(&VM) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.root_view = Some(Arc::new(root_view));
        self
    }

    /// Registers a dynamic window factory evaluated against the shared view model.
    pub fn windows(
        mut self,
        factory: impl Fn(&VM) -> Vec<WindowSpec<VM>> + Send + Sync + 'static,
    ) -> Self {
        self.windows_factory = Some(Box::new(factory));
        self
    }

    /// Builds the runtime and starts the application event loop.
    pub fn run(self) -> Result<(), TguiError> {
        let (config, view_model, windows, invalidation, animations) = self.into_runtime_parts();

        BoundRuntime::new(config, view_model, windows, invalidation, animations)?.run()
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    pub fn run_android(self, app: AndroidApp) -> Result<(), TguiError> {
        let (config, view_model, windows, invalidation, animations) = self.into_runtime_parts();

        if windows.explicit_windows {
            return Err(TguiError::Unsupported(
                "multi-window applications are not supported on Android yet".to_string(),
            ));
        }

        let main = (windows.factory)(&view_model)
            .into_iter()
            .next()
            .unwrap_or_else(|| WindowSpec::new("main"));
        let window_bindings = main.build_window_bindings(&view_model);
        let widget_tree = main.build_widget_tree(&view_model);
        let commands = main.commands;

        BoundRuntime::new_android(
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
            app,
        )?
        .run()
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub fn run_ohos(self, app: OhosApp) -> Result<(), TguiError> {
        let (config, view_model, windows, invalidation, animations) = self.into_runtime_parts();

        if windows.explicit_windows {
            return Err(TguiError::Unsupported(
                "multi-window applications are not supported on OHOS yet".to_string(),
            ));
        }

        let main = (windows.factory)(&view_model)
            .into_iter()
            .next()
            .unwrap_or_else(|| WindowSpec::new("main"));
        let window_bindings = main.build_window_bindings(&view_model);
        let widget_tree = main.build_widget_tree(&view_model);
        let commands = main.commands;

        BoundRuntime::new_ohos(
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
            app,
        )?
        .run()
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub fn into_ohos_handler(self) -> impl winit_core::application::ApplicationHandler + Send
    where
        VM: Send,
    {
        let (config, view_model, windows, invalidation, animations) = self.into_runtime_parts();

        if windows.explicit_windows {
            panic!("multi-window applications are not supported on OHOS yet");
        }

        let main = (windows.factory)(&view_model)
            .into_iter()
            .next()
            .unwrap_or_else(|| WindowSpec::new("main"));
        let window_bindings = main.build_window_bindings(&view_model);
        let widget_tree = main.build_widget_tree(&view_model);
        let commands = main.commands;

        BoundRuntime::handler(
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        )
    }

    fn into_runtime_parts(
        self,
    ) -> (
        ApplicationConfig,
        VM,
        WindowSetFactory<VM>,
        InvalidationSignal,
        AnimationCoordinator,
    ) {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = (self.factory)(&context);
        let config = self.app.config();
        let windows = if let Some(factory) = self.windows_factory {
            WindowSetFactory {
                factory,
                explicit_windows: true,
            }
        } else {
            let title_binding = self.title_binding;
            let clear_color_binding = self.clear_color_binding;
            let theme_set_binding = self.theme_set_binding;
            let theme_mode_binding = self.theme_mode_binding;
            let root_view = self.root_view;
            let commands = self.commands;
            let main_config = config.clone();
            WindowSetFactory {
                factory: Box::new(move |_vm| {
                    vec![WindowSpec {
                        key: "main".to_string(),
                        role: WindowRole::Main,
                        title: Some(main_config.title.clone()),
                        size: Some(main_config.size),
                        min_size: main_config.min_size,
                        max_size: main_config.max_size,
                        decorations: Some(main_config.decorations),
                        title_binding: title_binding.clone(),
                        clear_color_binding: clear_color_binding.clone(),
                        theme_set_binding: theme_set_binding.clone(),
                        theme_mode_binding: theme_mode_binding.clone(),
                        root_view: root_view.clone(),
                        commands: commands.clone(),
                        close_policy: WindowClosePolicy::Close,
                    }]
                }),
                explicit_windows: false,
            }
        };
        (config, view_model, windows, invalidation, animations)
    }
}
