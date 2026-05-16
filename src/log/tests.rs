use std::sync::Arc;

use super::api::{Log, LogLevel};
use super::dispatcher::{LogDispatcher, LogRecord, QueueKind};
use super::platform;

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
    #[cfg(not(any(target_os = "android", target_env = "ohos")))]
    assert_eq!(
        platform::format_line(LogLevel::Info, "tgui", "hello\n"),
        "[INFO] [tgui] hello"
    );
}
