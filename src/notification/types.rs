use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_ACTIONS: usize = 2;

static NEXT_NOTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

/// 表示通知子系统返回的错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    /// 当前平台尚未实现通知能力。
    UnsupportedPlatform,
    /// 调用方传入的通知参数不合法。
    InvalidOptions(String),
    /// 平台后端或运行时桥接过程中返回的错误。
    Backend(String),
}

impl Display for NotificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                write!(f, "notifications are not supported on this platform")
            }
            Self::InvalidOptions(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl Error for NotificationError {}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for NotificationError {
    fn from(err: jni::errors::Error) -> Self {
        NotificationError::Backend(format!("jni error: {err}"))
    }
}

/// 表示当前平台的通知权限状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPermission {
    /// 已授予通知权限。
    Granted,
    /// 已明确拒绝通知权限。
    Denied,
    /// 平台尚未决定，或需要显式请求。
    NotDetermined,
}

/// 表示交互式通知中的一个动作按钮。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    id: String,
    label: String,
}

impl NotificationAction {
    /// 创建一个通知动作。
    ///
    /// 参数:
    /// - `id`: 动作的唯一标识，会在回调中原样返回。
    /// - `label`: 系统通知中展示给用户的按钮文本。
    ///
    /// 返回值: 构造好的通知动作。
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    /// 返回动作唯一标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回动作显示文本。
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// 描述用户点击交互式通知动作后回传的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationActionEvent {
    /// 触发动作的通知 ID。
    pub notification_id: String,
    /// 被点击的动作 ID。
    pub action_id: String,
}

/// 描述一次系统通知的展示参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationOptions {
    id: Option<String>,
    title: String,
    body: Option<String>,
    subtitle: Option<String>,
    app_name: Option<String>,
    icon: Option<String>,
    sound: bool,
    actions: Vec<NotificationAction>,
}

impl NotificationOptions {
    /// 创建通知参数。
    ///
    /// 参数:
    /// - `title`: 通知标题，不能为空白字符串。
    ///
    /// 返回值: 带默认配置的通知参数对象。
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            title: title.into(),
            body: None,
            subtitle: None,
            app_name: None,
            icon: None,
            sound: true,
            actions: Vec::new(),
        }
    }

    /// 设置通知 ID。
    ///
    /// 参数:
    /// - `id`: 通知唯一标识；若未设置，会在发送前自动生成。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// 设置通知正文。
    ///
    /// 参数:
    /// - `body`: 通知正文文本。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// 设置通知副标题。
    ///
    /// 参数:
    /// - `subtitle`: 通知副标题文本。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// 设置应用显示名称。
    ///
    /// 参数:
    /// - `app_name`: 平台通知中心中展示的应用名称。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    /// 设置通知图标名称或资源标识。
    ///
    /// 参数:
    /// - `icon`: 平台后端可识别的图标名称。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置是否启用提示音。
    ///
    /// 参数:
    /// - `sound`: `true` 表示启用提示音，`false` 表示静音。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn sound(mut self, sound: bool) -> Self {
        self.sound = sound;
        self
    }

    /// 追加一个交互动作。
    ///
    /// 参数:
    /// - `action`: 要追加的通知动作。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    /// 追加多个交互动作。
    ///
    /// 参数:
    /// - `actions`: 要追加的动作集合。
    ///
    /// 返回值: 更新后的通知参数对象。
    pub fn actions(mut self, actions: impl IntoIterator<Item = NotificationAction>) -> Self {
        self.actions.extend(actions);
        self
    }

    /// 返回通知 ID。
    ///
    /// 返回值: 若调用方显式设置了 ID，则返回该 ID；否则返回 `None`。
    pub fn notification_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// 返回通知标题。
    pub fn title(&self) -> &str {
        &self.title
    }

    /// 返回通知正文。
    pub fn body_text(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// 返回通知副标题。
    pub fn subtitle_text(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    /// 返回通知中展示的应用名称。
    pub fn app_name_text(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    /// 返回通知图标名称或资源标识。
    pub fn icon_name(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// 返回当前是否启用提示音。
    pub fn sound_enabled(&self) -> bool {
        self.sound
    }

    /// 返回所有交互动作。
    pub fn action_items(&self) -> &[NotificationAction] {
        &self.actions
    }

    pub(super) fn ensure_id(&mut self) -> String {
        if let Some(id) = self.id.as_ref() {
            return id.clone();
        }

        let id = format!(
            "tgui-notification-{}",
            NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::Relaxed)
        );
        self.id = Some(id.clone());
        id
    }

    pub(super) fn validate(&self, require_actions: bool) -> Result<(), NotificationError> {
        if self.title.trim().is_empty() {
            return Err(NotificationError::InvalidOptions(
                "notification title cannot be empty".to_string(),
            ));
        }

        if require_actions && self.actions.is_empty() {
            return Err(NotificationError::InvalidOptions(
                "interactive notifications require at least one action".to_string(),
            ));
        }

        if self.actions.len() > MAX_ACTIONS {
            return Err(NotificationError::InvalidOptions(format!(
                "notifications support at most {MAX_ACTIONS} actions"
            )));
        }

        for action in &self.actions {
            if action.id.trim().is_empty() {
                return Err(NotificationError::InvalidOptions(
                    "notification action id cannot be empty".to_string(),
                ));
            }
            if action.label.trim().is_empty() {
                return Err(NotificationError::InvalidOptions(
                    "notification action label cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}
