use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

/// 表示对话框子系统返回的错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogError {
    /// 当前平台尚未实现原生对话框能力。
    UnsupportedPlatform,
    /// 原生后端或运行时桥接过程中返回的错误。
    Backend(String),
}

impl Display for DialogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => write!(f, "dialogs are not supported on this platform"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl Error for DialogError {}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for DialogError {
    fn from(err: jni::errors::Error) -> Self {
        DialogError::Backend(format!("jni error: {err}"))
    }
}

/// 描述文件对话框的显示配置。
#[derive(Debug, Clone, Default)]
pub struct FileDialogOptions {
    pub(crate) title: Option<String>,
    pub(crate) directory: Option<PathBuf>,
    pub(crate) file_name: Option<String>,
    pub(crate) filters: Vec<FileDialogFilter>,
    pub(crate) can_create_directories: Option<bool>,
}

impl FileDialogOptions {
    /// 创建一组默认的文件对话框参数。
    ///
    /// 返回值: 空配置的文件对话框参数对象。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置对话框标题。
    ///
    /// 参数:
    /// - `title`: 原生文件对话框中显示的标题文本。
    ///
    /// 返回值: 更新后的文件对话框参数对象。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置初始目录。
    ///
    /// 参数:
    /// - `directory`: 对话框打开时默认显示的目录路径。
    ///
    /// 返回值: 更新后的文件对话框参数对象。
    pub fn directory<P: AsRef<Path>>(mut self, directory: P) -> Self {
        self.directory = Some(directory.as_ref().to_path_buf());
        self
    }

    /// 设置默认文件名。
    ///
    /// 参数:
    /// - `file_name`: 文件选择器中预填充的文件名。
    ///
    /// 返回值: 更新后的文件对话框参数对象。
    pub fn file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// 添加一个扩展名过滤器。
    ///
    /// 参数:
    /// - `name`: 过滤器名称。
    /// - `extensions`: 允许显示的文件扩展名集合，不包含前导点也可使用。
    ///
    /// 返回值: 更新后的文件对话框参数对象。
    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        self.filters.push(FileDialogFilter {
            name: name.into(),
            extensions: extensions.iter().map(|ext| ext.to_string()).collect(),
        });
        self
    }

    /// 设置是否允许在对话框中创建目录。
    ///
    /// 参数:
    /// - `can_create_directories`: `true` 表示允许创建目录。
    ///
    /// 返回值: 更新后的文件对话框参数对象。
    pub fn can_create_directories(mut self, can_create_directories: bool) -> Self {
        self.can_create_directories = Some(can_create_directories);
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FileDialogFilter {
    pub(crate) name: String,
    pub(crate) extensions: Vec<String>,
}

/// 表示消息对话框的视觉级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogLevel {
    /// 普通信息提示。
    #[default]
    Info,
    /// 警告提示。
    Warning,
    /// 错误提示。
    Error,
}

/// 表示消息对话框的按钮布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogButtons {
    /// 仅展示确认按钮。
    #[default]
    Ok,
    /// 展示确认与取消按钮。
    OkCancel,
    /// 展示是与否按钮。
    YesNo,
    /// 展示是、否与取消按钮。
    YesNoCancel,
}

/// 表示消息对话框关闭后返回的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogResult {
    /// 用户选择“是”。
    Yes,
    /// 用户选择“否”。
    No,
    /// 用户选择“确定”。
    Ok,
    /// 用户取消或平台返回自定义关闭结果。
    #[default]
    Cancel,
}

/// 描述消息对话框的显示配置。
#[derive(Debug, Clone, Default)]
pub struct MessageDialogOptions {
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) level: MessageDialogLevel,
    pub(crate) buttons: MessageDialogButtons,
}

impl MessageDialogOptions {
    /// 创建一组默认的消息对话框参数。
    ///
    /// 返回值: 空配置的消息对话框参数对象。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置对话框标题。
    ///
    /// 参数:
    /// - `title`: 消息对话框标题文本。
    ///
    /// 返回值: 更新后的消息对话框参数对象。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置对话框描述文本。
    ///
    /// 参数:
    /// - `description`: 消息正文内容。
    ///
    /// 返回值: 更新后的消息对话框参数对象。
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置消息级别。
    ///
    /// 参数:
    /// - `level`: 对话框显示级别。
    ///
    /// 返回值: 更新后的消息对话框参数对象。
    pub fn level(mut self, level: MessageDialogLevel) -> Self {
        self.level = level;
        self
    }

    /// 设置按钮布局。
    ///
    /// 参数:
    /// - `buttons`: 对话框按钮组合。
    ///
    /// 返回值: 更新后的消息对话框参数对象。
    pub fn buttons(mut self, buttons: MessageDialogButtons) -> Self {
        self.buttons = buttons;
        self
    }
}
