use std::sync::Arc;

use crate::foundation::view_model::ValueCommand;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{FlipPolicy, Placement};
use crate::ui::widget::style::PopoverStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverTriggerMode {
    Click,
    Hover,
    ClickAndHoverPreview,
}

impl PopoverTriggerMode {
    pub(crate) fn allows_click(self) -> bool {
        matches!(self, Self::Click | Self::ClickAndHoverPreview)
    }

    pub(crate) fn allows_hover(self) -> bool {
        matches!(self, Self::Hover | Self::ClickAndHoverPreview)
    }
}

pub struct PopoverDescriptor<VM> {
    pub(crate) content: Box<Element<VM>>,
    pub(crate) open: Value<bool>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) placement: Placement,
    pub(crate) flip_policy: FlipPolicy,
    pub(crate) disabled: Value<bool>,
    pub(crate) style: Option<PopoverStyle>,
    pub(crate) trigger_mode: PopoverTriggerMode,
    pub(crate) close_on_escape: bool,
    pub(crate) close_on_outside_click: bool,
    pub(crate) match_anchor_width: bool,
}

impl<VM> Clone for PopoverDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            open: self.open.clone(),
            on_open_change: self.on_open_change.clone(),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled.clone(),
            style: self.style.clone(),
            trigger_mode: self.trigger_mode,
            close_on_escape: self.close_on_escape,
            close_on_outside_click: self.close_on_outside_click,
            match_anchor_width: self.match_anchor_width,
        }
    }
}

impl<VM> PopoverDescriptor<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> PopoverDescriptor<RootVm>
    where
        VM: 'static,
    {
        PopoverDescriptor {
            content: Box::new(self.content.scope_with_selector(selector.clone())),
            open: self.open,
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector)),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled,
            style: self.style,
            trigger_mode: self.trigger_mode,
            close_on_escape: self.close_on_escape,
            close_on_outside_click: self.close_on_outside_click,
            match_anchor_width: self.match_anchor_width,
        }
    }

    pub(crate) fn resolved_style(&self, mode: ResolvedThemeMode) -> PopoverStyle {
        self.style
            .clone()
            .unwrap_or_else(|| PopoverStyle::default_for(mode))
    }
}
