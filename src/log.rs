#[cfg(any(target_os = "android", target_env = "ohos"))]
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};

const DEFAULT_TAG: &str = "tgui";
const HIGH_PRIORITY_QUEUE_CAPACITY: usize = 256;
const LOW_PRIORITY_QUEUE_CAPACITY: usize = 1024;
const TEXT_PROFILE_ENV: &str = "TGUI_TEXT_PROFILE";
const TEXT_PROFILE_MIN_MS_ENV: &str = "TGUI_TEXT_PROFILE_MIN_MS";
const TEXT_PROFILE_LABELS: &[&str] = &[
    "textarea_about_to_wait",
    "textarea_animation",
    "textarea_animation_keys",
    "textarea_computed_scene",
    "textarea_flush_pending",
    "textarea_flush_session",
    "textarea_input_edit",
    "textarea_invalidate_scene",
    "textarea_invalidation",
    "textarea_keyboard",
    "textarea_patch_layout",
    "textarea_patch_scene",
    "textarea_patch_scene_collect",
    "textarea_patch_scene_collect_root",
    "textarea_patch_scene_focus_override",
    "textarea_patch_scene_layout_overrides",
    "textarea_patch_scene_recompose",
    "textarea_patch_scene_resolve_roots",
    "textarea_patch_scene_root_clone",
    "textarea_redraw",
    "textarea_render",
    "textarea_text_widget",
    "textarea_theme_sync",
];

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
        let mut scoped = String::with_capacity(self.tag.len() + 1 + tag.len());
        scoped.push_str(self.tag());
        scoped.push('/');
        scoped.push_str(&tag);
        Self::with_tag(scoped)
    }

    pub fn log(&self, level: LogLevel, message: impl Display) {
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

pub(crate) fn text_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var(TEXT_PROFILE_ENV)
            .map(|value| {
                let value = value.trim();
                matches!(value, "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
            })
            .unwrap_or(false)
    })
}

fn text_profile_min_duration() -> Duration {
    static MIN_DURATION: OnceLock<Duration> = OnceLock::new();
    *MIN_DURATION.get_or_init(|| {
        std::env::var(TEXT_PROFILE_MIN_MS_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| Duration::from_secs_f64(value / 1000.0))
            .unwrap_or(Duration::ZERO)
    })
}

fn text_profile_label_enabled(label: &str) -> bool {
    TEXT_PROFILE_LABELS.contains(&label)
}

pub(crate) fn log_text_profile(label: &str, duration: Duration, message: impl Display) {
    if !text_profile_enabled()
        || !text_profile_label_enabled(label)
        || duration < text_profile_min_duration()
    {
        return;
    }

    Log::with_tag("tgui-text-prof").debug(format_args!(
        "{label} took {:.3}ms {message}",
        duration.as_secs_f64() * 1000.0
    ));
}

#[derive(Debug)]
struct LogRecord {
    level: LogLevel,
    tag: Arc<str>,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueKind {
    High,
    Low,
}

#[derive(Debug)]
struct QueueSlots {
    len: AtomicUsize,
    capacity: usize,
}

impl QueueSlots {
    fn new(capacity: usize) -> Self {
        Self {
            len: AtomicUsize::new(0),
            capacity,
        }
    }

    fn try_reserve(&self) -> bool {
        let mut current = self.len.load(Ordering::Relaxed);
        loop {
            if current >= self.capacity {
                return false;
            }

            match self.len.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }

    fn release(&self) {
        self.len.fetch_sub(1, Ordering::Release);
    }
}

#[derive(Debug)]
struct LogDispatcher {
    high_tx: Sender<LogRecord>,
    low_tx: Sender<LogRecord>,
    high_rx: Receiver<LogRecord>,
    low_rx: Receiver<LogRecord>,
    high_slots: Arc<QueueSlots>,
    low_slots: Arc<QueueSlots>,
}

impl LogDispatcher {
    fn new() -> Self {
        let (high_tx, high_rx) = unbounded();
        let (low_tx, low_rx) = unbounded();
        let high_slots = Arc::new(QueueSlots::new(HIGH_PRIORITY_QUEUE_CAPACITY));
        let low_slots = Arc::new(QueueSlots::new(LOW_PRIORITY_QUEUE_CAPACITY));
        let dispatcher = Self {
            high_tx,
            low_tx,
            high_rx,
            low_rx,
            high_slots,
            low_slots,
        };
        dispatcher.spawn_worker();
        dispatcher
    }

    fn spawn_worker(&self) {
        let high_rx = self.high_rx.clone();
        let low_rx = self.low_rx.clone();
        let high_slots = self.high_slots.clone();
        let low_slots = self.low_slots.clone();

        thread::Builder::new()
            .name("tgui-log".to_string())
            .spawn(move || worker_loop(high_rx, low_rx, high_slots, low_slots))
            .expect("failed to spawn tgui log worker");
    }

    fn reserve(&self, level: LogLevel) -> Option<QueueKind> {
        match level {
            LogLevel::Warn | LogLevel::Error => {
                if self.high_slots.try_reserve() {
                    Some(QueueKind::High)
                } else if self.low_slots.try_reserve() {
                    Some(QueueKind::Low)
                } else {
                    None
                }
            }
            LogLevel::Trace | LogLevel::Debug | LogLevel::Info => {
                if self.low_slots.try_reserve() {
                    Some(QueueKind::Low)
                } else {
                    None
                }
            }
        }
    }

