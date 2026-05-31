//! Drawer / Sidebar 从屏幕边缘滑出的容器组件。
//!
//! - [`Drawer`]：从四个方向之一滑出的侧边栏 widget builder；
//! - [`DrawerPlacement`]：Drawer 出现的方向（Left / Right / Top / Bottom）。
//!
//! Drawer 用于导航、过滤、详情等场景。与 Modal 类似，但从边缘滑出而非居中。
//! 支持两种模式：
//! - Overlay 模式（默认）：覆盖在内容之上，带半透明遮罩；
//! - Push 模式：推动主内容（暂未实现，需要应用层配合）。
//!
//! 实现要点：
//! - 使用 Stack 作为外层容器，全屏覆盖；
//! - backdrop（半透明遮罩）+ drawer panel（从边缘滑出）；
//! - panel 通过 `position_absolute` + 方向对应的边距实现定位；
//! - 动画：backdrop fade + panel slide（通过 `left`/`right`/`top`/`bottom` 的 animated 值）；
//! - 使用 `FocusScopeOptions::trap(true)` 实现 focus trap；
//! - Esc 关闭、focus return、关闭命令派发通过 `DrawerDescriptor` + collect 阶段 sentinel overlay 完成；
//! - backdrop 点击关闭在 collect 阶段按当前 open 状态注入命中区。

mod descriptor;
mod placement;
mod widget;

pub(crate) use descriptor::DrawerDescriptor;
pub use placement::DrawerPlacement;
pub use widget::Drawer;
