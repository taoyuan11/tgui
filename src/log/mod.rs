mod api;
mod dispatcher;
mod platform;
mod profiler;
#[cfg(test)]
mod tests;

pub use api::{tgui_log, Log, LogLevel};

pub(crate) use profiler::{log_startup_phase, log_text_profile, text_profile_enabled};
