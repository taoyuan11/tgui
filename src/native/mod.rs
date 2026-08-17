//! Native-host escape-hatch boundary.
//!
//! Native hosts are reserved for capabilities such as WebView or foreign
//! surfaces. They are never an implementation path for ordinary controls.

/// Reviewable invariant used by architecture tests and documentation.
pub const ORDINARY_CONTROLS_MAY_USE_NATIVE_HOST: bool = false;

pub const WEBVIEW_ADAPTER_ENABLED: bool = cfg!(feature = "webview");
