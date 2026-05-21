//! 浮层关闭句柄。
//!
//! 由 `emit_overlay` 在 collect 阶段写入 `ComputedScene::overlay_close_handlers`，
//! 由 runtime 在 pointer press / Esc 时反向遍历消费。
//!
//! "清空-重建"语义：每帧 collect 重新生成；上一帧的 handle 在新帧不再被写入即自动失效。

use crate::foundation::view_model::Command;
use crate::ui::widget::common::{Rect, WidgetId};

use super::placement::{OverlayId, OverlayLayer};

pub(crate) struct OverlayCloseHandle<VM> {
    pub overlay_id: OverlayId,
    pub rect: Rect,
    pub layer: OverlayLayer,
    pub on_close: Option<Command<VM>>,
    pub return_focus_to: Option<WidgetId>,
    pub close_on_outside_click: bool,
    pub close_on_escape: bool,
}

impl<VM> Clone for OverlayCloseHandle<VM> {
    fn clone(&self) -> Self {
        Self {
            overlay_id: self.overlay_id,
            rect: self.rect,
            layer: self.layer,
            on_close: self.on_close.clone(),
            return_focus_to: self.return_focus_to,
            close_on_outside_click: self.close_on_outside_click,
            close_on_escape: self.close_on_escape,
        }
    }
}
