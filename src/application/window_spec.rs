use std::sync::Arc;

use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::foundation::event::InputTrigger;
use crate::foundation::view_model::Command;
use crate::platform::dpi::LogicalSize;
use crate::runtime::{WindowBindings, WindowCommand};
use crate::ui::layout::Insets;
use crate::ui::theme::{ThemeMode, ThemeSet};
use crate::ui::unit::Dp;
use crate::ui::widget::{Element, WidgetTree};

use super::config::{logical_window_size, ApplicationConfig, MsaaMode};

pub(crate) type TitleBinding<VM> = Arc<dyn Fn(&VM) -> Signal<String> + Send + Sync>;
pub(crate) type ClearColorBinding<VM> = Arc<dyn Fn(&VM) -> Signal<Color> + Send + Sync>;
pub(crate) type ThemeSetBinding<VM> = Arc<dyn Fn(&VM) -> Signal<ThemeSet> + Send + Sync>;
pub(crate) type ThemeModeBinding<VM> = Arc<dyn Fn(&VM) -> Signal<ThemeMode> + Send + Sync>;
pub(crate) type ReducedMotionBinding<VM> = Arc<dyn Fn(&VM) -> Signal<bool> + Send + Sync>;
pub(crate) type RootViewFactory<VM> = Arc<dyn Fn(&VM) -> Element<VM> + Send + Sync>;
pub(crate) type WindowsFactory<VM> = Box<dyn Fn(&VM) -> Vec<WindowSpec<VM>> + Send + Sync>;

fn build_root_element<VM>(root_view: &RootViewFactory<VM>, view_model: &VM) -> Element<VM> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const ROOT_VIEW_STACK_SIZE: usize = 8 * 1024 * 1024;
        const ROOT_VIEW_STACK_RED_ZONE: usize = ROOT_VIEW_STACK_SIZE;
        stacker::maybe_grow(ROOT_VIEW_STACK_RED_ZONE, ROOT_VIEW_STACK_SIZE, || {
            root_view(view_model)
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        root_view(view_model)
    }
}

/// 表示窗口关闭请求的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowClosePolicy {
    /// 关闭当前原生窗口，并让其余窗口继续运行。
    #[default]
    Close,
}

/// 表示运行时窗口在应用中的角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRole {
    /// 主窗口。
    Main,
    /// 子窗口。
    Child { blocks_main_window: bool },
}

/// 描述一个由运行时管理的窗口规格。
pub struct WindowSpec<VM> {
    pub(crate) key: String,
    pub(crate) role: WindowRole,
    pub(crate) title: Option<String>,
    pub(crate) size: Option<LogicalSize<f64>>,
    pub(crate) min_size: Option<LogicalSize<f64>>,
    pub(crate) max_size: Option<LogicalSize<f64>>,
    pub(crate) decorations: Option<bool>,
    pub(crate) viewport_insets: Option<Insets>,
    pub(crate) msaa: Option<MsaaMode>,
    pub(crate) title_binding: Option<TitleBinding<VM>>,
    pub(crate) clear_color_binding: Option<ClearColorBinding<VM>>,
    pub(crate) theme_set_binding: Option<ThemeSetBinding<VM>>,
    pub(crate) theme_mode_binding: Option<ThemeModeBinding<VM>>,
    pub(crate) reduced_motion_binding: Option<ReducedMotionBinding<VM>>,
    pub(crate) root_view: Option<RootViewFactory<VM>>,
    pub(crate) commands: Vec<WindowCommand<VM>>,
    pub(crate) close_policy: WindowClosePolicy,
}

