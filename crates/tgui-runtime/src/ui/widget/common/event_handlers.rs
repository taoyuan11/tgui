use super::*;

pub(crate) struct InteractionHandlers<VM> {
    pub on_click: Option<Command<VM>>,
    pub on_double_click: Option<Command<VM>>,
    pub on_focus: Option<Command<VM>>,
    pub on_blur: Option<Command<VM>>,
    pub on_mouse_enter: Option<Command<VM>>,
    pub on_mouse_leave: Option<Command<VM>>,
    pub on_mouse_move: Option<ValueCommand<VM, Point>>,
    pub on_file_drop: Option<ValueCommand<VM, FileDropEvent>>,
    pub gesture: Option<crate::ui::widget::GestureRecognizer<VM>>,
    pub cursor_style: Option<Value<CursorStyle>>,
    pub number_input: Option<NumberInputInteraction<VM>>,
    pub calendar_day: Option<CalendarDayInteraction<VM>>,
    pub radio_group: Option<RadioGroupInteraction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RadioGroupInteraction {
    pub(crate) group_id: WidgetId,
    pub(crate) index: usize,
    pub(crate) direction: Axis,
}

pub(crate) struct CalendarDayInteraction<VM> {
    pub(crate) owner_id: WidgetId,
    pub(crate) date: chrono::NaiveDate,
    pub(crate) on_focus_move: ValueCommand<VM, chrono::NaiveDate>,
}

impl<VM> Clone for CalendarDayInteraction<VM> {
    fn clone(&self) -> Self {
        Self {
            owner_id: self.owner_id,
            date: self.date,
            on_focus_move: self.on_focus_move.clone(),
        }
    }
}

impl<VM: 'static> CalendarDayInteraction<VM> {
    fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> CalendarDayInteraction<RootVm> {
        CalendarDayInteraction {
            owner_id: self.owner_id,
            date: self.date,
            on_focus_move: self.on_focus_move.scope(selector),
        }
    }
}

pub(crate) struct NumberInputInteraction<VM> {
    pub(crate) increment: Command<VM>,
    pub(crate) decrement: Command<VM>,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) step: f64,
}

impl<VM> Clone for NumberInputInteraction<VM> {
    fn clone(&self) -> Self {
        Self {
            increment: self.increment.clone(),
            decrement: self.decrement.clone(),
            min: self.min,
            max: self.max,
            step: self.step,
        }
    }
}

impl<VM: 'static> NumberInputInteraction<VM> {
    fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> NumberInputInteraction<RootVm> {
        NumberInputInteraction {
            increment: self.increment.scope(selector.clone()),
            decrement: self.decrement.scope(selector),
            min: self.min,
            max: self.max,
            step: self.step,
        }
    }
}

pub(crate) struct CanvasItemInteractionHandlers<VM> {
    pub on_click: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_double_click: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_down: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_up: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_enter: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_leave: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_move: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_wheel: Option<ValueCommand<VM, CanvasWheelEvent>>,
    pub on_drag_start: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub on_drag: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub on_drag_end: Option<ValueCommand<VM, CanvasDragEvent>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MediaEventPhase {
    Loading,
    Success,
    Error(String),
}

pub(crate) struct MediaEventHandlers<VM> {
    pub on_loading: Option<Command<VM>>,
    pub on_success: Option<Command<VM>>,
    pub on_error: Option<ValueCommand<VM, String>>,
}

pub(crate) struct LifecycleEventHandlers<VM> {
    pub on_mount: Option<Command<VM>>,
    pub on_unmount: Option<Command<VM>>,
    pub on_update: Option<Command<VM>>,
}

impl<VM> Clone for MediaEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_loading: self.on_loading.clone(),
            on_success: self.on_success.clone(),
            on_error: self.on_error.clone(),
        }
    }
}

impl<VM> Default for MediaEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_loading: None,
            on_success: None,
            on_error: None,
        }
    }
}

impl<VM> MediaEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_loading.is_some() || self.on_success.is_some() || self.on_error.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> MediaEventHandlers<RootVm>
    where
        VM: 'static,
    {
        MediaEventHandlers {
            on_loading: self
                .on_loading
                .map(|command| command.scope(selector.clone())),
            on_success: self
                .on_success
                .map(|command| command.scope(selector.clone())),
            on_error: self.on_error.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> Clone for LifecycleEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_mount: self.on_mount.clone(),
            on_unmount: self.on_unmount.clone(),
            on_update: self.on_update.clone(),
        }
    }
}

impl<VM> Default for LifecycleEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_mount: None,
            on_unmount: None,
            on_update: None,
        }
    }
}

