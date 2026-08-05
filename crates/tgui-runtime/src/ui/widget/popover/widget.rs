use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::theme::StyleContext;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{Alignment, FlipPolicy, Placement};
use crate::ui::widget::style::{PopoverStyle, StyleResolver};
use crate::ui::widget::WidgetId;

use super::descriptor::{PopoverDescriptor, PopoverOpenHandle, PopoverTriggerMode};

pub struct Popover<VM> {
    trigger: Element<VM>,
    content: Option<Element<VM>>,
    open: Option<Value<bool>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    placement: Placement,
    flip_policy: FlipPolicy,
    disabled: Value<bool>,
    style: Option<StyleResolver<PopoverStyle>>,
    trigger_mode: PopoverTriggerMode,
    close_on_escape: bool,
    close_on_outside_click: bool,
    match_anchor_width: bool,
    internal_open: Option<PopoverOpenHandle>,
    list_keyboard_navigation: bool,
    return_focus_to: Option<WidgetId>,
}

impl<VM: 'static> Popover<VM> {
    pub fn new(trigger: impl Into<Element<VM>>) -> Self {
        Self {
            trigger: trigger.into(),
            content: None,
            open: None,
            on_open_change: None,
            placement: Placement::bottom().align(Alignment::Start),
            flip_policy: FlipPolicy::FlipSide,
            disabled: Value::Static(false),
            style: None,
            trigger_mode: PopoverTriggerMode::Click,
            close_on_escape: true,
            close_on_outside_click: true,
            match_anchor_width: false,
            internal_open: None,
            list_keyboard_navigation: false,
            return_focus_to: None,
        }
    }

    pub fn content(mut self, content: impl Into<Element<VM>>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = Some(open.into());
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn flip_policy(mut self, policy: FlipPolicy) -> Self {
        self.flip_policy = policy;
        self
    }

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut PopoverStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| PopoverStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> PopoverStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn trigger_mode(mut self, mode: PopoverTriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn close_on_outside_click(mut self, close: bool) -> Self {
        self.close_on_outside_click = close;
        self
    }

    pub fn match_anchor_width(mut self, on: bool) -> Self {
        self.match_anchor_width = on;
        self
    }

    #[allow(dead_code)] // Used by composite controls that share the trigger's uncontrolled state.
    pub(crate) fn open_handle(mut self, handle: PopoverOpenHandle) -> Self {
        self.internal_open = Some(handle);
        self
    }

    pub(crate) fn list_keyboard_navigation(mut self, enabled: bool) -> Self {
        self.list_keyboard_navigation = enabled;
        self
    }

    /// Sets the focus target restored when the popover closes.
    ///
    /// This is useful when the visual trigger is a composite element whose
    /// focusable control is one of its descendants.
    pub fn return_focus_to(mut self, widget_id: WidgetId) -> Self {
        self.return_focus_to = Some(widget_id);
        self
    }
}

impl<VM: 'static> From<Popover<VM>> for Element<VM> {
    fn from(popover: Popover<VM>) -> Element<VM> {
        let Popover {
            mut trigger,
            content,
            open,
            on_open_change,
            placement,
            flip_policy,
            disabled,
            style,
            trigger_mode,
            close_on_escape,
            close_on_outside_click,
            match_anchor_width,
            internal_open,
            list_keyboard_navigation,
            return_focus_to,
        } = popover;

        let content = content.expect("Popover::content(...) must be provided");
        // Omitting `.open(...)` creates an internal uncontrolled state. Explicit static and
        // signal values remain controlled and require `on_open_change` to accept user requests.
        let internal_open =
            internal_open.or_else(|| open.is_none().then(|| PopoverOpenHandle::new(false)));
        let on_open_change = match (internal_open.clone(), on_open_change) {
            (Some(internal_open), notify) => {
                Some(ValueCommand::new_with_context(move |vm, next, context| {
                    internal_open.set(next);
                    if let Some(command) = notify.as_ref() {
                        command.execute_with_context(vm, next, context);
                    }
                }))
            }
            (None, command) => command,
        };

        let descriptor = PopoverDescriptor {
            content: Box::new(content),
            open,
            internal_open,
            on_open_change,
            placement,
            flip_policy,
            disabled,
            style,
            trigger_mode,
            close_on_escape,
            close_on_outside_click,
            match_anchor_width,
            list_keyboard_navigation,
            virtual_list_navigation: None,
            return_focus_to,
        };
        trigger.popover = Some(Box::new(descriptor));
        trigger
    }
}
