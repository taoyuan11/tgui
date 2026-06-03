mod api;
mod config;
mod dispatcher;
mod file;
mod platform;
mod profiler;
#[cfg(test)]
mod tests;

pub use api::{tgui_log, Log, LogLevel};
pub use config::{
    configure_logging, configure_logging_from_manifest, LogConfig, LogConfigError, LogFileConfig,
};

pub(crate) use profiler::{log_startup_phase, log_text_profile, text_profile_enabled};
