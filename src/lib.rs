//! Transactional, data-oriented GUI foundations.
//!
//! P0 deliberately contains contracts and a headless implementation rather
//! than a platform renderer. The minimal core has no backend dependencies and
//! remains available with `--no-default-features`.

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

pub use application::{Application, AtomicSnapshotStore, CpuSnapshot, WindowContext, WindowSpec};
pub use core::{
    Color, DpiScale, ElementId, Error, Point, Rect, RenderNodeId, Result, Size, WindowId,
};
pub use state::{BackgroundMessage, UiDispatcher, UiInbox, UpdateTxn, ui_channel};
