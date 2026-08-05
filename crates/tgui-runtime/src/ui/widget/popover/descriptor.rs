use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::foundation::view_model::ValueCommand;
use crate::theme::StyleContext;
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{FlipPolicy, Placement};
use crate::ui::widget::style::{PopoverStyle, StyleResolver};
use crate::ui::widget::WidgetId;

#[derive(Clone)]
pub(crate) struct PopoverOpenHandle {
    open: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct PopoverVirtualListNavigation {
    pub(crate) list_id: WidgetId,
    resolve_disabled: Arc<dyn Fn() -> Vec<bool> + Send + Sync>,
}

impl PopoverVirtualListNavigation {
    pub(crate) fn new(
        list_id: WidgetId,
        resolve_disabled: impl Fn() -> Vec<bool> + Send + Sync + 'static,
    ) -> Self {
        Self {
            list_id,
            resolve_disabled: Arc::new(resolve_disabled),
        }
    }

    pub(crate) fn resolve_disabled(&self) -> Vec<bool> {
        (self.resolve_disabled)()
    }
}

impl PopoverOpenHandle {
    pub(crate) fn new(open: bool) -> Self {
        Self {
            open: Arc::new(AtomicBool::new(open)),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    pub(crate) fn set(&self, open: bool) {
        self.open.store(open, Ordering::Relaxed);
    }
}

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
    pub(crate) open: Option<Value<bool>>,
    pub(crate) internal_open: Option<PopoverOpenHandle>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) placement: Placement,
    pub(crate) flip_policy: FlipPolicy,
    pub(crate) disabled: Value<bool>,
    pub(crate) style: Option<StyleResolver<PopoverStyle>>,
    pub(crate) trigger_mode: PopoverTriggerMode,
    pub(crate) close_on_escape: bool,
    pub(crate) close_on_outside_click: bool,
    pub(crate) match_anchor_width: bool,
    pub(crate) list_keyboard_navigation: bool,
    pub(crate) virtual_list_navigation: Option<PopoverVirtualListNavigation>,
    pub(crate) return_focus_to: Option<WidgetId>,
}

impl<VM> Clone for PopoverDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            open: self.open.clone(),
            internal_open: self.internal_open.clone(),
            on_open_change: self.on_open_change.clone(),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled.clone(),
            style: self.style.clone(),
            trigger_mode: self.trigger_mode,
            close_on_escape: self.close_on_escape,
            close_on_outside_click: self.close_on_outside_click,
            match_anchor_width: self.match_anchor_width,
            list_keyboard_navigation: self.list_keyboard_navigation,
            virtual_list_navigation: self.virtual_list_navigation.clone(),
            return_focus_to: self.return_focus_to,
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
            internal_open: self.internal_open,
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector)),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled,
            style: self.style,
            trigger_mode: self.trigger_mode,
            close_on_escape: self.close_on_escape,
            close_on_outside_click: self.close_on_outside_click,
            match_anchor_width: self.match_anchor_width,
            list_keyboard_navigation: self.list_keyboard_navigation,
            virtual_list_navigation: self.virtual_list_navigation,
            return_focus_to: self.return_focus_to,
        }
    }

    pub(crate) fn resolved_style(&self, context: &StyleContext<'_>) -> PopoverStyle {
        let mut base = PopoverStyle::default_for_theme(context.theme);
        context.theme.components.popover.apply(&mut base, context);
        self.style
            .as_ref()
            .map(|resolver| resolver.resolve_from(base.clone(), context))
            .unwrap_or(base)
    }

    pub(crate) fn is_open(&self) -> bool {
        self.internal_open
            .as_ref()
            .map(PopoverOpenHandle::is_open)
            .or_else(|| self.open.as_ref().map(Value::resolve))
            .unwrap_or(false)
    }
}
