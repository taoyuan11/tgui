#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::OhosApp;
use crate::platform::dpi::LogicalSize;

use crate::animation::AnimationCoordinator;
use crate::foundation::binding::{Binding, InvalidationSignal, ViewModelContext};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::runtime::{BoundRuntime, Runtime, WindowBindings, WindowCommand};
use crate::text::font::FontCatalog;
use crate::ui::theme::{Theme, ThemeMode};
use crate::ui::widget::{Element, WidgetTree};

#[derive(Debug, Clone)]
pub(crate) enum ThemeSelection {
    System,
    Fixed(Theme),
}

impl ThemeSelection {
    pub(crate) fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::System => Self::System,
            ThemeMode::Light | ThemeMode::Dark => Self::Fixed(Theme::from_mode(mode, None)),
        }
    }
}

#[derive(Debug, Clone)]
/// Entry point for configuring and launching a `tgui` application.
///
/// Use `Application` directly for a simple window, or call
/// [`Application::with_view_model`] to move into the MVVM builder flow.
///
/// ```no_run
/// use tgui::Application;
///
/// fn main() -> Result<(), tgui::TguiError> {
///     Application::new()
///         .title("Demo")
///         .window_size(1024, 768)
///         .run()
/// }
/// ```
pub struct Application {
    title: String,
    width: u32,
    height: u32,
    clear_color: Color,
    clear_color_overridden: bool,
    fonts: FontCatalog,
    theme: ThemeSelection,
}

impl Application {
    /// Creates an application with default title, window size, dark theme colors,
    /// and an empty font catalog.
    pub fn new() -> Self {
        Self {
            title: "tgui".to_string(),
            width: 800,
            height: 600,
            clear_color: Theme::default().palette.window_background,
            clear_color_overridden: false,
            fonts: FontCatalog::default(),
            theme: ThemeSelection::System,
        }
    }

    /// Sets the initial window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the initial window size in logical pixels.
    ///
    /// Values are clamped to at least `1x1`.
    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    /// Overrides the window clear color used by the renderer.
    pub fn clear_color(mut self, clear_color: Color) -> Self {
        self.clear_color = clear_color;
        self.clear_color_overridden = true;
        self
    }

    /// Registers an in-memory font blob under a logical family name.
    pub fn font(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
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
            self.clear_color = theme.palette.window_background;
        }
        self.theme = ThemeSelection::Fixed(theme);
        self
    }

    /// Runs the application without a view model.
    pub fn run(self) -> Result<(), TguiError> {
        Runtime::new(self.config())?.run()
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    pub fn run_android(self, app: AndroidApp) -> Result<(), TguiError> {
        Runtime::new_android(self.config(), app)?.run()
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub fn run_ohos(self, app: OhosApp) -> Result<(), TguiError> {
        Runtime::new_ohos(self.config(), app)?.run()
    }

    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    pub fn into_ohos_handler(self) -> impl winit_core::application::ApplicationHandler + Send {
        Runtime::handler(self.config())
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
        ApplicationConfig {
            title: self.title.clone(),
            size: LogicalSize::new(self.width as f64, self.height as f64),
            clear_color: self.clear_color,
            clear_color_overridden: self.clear_color_overridden,
            fonts: self.fonts.clone(),
            theme: self.theme.clone(),
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ApplicationConfig {
    pub(crate) title: String,
    pub(crate) size: LogicalSize<f64>,
    pub(crate) clear_color: Color,
    pub(crate) clear_color_overridden: bool,
    pub(crate) fonts: FontCatalog,
    pub(crate) theme: ThemeSelection,
}

type TitleBinding<VM> = Box<dyn Fn(&VM) -> Binding<String> + Send + Sync>;
type ClearColorBinding<VM> = Box<dyn Fn(&VM) -> Binding<Color> + Send + Sync>;
type ThemeModeBinding<VM> = Box<dyn Fn(&VM) -> Binding<ThemeMode> + Send + Sync>;
type RootViewFactory<VM> = Box<dyn Fn(&VM) -> Element<VM> + Send + Sync>;

pub struct ApplicationBuilder<VM, F>
where
    VM: ViewModel,
    F: FnOnce(&ViewModelContext) -> VM,
{
    app: Application,
    factory: F,
    title_binding: Option<TitleBinding<VM>>,
    clear_color_binding: Option<ClearColorBinding<VM>>,
    theme_mode_binding: Option<ThemeModeBinding<VM>>,
    root_view: Option<RootViewFactory<VM>>,
    commands: Vec<WindowCommand<VM>>,
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
            theme_mode_binding: None,
            root_view: None,
            commands: Vec::new(),
        }
    }

    /// Binds the window title to view-model state.
    pub fn bind_title(
        mut self,
        binding: impl Fn(&VM) -> Binding<String> + Send + Sync + 'static,
    ) -> Self {
        self.title_binding = Some(Box::new(binding));
        self
    }

    /// Binds the window clear color to view-model state.
    pub fn bind_clear_color(
        mut self,
        binding: impl Fn(&VM) -> Binding<Color> + Send + Sync + 'static,
    ) -> Self {
        self.clear_color_binding = Some(Box::new(binding));
        self
    }

    /// Binds the active theme mode to view-model state.
    ///
    /// Runtime theme changes are animated automatically.
    pub fn bind_theme_mode(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeMode> + Send + Sync + 'static,
    ) -> Self {
        self.theme_mode_binding = Some(Box::new(binding));
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
        self.root_view = Some(Box::new(root_view));
        self
    }

    /// Builds the runtime and starts the application event loop.
    pub fn run(self) -> Result<(), TguiError> {
        let (
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        ) = self.into_runtime_parts();

        BoundRuntime::new(
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        )?
        .run()
    }

    #[cfg(all(target_os = "android", feature = "android"))]
    pub fn run_android(self, app: AndroidApp) -> Result<(), TguiError> {
        let (
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        ) = self.into_runtime_parts();

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
        let (
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        ) = self.into_runtime_parts();

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
        let (
            config,
            view_model,
            window_bindings,
            widget_tree,
            commands,
            invalidation,
            animations,
        ) = self.into_runtime_parts();

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
        WindowBindings,
        Option<WidgetTree<VM>>,
        Vec<WindowCommand<VM>>,
        InvalidationSignal,
        AnimationCoordinator,
    ) {
        let invalidation = InvalidationSignal::new();
        let animations = AnimationCoordinator::default();
        let context = ViewModelContext::new(invalidation.clone(), animations.clone());
        let view_model = (self.factory)(&context);
        let window_bindings = WindowBindings {
            title: self.title_binding.map(|binding| binding(&view_model)),
            clear_color: self.clear_color_binding.map(|binding| binding(&view_model)),
            theme_mode: self.theme_mode_binding.map(|binding| binding(&view_model)),
        };
        let widget_tree = self
            .root_view
            .map(|root_view| WidgetTree::new(root_view(&view_model)));
        (
            self.app.config(),
            view_model,
            window_bindings,
            widget_tree,
            self.commands,
            invalidation,
            animations,
        )
    }
}
