use std::sync::Arc;

#[cfg(all(target_os = "android", feature = "android"))]
use crate::platform::android::activity::AndroidApp;
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use crate::platform::ohos::OhosApp;

use crate::animation::AnimationCoordinator;
use crate::foundation::binding::{InvalidationSignal, Signal, ViewModelContext};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::{Command, ViewModel};
use crate::runtime::{BoundRuntime, WindowCommand};
use crate::ui::theme::{ThemeMode, ThemeSet};
use crate::ui::widget::Element;

use super::config::{Application, ApplicationConfig};
use super::window_spec::{
    ClearColorBinding, RootViewFactory, ThemeModeBinding, ThemeSetBinding, TitleBinding,
    WindowClosePolicy, WindowRole, WindowSpec, WindowsFactory,
};

pub(crate) struct WindowSetFactory<VM> {
    pub(crate) factory: WindowsFactory<VM>,
    #[allow(dead_code)]
    pub(crate) explicit_windows: bool,
}

/// 表示基于 ViewModel 的应用构建器。
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
    pub(crate) fn new(app: Application, factory: F) -> Self {
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

    /// 绑定窗口标题信号。
    pub fn bind_title(
        mut self,
        signal: impl Fn(&VM) -> Signal<String> + Send + Sync + 'static,
    ) -> Self {
        self.title_binding = Some(Arc::new(signal));
        self
    }

    /// 绑定窗口清屏颜色信号。
    pub fn bind_clear_color(
        mut self,
        signal: impl Fn(&VM) -> Signal<Color> + Send + Sync + 'static,
    ) -> Self {
        self.clear_color_binding = Some(Arc::new(signal));
        self
    }

    /// 绑定主题集合信号。
    pub fn bind_theme_set(
        mut self,
        signal: impl Fn(&VM) -> Signal<ThemeSet> + Send + Sync + 'static,
    ) -> Self {
        self.theme_set_binding = Some(Arc::new(signal));
        self
    }

    /// 绑定主题模式信号。
    pub fn bind_theme_mode(
        mut self,
        signal: impl Fn(&VM) -> Signal<ThemeMode> + Send + Sync + 'static,
    ) -> Self {
        self.theme_mode_binding = Some(Arc::new(signal));
        self
    }

    /// 注册应用主窗口的输入命令。
    pub fn on_input(mut self, trigger: InputTrigger, command: Command<VM>) -> Self {
        self.commands.push(WindowCommand { trigger, command });
        self
    }

    /// 注册应用主窗口的根视图工厂。
    pub fn root_view(
        mut self,
        root_view: impl Fn(&VM) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.root_view = Some(Arc::new(root_view));
        self
    }

    /// 注册动态窗口集合工厂。
    pub fn windows(
        mut self,
        factory: impl Fn(&VM) -> Vec<WindowSpec<VM>> + Send + Sync + 'static,
    ) -> Self {
        self.windows_factory = Some(Box::new(factory));
        self
    }

    /// 构建运行时并启动应用事件循环。
    pub fn run(self) -> Result<(), TguiError> {
        let (config, view_model, windows, invalidation, animations) = self.into_runtime_parts();

        BoundRuntime::new(config, view_model, windows, invalidation, animations)?.run()
    }

    /// 在 Android 平台上启动应用。
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

    /// 在 OHOS 平台上启动应用。
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

    /// 构建 OHOS 平台处理器。
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
                        msaa: Some(main_config.msaa),
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