impl<VM> LifecycleEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_mount.is_some() || self.on_unmount.is_some() || self.on_update.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> LifecycleEventHandlers<RootVm>
    where
        VM: 'static,
    {
        LifecycleEventHandlers {
            on_mount: self.on_mount.map(|command| command.scope(selector.clone())),
            on_unmount: self
                .on_unmount
                .map(|command| command.scope(selector.clone())),
            on_update: self.on_update.map(|command| command.scope(selector)),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MediaEventState<VM> {
    pub widget_id: WidgetId,
    pub media_phase: Option<MediaEventPhase>,
    pub handlers: MediaEventHandlers<VM>,
}

pub(crate) struct LifecycleEventState<VM> {
    pub widget_id: WidgetId,
    pub snapshot: crate::ui::widget::core::LifecycleSnapshot,
    pub handlers: LifecycleEventHandlers<VM>,
}

impl<VM> Clone for LifecycleEventState<VM> {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            snapshot: self.snapshot.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl<VM> Clone for InteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
            on_double_click: self.on_double_click.clone(),
            on_focus: self.on_focus.clone(),
            on_blur: self.on_blur.clone(),
            on_mouse_enter: self.on_mouse_enter.clone(),
            on_mouse_leave: self.on_mouse_leave.clone(),
            on_mouse_move: self.on_mouse_move.clone(),
            on_file_drop: self.on_file_drop.clone(),
            gesture: self.gesture.clone(),
            cursor_style: self.cursor_style.clone(),
            number_input: self.number_input.clone(),
            calendar_day: self.calendar_day.clone(),
            radio_group: self.radio_group,
        }
    }
}

impl<VM> Clone for CanvasItemInteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
            on_double_click: self.on_double_click.clone(),
            on_mouse_down: self.on_mouse_down.clone(),
            on_mouse_up: self.on_mouse_up.clone(),
            on_mouse_enter: self.on_mouse_enter.clone(),
            on_mouse_leave: self.on_mouse_leave.clone(),
            on_mouse_move: self.on_mouse_move.clone(),
            on_wheel: self.on_wheel.clone(),
            on_drag_start: self.on_drag_start.clone(),
            on_drag: self.on_drag.clone(),
            on_drag_end: self.on_drag_end.clone(),
        }
    }
}

impl<VM> Default for InteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
            on_double_click: None,
            on_focus: None,
            on_blur: None,
            on_mouse_enter: None,
            on_mouse_leave: None,
            on_mouse_move: None,
            on_file_drop: None,
            gesture: None,
            cursor_style: None,
            number_input: None,
            calendar_day: None,
            radio_group: None,
        }
    }
}

impl<VM> Default for CanvasItemInteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
            on_double_click: None,
            on_mouse_down: None,
            on_mouse_up: None,
            on_mouse_enter: None,
            on_mouse_leave: None,
            on_mouse_move: None,
            on_wheel: None,
            on_drag_start: None,
            on_drag: None,
            on_drag_end: None,
        }
    }
}

impl<VM> InteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_focus.is_some()
            || self.on_blur.is_some()
            || self.on_mouse_enter.is_some()
            || self.on_mouse_leave.is_some()
            || self.on_mouse_move.is_some()
            || self.on_file_drop.is_some()
            || self
                .gesture
                .as_ref()
                .map(|gesture| gesture.has_any())
                .unwrap_or(false)
            || self.cursor_style.is_some()
            || self.number_input.is_some()
            || self.calendar_day.is_some()
            || self.radio_group.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> InteractionHandlers<RootVm>
    where
        VM: 'static,
    {
        InteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_double_click: self
                .on_double_click
                .map(|command| command.scope(selector.clone())),
            on_focus: self.on_focus.map(|command| command.scope(selector.clone())),
            on_blur: self.on_blur.map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self
                .on_mouse_move
                .map(|command| command.scope(selector.clone())),
            on_file_drop: self
                .on_file_drop
                .map(|command| command.scope(selector.clone())),
            gesture: self.gesture.map(|gesture| gesture.scope(selector.clone())),
            cursor_style: self.cursor_style,
            number_input: self
                .number_input
                .map(|number_input| number_input.scope(selector.clone())),
            calendar_day: self
                .calendar_day
                .map(|calendar_day| calendar_day.scope(selector)),
            radio_group: self.radio_group,
        }
    }
}

impl<VM: 'static> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> CanvasItemInteractionHandlers<RootVm> {
        CanvasItemInteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_double_click: self
                .on_double_click
                .map(|command| command.scope(selector.clone())),
            on_mouse_down: self
                .on_mouse_down
                .map(|command| command.scope(selector.clone())),
            on_mouse_up: self
                .on_mouse_up
                .map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self
                .on_mouse_move
                .map(|command| command.scope(selector.clone())),
            on_wheel: self.on_wheel.map(|command| command.scope(selector.clone())),
            on_drag_start: self
                .on_drag_start
                .map(|command| command.scope(selector.clone())),
            on_drag: self.on_drag.map(|command| command.scope(selector.clone())),
            on_drag_end: self.on_drag_end.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_mouse_down.is_some()
            || self.on_mouse_up.is_some()
            || self.on_mouse_enter.is_some()
            || self.on_mouse_leave.is_some()
            || self.on_mouse_move.is_some()
            || self.on_wheel.is_some()
            || self.on_drag_start.is_some()
            || self.on_drag.is_some()
            || self.on_drag_end.is_some()
    }
}
