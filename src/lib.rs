//! Public compatibility facade for the `tgui` GUI framework.
//!
//! The implementation lives in workspace crates under `crates/`; this crate
//! keeps the stable `tgui::...` API surface for applications.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub use tgui_runtime::{
    animation, application, canvas, core, dialog, layout, logging, media, mvvm, notification,
    platform, prelude, theme, widgets,
};

#[cfg(feature = "audio")]
pub use tgui_runtime::audio;

#[cfg(feature = "video")]
pub use tgui_runtime::video;

pub use tgui_runtime::{el, init_logging_from_cargo_toml};
