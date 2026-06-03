use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{OnceLock, RwLock};

use toml_edit::{DocumentMut, Item};

use super::api::LogLevel;

const DEFAULT_LOG_FILE_NAME: &str = "tgui.log";
const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFileConfig {
    pub log_dir: PathBuf,
    pub file_name: String,
    pub max_file_size_bytes: u64,
    pub max_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogConfig {
    pub level: LogLevel,
    pub file: Option<LogFileConfig>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Trace,
            file: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogConfigError {
    ParseManifest(String),
    InvalidLevel(String),
    InvalidType(&'static str),
    InvalidValue(&'static str, String),
}

impl Display for LogConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseManifest(error) => write!(f, "failed to parse Cargo.toml: {error}"),
            Self::InvalidLevel(level) => write!(f, "invalid tgui logging level: {level}"),
            Self::InvalidType(field) => write!(f, "invalid tgui logging field type: {field}"),
            Self::InvalidValue(field, value) => {
                write!(f, "invalid tgui logging value for {field}: {value}")
            }
        }
    }
}

impl Error for LogConfigError {}

static LOG_CONFIG: OnceLock<RwLock<LogConfig>> = OnceLock::new();
static MIN_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Trace as u8);

pub fn configure_logging(config: LogConfig) -> Result<(), LogConfigError> {
    validate_config(&config)?;
    let mut current = config_state()
        .write()
        .expect("logging config lock poisoned");
    MIN_LOG_LEVEL.store(config.level as u8, Ordering::Release);
    *current = config;
    Ok(())
}

pub fn configure_logging_from_manifest(
    manifest: &str,
    manifest_dir: impl AsRef<Path>,
) -> Result<(), LogConfigError> {
    let config = parse_manifest_config(manifest, manifest_dir.as_ref())?;
    configure_logging(config)
}

pub(super) fn current_config() -> LogConfig {
    config_state()
        .read()
        .expect("logging config lock poisoned")
        .clone()
}

pub(super) fn enabled(level: LogLevel) -> bool {
    (level as u8) >= MIN_LOG_LEVEL.load(Ordering::Acquire)
}

fn config_state() -> &'static RwLock<LogConfig> {
    LOG_CONFIG.get_or_init(|| RwLock::new(LogConfig::default()))
}

fn parse_manifest_config(manifest: &str, manifest_dir: &Path) -> Result<LogConfig, LogConfigError> {
    let document = manifest
        .parse::<DocumentMut>()
        .map_err(|error| LogConfigError::ParseManifest(error.to_string()))?;
    let Some(logging) = document
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("tgui"))
        .and_then(|tgui| tgui.get("logging"))
    else {
        return Ok(LogConfig::default());
    };

    let level = match logging.get("level") {
        Some(item) => {
            let value = required_str(item, "level")?;
            LogLevel::from_str(value)?
        }
        None => LogLevel::Trace,
    };

    let file = match logging.get("log_dir") {
        Some(item) => {
            let log_dir = PathBuf::from(required_str(item, "log_dir")?);
            let log_dir = if log_dir.is_absolute() {
                log_dir
            } else {
                manifest_dir.join(log_dir)
            };
            Some(LogFileConfig {
                log_dir,
                file_name: optional_str(logging, "file_name")?
                    .unwrap_or(DEFAULT_LOG_FILE_NAME)
                    .to_string(),
                max_file_size_bytes: optional_u64(logging, "max_file_size_bytes")?
                    .unwrap_or(DEFAULT_MAX_FILE_SIZE_BYTES),
                max_files: optional_usize(logging, "max_files")?.unwrap_or(DEFAULT_MAX_FILES),
            })
        }
        None => None,
    };

    Ok(LogConfig { level, file })
}

fn validate_config(config: &LogConfig) -> Result<(), LogConfigError> {
    if let Some(file) = &config.file {
        if file.file_name.trim().is_empty() {
            return Err(LogConfigError::InvalidValue(
                "file_name",
                file.file_name.clone(),
            ));
        }
        if file.max_file_size_bytes == 0 {
            return Err(LogConfigError::InvalidValue(
                "max_file_size_bytes",
                "0".to_string(),
            ));
        }
        if file.max_files == 0 {
            return Err(LogConfigError::InvalidValue("max_files", "0".to_string()));
        }
    }

    Ok(())
}

fn required_str<'a>(item: &'a Item, field: &'static str) -> Result<&'a str, LogConfigError> {
    item.as_str().ok_or(LogConfigError::InvalidType(field))
}

fn optional_str<'a>(
    table: &'a Item,
    field: &'static str,
) -> Result<Option<&'a str>, LogConfigError> {
    table
        .get(field)
        .map(|item| required_str(item, field))
        .transpose()
}

fn optional_u64(table: &Item, field: &'static str) -> Result<Option<u64>, LogConfigError> {
    table
        .get(field)
        .map(|item| {
            item.as_integer()
                .and_then(|value| u64::try_from(value).ok())
                .ok_or(LogConfigError::InvalidType(field))
        })
        .transpose()
}

fn optional_usize(table: &Item, field: &'static str) -> Result<Option<usize>, LogConfigError> {
    table
        .get(field)
        .map(|item| {
            item.as_integer()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(LogConfigError::InvalidType(field))
        })
        .transpose()
}

#[cfg(test)]
pub(super) fn reset_for_test() {
    configure_logging(LogConfig::default()).unwrap();
}
