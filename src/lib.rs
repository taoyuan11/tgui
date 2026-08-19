//! Transactional, data-oriented GUI foundations.
//!
//! P3 provides immutable widget declarations, a retained generational Element
//! and Render Tree, reactive transactions and events, logical-pixel Taffy
//! layout, typed Paint IR, chunk-cached scene compilation, and an optional wgpu
//! executor. The minimal `--no-default-features` build remains fully headless.

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
    CpuSnapshot, EventDispatchReceipt, LayoutFrameReceipt, RenderFrameReceipt, WindowContext,
    WindowSpec,
};
pub use core::{
    Color, DpiScale, ElementId, Error, Point, Rect, RenderNodeId, Result, Size, WindowId,
};
pub use state::{
    BackgroundMessage, DependencyPhase, RevisionMask, Signal, State, StateInvalidation, TxnReceipt,
    UiCommand, UiDispatcher, UiInbox, UpdateTxn, ui_channel,
};
