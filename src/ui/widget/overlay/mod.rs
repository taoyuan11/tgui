//! Overlay / Popup anchoring 引擎。
//!
//! 本模块给浮层组件（Tooltip / Menu / Popover / Modal / DatePicker / Combobox 等）提供
//! 共用的"锚点 + 视口 + 翻转策略 -> 最终矩形"求解能力。
//!
//! 设计要点见 `docs/`（待补）与 `COMPONENTS_ROADMAP.md` P0 §1。
//!
//! 本轮交付仅包含**纯函数求解器与公开类型**；collect 阶段集成与运行时关闭机制由后续任务补全。

mod anchor;
mod close;
pub(crate) mod collect;
mod overlay;
mod placement;
mod solver;

#[cfg(test)]
mod tests;

pub use anchor::Anchor;
pub use placement::{
    Alignment, FlipPolicy, OverlayId, OverlayLayer, Placement, PlacementOptions, Side,
    SolvedPlacement,
};
pub use solver::solve_placement;

pub(crate) use close::OverlayCloseHandle;
pub(crate) use overlay::{Overlay, OverlayContent, OverlayPrimitive};
