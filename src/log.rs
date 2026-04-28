#[cfg(any(target_os = "android", target_env = "ohos"))]
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

const DEFAULT_TAG: &str = "tgui";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// Cross-platform logging utility.
///
/// Desktop targets write to `stderr`, Android writes to `logcat`, and OHOS
/// writes to `hilog`.
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tag(tag: impl Into<String>) -> Self {
        let tag = tag.into();
        let tag = if tag.trim().is_empty() {
            DEFAULT_TAG.to_string()
        } else {
            tag
        };
        Self { tag: tag.into() }
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn scoped(&self, tag: impl Into<String>) -> Self {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return self.clone();
        }
        Self::with_tag(format!("{}/{}", self.tag(), tag))
    }

    pub fn log(&self, level: LogLevel, message: impl Display) {
        platform::write(level, self.tag(), &message.to_string());
    }

    pub fn trace(&self, message: impl Display) {
        self.log(LogLevel::Trace, message);
    }

    pub fn debug(&self, message: impl Display) {
        self.log(LogLevel::Debug, message);
    }

    pub fn info(&self, message: impl Display) {
        self.log(LogLevel::Info, message);
    }

    pub fn warn(&self, message: impl Display) {
        self.log(LogLevel::Warn, message);
    }

    pub fn error(&self, message: impl Display) {
        self.log(LogLevel::Error, message);
    }
}

pub fn tgui_log(level: LogLevel, message: impl Display) {
    Log::default().log(level, message);
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn sanitize_c_string(input: &str) -> Cow<'_, str> {
    if input.contains('\0') {
        Cow::Owned(input.replace('\0', " "))
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(not(any(target_os = "android", target_env = "ohos")))]
mod platform {
    use std::io::{self, Write};

    use super::LogLevel;

    pub(super) fn write(level: LogLevel, tag: &str, message: &str) {
        let message = message.trim_end_matches('\n');
        let _ = writeln!(io::stderr().lock(), "[{level}] [{tag}] {message}");
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    use super::{sanitize_c_string, LogLevel};

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    pub(super) fn write(level: LogLevel, tag: &str, message: &str) {
        let tag = CString::new(sanitize_c_string(tag).as_ref())
            .expect("Android log tag should not contain interior nulls");
        let message = CString::new(sanitize_c_string(message).as_ref())
            .expect("Android log message should not contain interior nulls");
        unsafe {
            __android_log_write(priority(level), tag.as_ptr(), message.as_ptr());
        }
    }

    fn priority(level: LogLevel) -> c_int {
        match level {
            LogLevel::Trace => 2,
            LogLevel::Debug => 3,
            LogLevel::Info => 4,
            LogLevel::Warn => 5,
            LogLevel::Error => 6,
        }
    }
}

#[cfg(target_env = "ohos")]
mod platform {
    use std::ffi::CString;

    use hilog_sys::{LogLevel as OhosLogLevel, LogType as OhosLogType, OH_LOG_Print};

    use super::{sanitize_c_string, LogLevel};

    const OHOS_APP_DOMAIN: u32 = 0x0000;
    const OHOS_PUBLIC_STRING_FMT: &[u8] = b"%{public}s\0";

    pub(super) fn write(level: LogLevel, tag: &str, message: &str) {
        let tag = CString::new(sanitize_c_string(tag).as_ref())
            .expect("OHOS log tag should not contain interior nulls");
        let message = CString::new(sanitize_c_string(message).as_ref())
            .expect("OHOS log message should not contain interior nulls");
        unsafe {
            OH_LOG_Print(
                OhosLogType::LOG_APP,
                level_to_ohos(level),
                OHOS_APP_DOMAIN,
                tag.as_ptr(),
                OHOS_PUBLIC_STRING_FMT.as_ptr() as *const _,
                message.as_ptr(),
            );
        }
    }

    fn level_to_ohos(level: LogLevel) -> OhosLogLevel {
        match level {
            LogLevel::Trace | LogLevel::Debug => OhosLogLevel::LOG_DEBUG,
            LogLevel::Info => OhosLogLevel::LOG_INFO,
            LogLevel::Warn => OhosLogLevel::LOG_WARN,
            LogLevel::Error => OhosLogLevel::LOG_ERROR,
        }
    }
}
