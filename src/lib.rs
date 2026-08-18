//! Transactional, data-oriented GUI foundations.
//!
//! P1 provides immutable widget declarations, a retained generational Element
//! tree, reactive State/Signal transactions, and backend-neutral event routing.
//! The minimal core remains headless and dependency-free with
//! `--no-default-features`.

#![forbid(unsafe_code)]

pub mod accessibility;
pub mod animation;
pub mod application;
pub mod core;
pub mod diagnostics;
pub(crate) mod dirty;
pub mod event;
pub mod layout;
pub mod media;
pub mod native;
pub mod platform;
pub mod render;
pub mod state;
pub mod test_support;
pub mod text;
pub mod virtualization;
pub mod widget;
pub mod widgets;

pub use application::{
    Application, ApplicationTxnReceipt, AtomicSnapshotStore, BackgroundDispatchReceipt,
    CpuSnapshot, EventDispatchReceipt, WindowContext, WindowSpec,
};
pub use core::{
    Color, DpiScale, ElementId, Error, Point, Rect, RenderNodeId, Result, Size, WindowId,
};
pub use state::{
    BackgroundMessage, DependencyPhase, RevisionMask, Signal, State, StateInvalidation, TxnReceipt,
    UiCommand, UiDispatcher, UiInbox, UpdateTxn, ui_channel,
};