    fn dispatch(&self, reservation: QueueKind, record: LogRecord) -> Result<(), LogRecord> {
        let send_result = match reservation {
            QueueKind::High => self.high_tx.send(record),
            QueueKind::Low => self.low_tx.send(record),
        };

        send_result.map_err(|error| error.0)
    }

    fn release(&self, reservation: QueueKind) {
        match reservation {
            QueueKind::High => self.high_slots.release(),
            QueueKind::Low => self.low_slots.release(),
        }
    }

    #[cfg(test)]
    fn try_drain_one(&self) -> Option<LogRecord> {
        if let Ok(record) = self.high_rx.try_recv() {
            self.high_slots.release();
            return Some(record);
        }

        if let Ok(record) = self.low_rx.try_recv() {
            self.low_slots.release();
            return Some(record);
        }

        None
    }
}

fn worker_loop(
    high_rx: Receiver<LogRecord>,
    low_rx: Receiver<LogRecord>,
    high_slots: Arc<QueueSlots>,
    low_slots: Arc<QueueSlots>,
) {
    loop {
        select_biased! {
            recv(high_rx) -> record => match record {
                Ok(record) => {
                    emit_record(record);
                    high_slots.release();
                }
                Err(_) => return,
            },
            recv(low_rx) -> record => match record {
                Ok(record) => {
                    emit_record(record);
                    low_slots.release();
                }
                Err(_) => return,
            },
        }
    }
}

fn emit_record(record: LogRecord) {
    platform::write(record.level, &record.tag, &record.message);
}

fn logger() -> &'static LogDispatcher {
    static LOGGER: OnceLock<LogDispatcher> = OnceLock::new();
    LOGGER.get_or_init(LogDispatcher::new)
}

#[cfg(test)]
impl LogDispatcher {
    fn new_for_test(high_capacity: usize, low_capacity: usize) -> Self {
        let (high_tx, high_rx) = unbounded();
        let (low_tx, low_rx) = unbounded();
        Self {
            high_tx,
            low_tx,
            high_rx,
            low_rx,
            high_slots: Arc::new(QueueSlots::new(high_capacity)),
            low_slots: Arc::new(QueueSlots::new(low_capacity)),
        }
    }
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

    pub(super) fn format_line(level: LogLevel, tag: &str, message: &str) -> String {
        let message = message.trim_end_matches('\n');
        format!("[{level}] [{tag}] {message}")
    }

    pub(super) fn write(level: LogLevel, tag: &str, message: &str) {
        let line = format_line(level, tag, message);
        let _ = writeln!(io::stderr().lock(), "{line}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_formats_as_expected() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn scoped_tags_are_joined_without_trimming() {
        let log = Log::with_tag("root").scoped(" child ");
        assert_eq!(log.tag(), "root/ child ");
    }

    #[test]
    fn low_priority_logs_drop_when_queue_is_full() {
        let dispatcher = LogDispatcher::new_for_test(1, 1);

        assert!(dispatcher.reserve(LogLevel::Info).is_some());
        assert!(dispatcher
            .dispatch(
                QueueKind::Low,
                LogRecord {
                    level: LogLevel::Info,
                    tag: Arc::from("tgui"),
                    message: "first".to_string(),
                }
            )
            .is_ok());

        assert_eq!(dispatcher.reserve(LogLevel::Info), None);

        let record = dispatcher
            .try_drain_one()
            .expect("expected one queued record");
        assert_eq!(record.message, "first");

        assert!(dispatcher.reserve(LogLevel::Info).is_some());
    }

    #[test]
    fn warn_logs_use_high_queue_before_low_overflow() {
        let dispatcher = LogDispatcher::new_for_test(1, 2);

        assert_eq!(dispatcher.reserve(LogLevel::Warn), Some(QueueKind::High));
        assert!(dispatcher
            .dispatch(
                QueueKind::High,
                LogRecord {
                    level: LogLevel::Warn,
                    tag: Arc::from("tgui"),
                    message: "high".to_string(),
                }
            )
            .is_ok());

        assert_eq!(dispatcher.reserve(LogLevel::Info), Some(QueueKind::Low));
        assert!(dispatcher
            .dispatch(
                QueueKind::Low,
                LogRecord {
                    level: LogLevel::Info,
                    tag: Arc::from("tgui"),
                    message: "low".to_string(),
                }
            )
            .is_ok());

        assert_eq!(dispatcher.reserve(LogLevel::Warn), Some(QueueKind::Low));
        assert!(dispatcher
            .dispatch(
                QueueKind::Low,
                LogRecord {
                    level: LogLevel::Warn,
                    tag: Arc::from("tgui"),
                    message: "overflow".to_string(),
                }
            )
            .is_ok());

        assert_eq!(
            dispatcher.try_drain_one().map(|record| record.message),
            Some("high".to_string())
        );
        assert_eq!(
            dispatcher.try_drain_one().map(|record| record.message),
            Some("low".to_string())
        );
        assert_eq!(
            dispatcher.try_drain_one().map(|record| record.message),
            Some("overflow".to_string())
        );
    }

    #[test]
    fn desktop_format_line_keeps_existing_shape() {
        assert_eq!(
            platform::format_line(LogLevel::Info, "tgui", "hello\n"),
            "[INFO] [tgui] hello"
        );
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
