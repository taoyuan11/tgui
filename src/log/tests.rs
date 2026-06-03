use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};

use super::api::{Log, LogLevel};
use super::config::{
    configure_logging, configure_logging_from_manifest, reset_for_test, LogConfig, LogConfigError,
    LogFileConfig,
};
use super::dispatcher::{format_record, format_timestamp, LogDispatcher, LogRecord, QueueKind};
use super::file;
use super::platform;

fn log_config_test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn log_level_formats_as_expected() {
    assert_eq!(LogLevel::Trace.to_string(), "TRACE");
    assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    assert_eq!(LogLevel::Info.to_string(), "INFO");
    assert_eq!(LogLevel::Warn.to_string(), "WARN");
    assert_eq!(LogLevel::Error.to_string(), "ERROR");
}

#[test]
fn log_level_parses_and_sorts_as_expected() {
    assert_eq!("trace".parse::<LogLevel>(), Ok(LogLevel::Trace));
    assert_eq!("DEBUG".parse::<LogLevel>(), Ok(LogLevel::Debug));
    assert_eq!("warning".parse::<LogLevel>(), Ok(LogLevel::Warn));
    assert_eq!(
        "verbose".parse::<LogLevel>(),
        Err(LogConfigError::InvalidLevel("verbose".to_string()))
    );
    assert!(LogLevel::Error > LogLevel::Warn);
    assert!(LogLevel::Debug > LogLevel::Trace);
}

#[test]
fn scoped_tags_are_joined_without_trimming() {
    let log = Log::with_tag("root").scoped(" child ");
    assert_eq!(log.tag(), "root/ child ");
}

#[test]
fn log_level_filter_drops_records_before_queueing() {
    let _guard = log_config_test_guard();
    reset_for_test();
    configure_logging(LogConfig {
        level: LogLevel::Warn,
        file: None,
    })
    .unwrap();

    let dispatcher = LogDispatcher::new_for_test(1, 1);
    assert_eq!(dispatcher.reserve_if_enabled(LogLevel::Info), None);
    assert_eq!(
        dispatcher.reserve_if_enabled(LogLevel::Error),
        Some(QueueKind::High)
    );

    reset_for_test();
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
fn log_desktop_format_line_keeps_formatted_record_shape() {
    #[cfg(not(any(target_os = "android", target_env = "ohos")))]
    assert_eq!(
        platform::format_line(
            LogLevel::Info,
            "tgui",
            "[2026-06-03 12:34:56.789 +08:00] [INFO] [tgui] hello\n"
        ),
        "[2026-06-03 12:34:56.789 +08:00] [INFO] [tgui] hello"
    );
}

#[test]
fn log_timestamp_formats_with_millisecond_precision_and_offset() {
    let timestamp = Local.with_ymd_and_hms(2026, 6, 3, 12, 34, 56).unwrap()
        + chrono::Duration::milliseconds(789);
    let formatted = format_timestamp(timestamp);
    assert_eq!(formatted.len(), "2026-06-03 12:34:56.789 +08:00".len());
    assert_eq!(&formatted[0..23], "2026-06-03 12:34:56.789");
    assert_eq!(formatted.as_bytes()[23], b' ');
    assert!(formatted[24..].starts_with('+') || formatted[24..].starts_with('-'));
    assert_eq!(formatted.as_bytes()[27], b':');
}

#[test]
fn log_record_format_includes_timestamp_level_tag_and_message() {
    let record = LogRecord {
        level: LogLevel::Info,
        tag: Arc::from("tgui"),
        message: "hello\n".to_string(),
    };
    let formatted = format_record(&record);
    assert!(formatted.contains("] [INFO] [tgui] hello"));
    assert!(!formatted.ends_with('\n'));
}

#[test]
fn log_manifest_config_uses_defaults_without_logging_table() {
    let _guard = log_config_test_guard();
    reset_for_test();
    configure_logging_from_manifest("[package]\nname = \"app\"\n", "/tmp/app").unwrap();
    assert_eq!(
        super::config::current_config(),
        LogConfig {
            level: LogLevel::Trace,
            file: None,
        }
    );
    reset_for_test();
}

#[test]
fn log_manifest_config_resolves_relative_log_dir_and_defaults() {
    let _guard = log_config_test_guard();
    reset_for_test();
    configure_logging_from_manifest(
        r#"
            [package]
            name = "app"

            [package.metadata.tgui.logging]
            level = "debug"
            log_dir = "logs"
        "#,
        "/tmp/app",
    )
    .unwrap();

    assert_eq!(
        super::config::current_config(),
        LogConfig {
            level: LogLevel::Debug,
            file: Some(LogFileConfig {
                log_dir: "/tmp/app/logs".into(),
                file_name: "tgui.log".to_string(),
                max_file_size_bytes: 10 * 1024 * 1024,
                max_files: 5,
            }),
        }
    );
    reset_for_test();
}

#[test]
fn log_manifest_config_rejects_invalid_level_and_rotation_values() {
    let _guard = log_config_test_guard();
    let invalid_level = configure_logging_from_manifest(
        r#"
            [package.metadata.tgui.logging]
            level = "chatty"
        "#,
        "/tmp/app",
    );
    assert!(matches!(
        invalid_level,
        Err(LogConfigError::InvalidLevel(_))
    ));

    let invalid_rotation = configure_logging_from_manifest(
        r#"
            [package.metadata.tgui.logging]
            log_dir = "logs"
            max_files = 0
        "#,
        "/tmp/app",
    );
    assert!(matches!(
        invalid_rotation,
        Err(LogConfigError::InvalidValue("max_files", _))
    ));
}

#[test]
fn log_file_sink_writes_and_rotates() {
    let dir = std::env::temp_dir().join(format!(
        "tgui-log-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = LogFileConfig {
        log_dir: dir.clone(),
        file_name: "tgui.log".to_string(),
        max_file_size_bytes: 12,
        max_files: 3,
    };

    file::write_lines_for_test(config, &["first-line", "second-line", "third-line"]).unwrap();

    let current = std::fs::read_to_string(dir.join("tgui.log")).unwrap();
    let rotated = std::fs::read_to_string(dir.join("tgui.log.1")).unwrap();
    assert!(current.contains("third-line"));
    assert!(rotated.contains("second-line"));

    let _ = std::fs::remove_dir_all(dir);
}
