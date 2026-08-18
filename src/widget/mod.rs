//! Immutable widget declarations and the retained element runtime.
//!
//! Application code creates [`WidgetNode`] values through [`Widget`] or
//! [`View`]. The persistent element tree is deliberately crate-private: it is
//! owned by the UI thread and is only exposed through read-only diagnostics and
//! the headless test harness.

mod declaration;
pub(crate) mod element;

pub use declaration::{
    BuildContext, LifecycleCallback, LifecycleEvent, PropertyValue, View, Widget, WidgetNode,
    WidgetType,
};
pub use element::{ElementNodeDiagnostics, ElementTreeStats, ReconcileDiagnostic, ReconcileReport};
