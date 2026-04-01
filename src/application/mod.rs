use winit::dpi::LogicalSize;

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

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    pub fn clear_color(mut self, clear_color: Color) -> Self {
        self.clear_color = clear_color;
        self.clear_color_overridden = true;
        self
    }

    pub fn font(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
        self.fonts.register_font(name, bytes);
        self
    }

    pub fn default_font(mut self, name: impl Into<String>) -> Self {
        self.fonts.set_default_font(name);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        if !self.clear_color_overridden {
            self.clear_color = theme.palette.window_background;
        }
        self.theme = ThemeSelection::Fixed(theme);
        self
    }

    pub fn run(self) -> Result<(), TguiError> {
        Runtime::new(self.config())?.run()
    }

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

    pub fn bind_title(
        mut self,
        binding: impl Fn(&VM) -> Binding<String> + Send + Sync + 'static,
    ) -> Self {
        self.title_binding = Some(Box::new(binding));
        self
    }

    pub fn bind_clear_color(
        mut self,
        binding: impl Fn(&VM) -> Binding<Color> + Send + Sync + 'static,
    ) -> Self {
        self.clear_color_binding = Some(Box::new(binding));
        self
    }

    pub fn bind_theme_mode(
        mut self,
        binding: impl Fn(&VM) -> Binding<ThemeMode> + Send + Sync + 'static,
    ) -> Self {
        self.theme_mode_binding = Some(Box::new(binding));
        self
    }

    pub fn on_input(mut self, trigger: InputTrigger, command: Command<VM>) -> Self {
        self.commands.push(WindowCommand { trigger, command });
        self
    }

    pub fn root_view(
        mut self,
        root_view: impl Fn(&VM) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.root_view = Some(Box::new(root_view));
        self
    }

    pub fn run(self) -> Result<(), TguiError> {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone());
        let view_model = (self.factory)(&context);
        let window_bindings = WindowBindings {
            title: self.title_binding.map(|binding| binding(&view_model)),
            clear_color: self.clear_color_binding.map(|binding| binding(&view_model)),
            theme_mode: self.theme_mode_binding.map(|binding| binding(&view_model)),
        };
        let widget_tree = self
            .root_view
            .map(|root_view| WidgetTree::new(root_view(&view_model)));

        BoundRuntime::new(
            self.app.config(),
            view_model,
            window_bindings,
            widget_tree,
            self.commands,
            invalidation,
        )?
        .run()
    }
}
