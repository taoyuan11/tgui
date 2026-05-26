use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{Rect, WidgetId};

use super::placement::{OverlayId, OverlayLayer};

pub(crate) struct OverlayCloseHandle<VM> {
    pub source_widget_id: Option<WidgetId>,
    pub overlay_id: OverlayId,
    pub rect: Rect,
    pub layer: OverlayLayer,
    pub on_close: Option<ValueCommand<VM, bool>>,
    pub close_value: bool,
    pub return_focus_to: Option<WidgetId>,
    pub close_on_outside_click: bool,
    pub close_on_escape: bool,
}

impl<VM> Clone for OverlayCloseHandle<VM> {
    fn clone(&self) -> Self {
        Self {
            source_widget_id: self.source_widget_id,
            overlay_id: self.overlay_id,
            rect: self.rect,
            layer: self.layer,
            on_close: self.on_close.clone(),
            close_value: self.close_value,
            return_focus_to: self.return_focus_to,
            close_on_outside_click: self.close_on_outside_click,
            close_on_escape: self.close_on_escape,
        }
    }
}
