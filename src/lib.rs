//! Transactional, data-oriented GUI foundations.
//!
//! P2 provides immutable widget declarations, a retained generational Element
//! tree, reactive transactions and events, logical-pixel Taffy layout, and a
//! boundary-aware Dirty Tree. The minimal `--no-default-features` build remains
//! fully headless and has no platform, GPU, text-shaping, or media backend.

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
    CpuSnapshot, EventDispatchReceipt, LayoutFrameReceipt, WindowContext, WindowSpec,
};
pub use core::{
    Color, DpiScale, ElementId, Error, Point, Rect, RenderNodeId, Result, Size, WindowId,
};
pub use state::{
    BackgroundMessage, DependencyPhase, RevisionMask, Signal, State, StateInvalidation, TxnReceipt,
    UiCommand, UiDispatcher, UiInbox, UpdateTxn, ui_channel,
};
