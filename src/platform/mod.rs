//! Outermost platform-adapter boundary.
//!
//! Platform code may call application contracts; core/application code never
//! depends on a concrete window, GPU, WebView, or accessibility adapter.

pub const DESKTOP_ENABLED: bool = cfg!(feature = "desktop");
pub const WINDOW_ENABLED: bool = cfg!(feature = "window");
pub const RENDER_ENABLED: bool = cfg!(feature = "render");
