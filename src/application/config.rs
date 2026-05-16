use super::builder::ApplicationBuilder;
use crate::foundation::color::Color;
use crate::foundation::view_model::ViewModel;
use crate::platform::dpi::LogicalSize;
use crate::text::font::FontCatalog;
use crate::ui::theme::{Theme, ThemeMode, ThemeSet};
use crate::ui::unit::Dp;

pub(crate) fn logical_window_size(width: Dp, height: Dp) -> LogicalSize<f64> {
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
}

impl ThemeSelection {
    pub(crate) fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::System => Self::System,
            ThemeMode::Light | ThemeMode::Dark => Self::Mode(mode),
        }
    }
}

/// 表示渲染器使用的多重采样抗锯齿模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MsaaMode {
    /// 关闭多重采样。
    Off,
    /// 自动选择当前平台支持的最佳模式。
    #[default]
    Auto,
    /// 使用 2x MSAA。
    X2,
    /// 使用 4x MSAA。
    X4,
    /// 使用 8x MSAA。
    X8,
}

/// 表示一个 `tgui` 应用的全局启动配置。
#[derive(Debug, Clone)]
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
    msaa: MsaaMode,
    fonts: FontCatalog,
    theme: ThemeSelection,
    theme_set: ThemeSet,
    window_icon: Option<&'static [u8]>,
}

impl Application {
    /// 创建应用配置对象。
    ///
    /// 返回值: 使用默认标题、默认窗口大小和默认主题的应用配置。
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
            msaa: MsaaMode::Auto,
            fonts: FontCatalog::default(),
            theme: ThemeSelection::System,
            theme_set: ThemeSet::default(),
            window_icon: None,
        }
    }

    /// 设置应用标识。
    ///
    /// 参数:
    /// - `app_id`: 供通知等平台服务使用的稳定应用标识。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// 设置初始窗口标题。
    ///
    /// 参数:
    /// - `title`: 主窗口初始标题文本。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置初始窗口逻辑尺寸。
    ///
    /// 参数:
    /// - `width`: 窗口逻辑宽度。
    /// - `height`: 窗口逻辑高度。
    ///
    /// 返回值: 更新后的应用配置对象；尺寸会被限制为至少 `1x1`。
    pub fn window_size(mut self, width: Dp, height: Dp) -> Self {
        self.width = width.max(Dp::new(1.0));
        self.height = height.max(Dp::new(1.0));
        self
    }

    /// 设置最小可调整窗口尺寸。
    ///
    /// 参数:
    /// - `width`: 最小逻辑宽度。
    /// - `height`: 最小逻辑高度。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn min_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.min_size = Some(logical_window_size(width, height));
        self
    }

    /// 设置最大可调整窗口尺寸。
    ///
    /// 参数:
    /// - `width`: 最大逻辑宽度。
    /// - `height`: 最大逻辑高度。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn max_window_size(mut self, width: Dp, height: Dp) -> Self {
        self.max_size = Some(logical_window_size(width, height));
        self
    }

    /// 覆盖渲染器的默认清屏颜色。
    ///
    /// 参数:
    /// - `clear_color`: 窗口清屏颜色。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn clear_color(mut self, clear_color: Color) -> Self {
        self.clear_color = clear_color;
        self.clear_color_overridden = true;
        self
    }

    /// 设置关闭主窗口时是否同时关闭所有子窗口。
    ///
    /// 参数:
    /// - `close_children_with_main`: `true` 表示主窗口关闭时同步关闭子窗口。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn close_children_with_main(mut self, close_children_with_main: bool) -> Self {
        self.close_children_with_main = close_children_with_main;
        self
    }

    /// 设置是否启用系统窗口装饰。
    ///
    /// 参数:
    /// - `decorations`: `false` 时可使用自绘标题栏。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    /// 设置渲染器使用的 MSAA 模式。
    ///
    /// 参数:
    /// - `mode`: 目标多重采样模式。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn msaa(mut self, mode: MsaaMode) -> Self {
        self.msaa = mode;
        self
    }

    /// 设置窗口图标资源。
    ///
    /// 参数:
    /// - `icon`: 静态字节形式的图标数据。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn window_icon(mut self, icon: &'static [u8]) -> Self {
        self.window_icon = Some(icon);
        self
    }

    /// 注册一份内存中的字体数据。
    ///
    /// 参数:
    /// - `name`: 逻辑字体族名称。
    /// - `bytes`: 字体二进制内容。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn font_bytes(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
        self.fonts.register_font(name, bytes);
        self
    }

    /// 注册一份磁盘字体文件。
    ///
    /// 参数:
    /// - `name`: 逻辑字体族名称。
    /// - `path`: 字体文件路径。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn font_file(
        mut self,
        name: impl Into<String>,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        self.fonts.register_font_file(name, path);
        self
    }

    /// 设置默认字体族。
    ///
    /// 参数:
    /// - `name`: 默认字体族名称。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn default_font(mut self, name: impl Into<String>) -> Self {
        self.fonts.set_default_font(name);
        self
    }

    /// 设置固定主题模式。
    ///
    /// 参数:
    /// - `mode`: 要使用的主题模式。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn theme_mode(mut self, mode: ThemeMode) -> Self {
        if !self.clear_color_overridden {
            self.clear_color = self.theme_set.resolve(mode, None).colors.background;
        }
        self.theme = ThemeSelection::from_mode(mode);
        self
    }

    /// 设置主题集合。
    ///
    /// 参数:
    /// - `theme_set`: 包含浅色和深色主题的集合。
    ///
    /// 返回值: 更新后的应用配置对象。
    pub fn theme_set(mut self, theme_set: ThemeSet) -> Self {
        if !self.clear_color_overridden {
            let mode = match self.theme {
                ThemeSelection::Mode(mode) => mode,
                ThemeSelection::System => ThemeMode::Dark,
            };
            self.clear_color = theme_set.resolve(mode, None).colors.background;
        }
        self.theme_set = theme_set;
        self
    }

    /// 进入基于 ViewModel 的应用构建流程。
    ///
    /// 参数:
    /// - `factory`: 用于创建 ViewModel 的工厂函数。
    ///
    /// 返回值: 可继续配置窗口绑定和启动参数的应用构建器。
    pub fn with_view_model<VM, F>(self, factory: F) -> ApplicationBuilder<VM, F>
    where
        VM: ViewModel,
        F: FnOnce(&crate::foundation::binding::ViewModelContext) -> VM,
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
            msaa: self.msaa,
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
    pub(crate) msaa: MsaaMode,
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