impl<VM: 'static> WindowSpec<VM> {
    /// 创建一个主窗口规格。
    ///
    /// 参数:
    /// - `key`: 窗口稳定标识。
    ///
    /// 返回值: 主窗口规格对象。
    pub fn new(key: impl Into<String>) -> Self {
        Self::main(key)
    }

    /// 创建主窗口规格。
    ///
    /// 参数:
    /// - `key`: 主窗口稳定标识。
    ///
    /// 返回值: 主窗口规格对象。
    pub fn main(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            role: WindowRole::Main,
            title: None,
            size: None,
            min_size: None,
            max_size: None,
            decorations: None,
            viewport_insets: None,
            msaa: None,
            title_binding: None,
            clear_color_binding: None,
            theme_set_binding: None,
            theme_mode_binding: None,
            reduced_motion_binding: None,
            root_view: None,
            commands: Vec::new(),
            close_policy: WindowClosePolicy::Close,
        }
    }

    /// 创建子窗口规格。
    ///
    /// 参数:
    /// - `key`: 子窗口稳定标识。
    ///
    /// 返回值: 子窗口规格对象；默认不阻塞主窗口。
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
            viewport_insets: None,
            msaa: None,
            title_binding: None,
            clear_color_binding: None,
            theme_set_binding: None,
            theme_mode_binding: None,
            reduced_motion_binding: None,
            root_view: None,
            commands: Vec::new(),
            close_policy: WindowClosePolicy::Close,
        }
    }

    /// 设置窗口标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置窗口逻辑尺寸。
    pub fn window_size(mut self, width: Dp, height: Dp) -> Self {
        self.size = Some(logical_window_size(width, height));
        self
    }

    /// 设置窗口最小逻辑尺寸。
    pub fn min_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.min_size = Some(logical_window_size(width, height));
        self
    }

    /// 设置窗口最大逻辑尺寸。
    pub fn max_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.max_size = Some(logical_window_size(width, height));
        self
    }

    /// 设置是否启用系统窗口装饰。
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = Some(decorations);
        self
    }

    /// 设置当前窗口布局视口的显式内边距。
    ///
    /// 参数:
    /// - `insets`: 从窗口表面四边预留给系统栏或自定义 chrome 的逻辑距离。
    ///
    /// 返回值: 更新后的窗口规格对象。默认不预留任何安全区。
    pub fn viewport_insets(mut self, insets: Insets) -> Self {
        self.viewport_insets = Some(insets);
        self
    }

    /// 设置当前窗口使用的 MSAA 模式。
    pub fn msaa(mut self, mode: MsaaMode) -> Self {
        self.msaa = Some(mode);
        self
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

    /// 绑定 reduced motion 信号。
    pub fn bind_reduced_motion(
        mut self,
        signal: impl Fn(&VM) -> Signal<bool> + Send + Sync + 'static,
    ) -> Self {
        self.reduced_motion_binding = Some(Arc::new(signal));
        self
    }

    /// 注册当前窗口的根视图工厂。
    pub fn root_view(
        mut self,
        root_view: impl Fn(&VM) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.root_view = Some(Arc::new(root_view));
        self
    }

    /// 注册窗口级输入命令。
    pub fn on_input(mut self, trigger: InputTrigger, command: Command<VM>) -> Self {
        self.commands.push(WindowCommand { trigger, command });
        self
    }

    /// 设置窗口关闭策略。
    pub fn close_policy(mut self, close_policy: WindowClosePolicy) -> Self {
        self.close_policy = close_policy;
        self
    }

    /// 设置子窗口是否阻塞主窗口。
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
        if let Some(viewport_insets) = self.viewport_insets {
            config.viewport_insets = viewport_insets;
        }
        if let Some(msaa) = self.msaa {
            config.msaa = msaa;
        }
        config.normalize_size_constraints();
        config
    }

    pub(crate) fn build_window_bindings(&self, view_model: &VM) -> WindowBindings {
        WindowBindings {
            title: self.title_binding.as_ref().map(|signal| signal(view_model)),
            clear_color: self
                .clear_color_binding
                .as_ref()
                .map(|signal| signal(view_model)),
            theme_set: self
                .theme_set_binding
                .as_ref()
                .map(|signal| signal(view_model)),
            theme_mode: self
                .theme_mode_binding
                .as_ref()
                .map(|signal| signal(view_model)),
            reduced_motion: self
                .reduced_motion_binding
                .as_ref()
                .map(|signal| signal(view_model)),
        }
    }

    pub(crate) fn build_widget_tree(&self, view_model: &VM) -> Option<WidgetTree<VM>> {
        self.root_view
            .as_ref()
            .map(|root_view| WidgetTree::new(build_root_element(root_view, view_model)))
    }
}
