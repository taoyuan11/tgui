//! Multi-file component gallery with categorized sidebar navigation.
//!
//! Run the deterministic headless path with:
//!
//! ```text
//! cargo run --example multi --no-default-features
//! ```
//!
//! Open the real desktop window with:
//!
//! ```text
//! cargo run --example multi
//! ```

#[path = "multi/app.rs"]
mod app;
#[cfg(feature = "desktop")]
#[path = "multi/desktop.rs"]
mod desktop;
#[path = "multi/layout.rs"]
mod layout;
#[path = "multi/navigation.rs"]
mod navigation;
#[path = "multi/pages/mod.rs"]
mod pages;

#[cfg(feature = "desktop")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    desktop::run()
}

#[cfg(not(feature = "desktop"))]
fn main() -> tgui::Result<()> {
    app::run_headless()
}
