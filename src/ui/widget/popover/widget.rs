use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{Alignment, FlipPolicy, Placement};
use crate::ui::widget::style::PopoverStyle;

use super::descriptor::{PopoverDescriptor, PopoverTriggerMode};

pub struct Popover<VM> {
    trigger: Element<VM>,
    content: Option<Element<VM>>,
    open: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    placement: Placement,
    flip_policy: FlipPolicy,
    disabled: Value<bool>,
    style: Option<PopoverStyle>,
    trigger_mode: PopoverTriggerMode,
    close_on_escape: bool,
    close_on_outside_click: bool,
    match_anchor_width: bool,
}

impl<VM: 'static> Popover<VM> {
    pub fn new(trigger: impl Into<Element<VM>>) -> Self {
        Self {
            trigger: trigger.into(),
            content: None,
            open: Value::Static(false),
            on_open_change: None,
            placement: Placement::bottom().align(Alignment::Start),
            flip_policy: FlipPolicy::FlipSide,
            disabled: Value::Static(false),
            style: None,
            trigger_mode: PopoverTriggerMode::Click,
            close_on_escape: true,
            close_on_outside_click: true,
            match_anchor_width: false,
        }
    }

    pub fn content(mut self, content: impl Into<Element<VM>>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
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

    pub fn style(mut self, style: PopoverStyle) -> Self {
        self.style = Some(style);
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
        } = popover;

        let content = content.expect("Popover::content(...) must be provided");
        let click_toggle = on_open_change
            .clone()
            .filter(|_| trigger_mode.allows_click())
            .map(|command| {
                let user_click = trigger.interactions.on_click.clone();
                let open_value = open.clone();
                Command::new_with_context(move |vm, context| {
                    if let Some(command) = user_click.as_ref() {
                        command.execute_with_context(vm, context);
                    }
                    command.execute_with_context(vm, !open_value.resolve(), context);
                })
            });
        if let Some(command) = click_toggle {
            trigger.interactions.on_click = Some(command);
        }

        let descriptor = PopoverDescriptor {
            content: Box::new(content),
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
        };
        trigger.popover = Some(Box::new(descriptor));
        trigger
    }
}
