use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::Arc;

use super::config::{self, LogConfigError};
use super::dispatcher::{logger, LogRecord};

const DEFAULT_TAG: &str = "tgui";

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let level = match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        };
        f.write_str(level)
    }
}

impl FromStr for LogLevel {
    type Err = LogConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(LogConfigError::InvalidLevel(value.to_string())),
        }
    }
}

/// 跨平台日志工具。
///
/// 当前桌面端输出到 `stderr`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    tag: Arc<str>,
}

impl Default for Log {
    fn default() -> Self {
        Self::with_tag(DEFAULT_TAG)
    }
}

impl Log {
    /// 创建默认 tag 的日志句柄。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用指定 tag 创建日志句柄。
    ///
    /// 参数：
    /// - `tag`：日志 tag；为空白时回退到默认 tag。
    ///
    /// 返回值：
    /// - 返回新的 `Log` 实例。
    pub fn with_tag(tag: impl Into<String>) -> Self {
        let tag = tag.into();
        let tag = if tag.trim().is_empty() {
            DEFAULT_TAG.to_string()
        } else {
            tag
        };
        Self { tag: tag.into() }
    }

    /// 返回当前日志 tag。
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// 基于当前 tag 派生子作用域 tag。
    ///
    /// 参数：
    /// - `tag`：要追加的子 tag。
    ///
    /// 返回值：
    /// - 返回新的 `Log` 实例。
    pub fn scoped(&self, tag: impl Into<String>) -> Self {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return self.clone();
        }
        let mut scoped = String::with_capacity(self.tag.len() + 1 + tag.len());
        scoped.push_str(self.tag());
        scoped.push('/');
        scoped.push_str(&tag);
        Self::with_tag(scoped)
    }

    /// 按指定级别输出一条日志。
    ///
    /// 参数：
    /// - `level`：日志级别。
    /// - `message`：日志消息。
    pub fn log(&self, level: LogLevel, message: impl Display) {
        if !config::enabled(level) {
            return;
        }

        let Some(reservation) = logger().reserve(level) else {
            return;
        };

        let record = LogRecord {
            level,
            tag: self.tag.clone(),
            message: message.to_string(),
        };

        if logger().dispatch(reservation, record).is_err() {
            logger().release(reservation);
        }
    }

    /// 输出 `TRACE` 日志。
    pub fn trace(&self, message: impl Display) {
        self.log(LogLevel::Trace, message);
    }

    /// 输出 `DEBUG` 日志。
    pub fn debug(&self, message: impl Display) {
        self.log(LogLevel::Debug, message);
    }

    /// 输出 `INFO` 日志。
    pub fn info(&self, message: impl Display) {
        self.log(LogLevel::Info, message);
    }

    /// 输出 `WARN` 日志。
    pub fn warn(&self, message: impl Display) {
        self.log(LogLevel::Warn, message);
    }

    /// 输出 `ERROR` 日志。
    pub fn error(&self, message: impl Display) {
        self.log(LogLevel::Error, message);
    }
}

/// 使用默认 tag 输出日志。
pub fn tgui_log(level: LogLevel, message: impl Display) {
    Log::default().log(level, message);
}
