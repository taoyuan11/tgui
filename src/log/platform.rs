#[cfg(any(target_os = "android", target_env = "ohos"))]
use std::borrow::Cow;

use super::api::LogLevel;

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn sanitize_c_string(input: &str) -> Cow<'_, str> {
    if input.contains('\0') {
        Cow::Owned(input.replace('\0', " "))
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(not(any(target_os = "android", target_env = "ohos")))]
mod imp {
    use std::io::{self, Write};

    use super::LogLevel;

    #[cfg(test)]
    pub(super) fn format_line(_level: LogLevel, _tag: &str, line: &str) -> String {
        line.trim_end_matches('\n').to_string()
    }

    pub(super) fn write(_level: LogLevel, _tag: &str, line: &str) {
        #[cfg(test)]
        let line = format_line(_level, _tag, line);
        #[cfg(not(test))]
        let line = line.trim_end_matches('\n');
        let _ = writeln!(io::stderr().lock(), "{line}");
    }
}

#[cfg(target_os = "android")]
mod imp {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    use super::{sanitize_c_string, LogLevel};

    #[link(name = "log")]
    unsafe extern "C" {
        // SAFETY: 由 Android NDK liblog 提供的稳定 C 接口；签名按 ndk
        // `<android/log.h>` 中的 `__android_log_write` 复刻，调用方需保证 `tag`
        // 和 `text` 指向以 NUL 结尾的合法 UTF-8 字符串（在 `write` 中通过
        // `CString` 满足）。
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    pub(super) fn write(level: LogLevel, tag: &str, line: &str) {
        let tag = CString::new(sanitize_c_string(tag).as_ref())
            .expect("Android log tag should not contain interior nulls");
        let message = CString::new(sanitize_c_string(line).as_ref())
            .expect("Android log message should not contain interior nulls");
        // SAFETY: `tag.as_ptr()` 和 `message.as_ptr()` 指向上面 `CString` 拥有的
        // 以 NUL 结尾的 UTF-8 缓冲区，并在调用期间保持有效；`priority(level)`
        // 取值为 ndk liblog 文档允许的 ANDROID_LOG_* 常量。
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
mod imp {
    use std::ffi::CString;

    use hilog_sys::{LogLevel as OhosLogLevel, LogType as OhosLogType, OH_LOG_Print};

    use super::{sanitize_c_string, LogLevel};

    const OHOS_APP_DOMAIN: u32 = 0x0000;
    const OHOS_PUBLIC_STRING_FMT: &[u8] = b"%{public}s\0";

    pub(super) fn write(level: LogLevel, tag: &str, line: &str) {
        let tag = CString::new(sanitize_c_string(tag).as_ref())
            .expect("OHOS log tag should not contain interior nulls");
        let message = CString::new(sanitize_c_string(line).as_ref())
            .expect("OHOS log message should not contain interior nulls");
        // SAFETY: `tag.as_ptr()` 和 `message.as_ptr()` 在调用期间持有 `CString`
        // 拥有的、以 NUL 结尾的 UTF-8 缓冲区；`OHOS_PUBLIC_STRING_FMT` 是常量
        // `&[u8]`，本身就是 `'static` 的，转 `*const c_char` 后地址仍然有效。
        // `OH_LOG_Print` 是 OHOS hilog 提供的可重入 C API，调用线程任意。
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

pub(super) fn write(level: LogLevel, tag: &str, line: &str) {
    imp::write(level, tag, line);
}

#[cfg(test)]
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
pub(super) fn format_line(level: LogLevel, tag: &str, line: &str) -> String {
    imp::format_line(level, tag, line)
}
