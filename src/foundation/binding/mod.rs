mod context;
mod dependency;
mod invalidation;
mod scroll;
mod signal_state;
#[cfg(test)]
mod tests;
mod text;
mod toast;

pub use context::ViewModelContext;
pub use scroll::{ScrollRequest, ScrollRequestMode, ScrollViewController};
pub use signal_state::{Signal, State};
pub use text::{TextChange, TextChangeSet, TextController, TextSnapshot};
pub use toast::{Toast, ToastAction, ToastEntry, ToastId, ToastKind, ToastPlacement, ToastQueue};

pub(crate) use dependency::{
    record_dependency_read, track_dependency_scope, track_property_scope,
    with_dependency_collection, DependencyGraph, DependencyId, DependencyOwner, DependencyPhase,
    DirtyDependencySet, PropertySlot,
};
pub(crate) use invalidation::InvalidationSignal;
