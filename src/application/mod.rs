mod builder;
mod config;
#[cfg(test)]
mod tests;
mod window_spec;

pub use builder::ApplicationBuilder;
pub use config::{Application, MsaaMode, ResourceBudget};
pub use window_spec::{WindowClosePolicy, WindowRole, WindowSpec};

pub(crate) use builder::WindowSetFactory;
pub(crate) use config::{ApplicationConfig, ThemeSelection};
