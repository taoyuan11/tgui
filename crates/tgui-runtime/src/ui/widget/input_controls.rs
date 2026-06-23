use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Timelike};

use crate::dialog::FileDialogOptions;
use crate::foundation::binding::{TextChangeSet, TextController};
use crate::foundation::color::Color;
use crate::foundation::form::ValidationVisualState;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{FontWeight, ResolvedThemeMode, StyleContext, Theme};
use crate::ui::layout::{fr, Align, Insets, Justify, Value, Wrap};
use crate::ui::theme::{StateValue, TextStyle};
use crate::ui::unit::{dp, sp, Dp};

use super::common::ButtonVariantKind;
use super::icon::SvgIconId;
use super::style::{
    ButtonStyle, ContainerStyle, InputStyle, PopoverStyle, SelectStyle, StyleResolver,
    TextWidgetStyle,
};
use super::{
    Button, CursorStyle, Element, FileDropEvent, Flex, Grid, Icon, Input, Popover, ProgressBar,
    Slider, Stack, Text,
};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const FIELD_HEIGHT: Dp = dp(40.0);
const PANEL_PADDING: Dp = dp(12.0);
const ICON_CALENDAR: SvgIconId = SvgIconId::Calendar;
const ICON_TIME: SvgIconId = SvgIconId::Clock;
const ICON_COLOR: SvgIconId = SvgIconId::Palette;
const ICON_EXPAND: SvgIconId = SvgIconId::ChevronDown;
const ICON_PREVIOUS: SvgIconId = SvgIconId::ChevronLeft;
const ICON_NEXT: SvgIconId = SvgIconId::ChevronRight;
const ICON_UP: SvgIconId = SvgIconId::ChevronUp;
const ICON_DOWN: SvgIconId = SvgIconId::ChevronDown;
const ICON_ADD: SvgIconId = SvgIconId::Plus;
const ICON_REMOVE: SvgIconId = SvgIconId::Minus;
const ICON_UPLOAD: SvgIconId = SvgIconId::Upload;
const ICON_DELETE: SvgIconId = SvgIconId::Delete;
const ICON_FILE: SvgIconId = SvgIconId::File;
const ICON_DONE: SvgIconId = SvgIconId::Success;
const ICON_ERROR: SvgIconId = SvgIconId::Error;
const ICON_PENDING: SvgIconId = SvgIconId::Pending;

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarStyle {
    pub day_size: Dp,
    pub gap: Dp,
    pub panel_width: Dp,
}

impl CalendarStyle {
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            day_size: theme.spacing.xl,
            gap: theme.spacing.xs,
            panel_width: dp(320.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatePickerStyle {
    pub width: Dp,
    pub calendar: CalendarStyle,
}

impl DatePickerStyle {
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            width: dp(320.0),
            calendar: CalendarStyle::default_for_theme(theme),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimePickerStyle {
    pub width: Dp,
    pub option_width: Dp,
}

impl TimePickerStyle {
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            width: dp(320.0),
            option_width: theme.spacing.xxl + theme.spacing.md + theme.spacing.xs,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberInputStyle {
    pub width: Dp,
    pub button_width: Dp,
}

impl NumberInputStyle {
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            width: dp(180.0),
            button_width: theme.spacing.xxl,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorPickerStyle {
    pub width: Dp,
    pub swatch_size: Dp,
}

impl ColorPickerStyle {
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            width: dp(320.0),
            swatch_size: theme.spacing.xl - theme.spacing.xxs,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UploadStyle {
    pub width: Dp,
}

impl UploadStyle {
    pub(crate) fn default_for_theme(_: &Theme) -> Self {
        Self { width: dp(460.0) }
    }
}

fn resolve_input_control_style<T: Clone>(
    style: Option<&StyleResolver<T>>,
    default: impl Fn(&Theme) -> T,
) -> T {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    let base = default(&theme);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), &context))
        .unwrap_or(base)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarChangeTrigger {
    Day,
    PreviousMonth,
    NextMonth,
    Today,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarSelectionChange {
    pub date: NaiveDate,
    pub display_month: NaiveDate,
    pub trigger: CalendarChangeTrigger,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatePickerChange {
    pub date: Option<NaiveDate>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimePickerChange {
    pub time: Option<NaiveTime>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberInputChangeTrigger {
    Text,
    StepUp,
    StepDown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberInputChange {
    pub value: Option<f64>,
    pub text: String,
    pub trigger: NumberInputChangeTrigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorPickerChangeTrigger {
    Swatch,
    Red,
    Green,
    Blue,
    Alpha,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorPickerChange {
    pub color: Color,
    pub trigger: ColorPickerChangeTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UploadFileId(String);

impl UploadFileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for UploadFileId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for UploadFileId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UploadStatus {
    Queued,
    Uploading { progress: f32 },
    Complete,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UploadFile {
    pub id: UploadFileId,
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: Option<u64>,
    pub status: UploadStatus,
}

impl UploadFile {
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled")
            .to_string();
        let size_bytes = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
        Self {
            id: UploadFileId::new(path.to_string_lossy().to_string()),
            path,
            name,
            size_bytes,
            status: UploadStatus::Queued,
        }
    }

    pub fn progress(&self) -> f32 {
        match &self.status {
            UploadStatus::Queued => 0.0,
            UploadStatus::Uploading { progress } => progress.clamp(0.0, 1.0),
            UploadStatus::Complete => 1.0,
            UploadStatus::Error(_) => 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UploadSelection {
    pub files: Vec<UploadFile>,
    pub rejected: Vec<UploadRejection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UploadRemove {
    pub id: UploadFileId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UploadRejection {
    pub path: PathBuf,
    pub reason: String,
}

pub struct Calendar<VM> {
    display_month: Value<NaiveDate>,
    selected: Value<Option<NaiveDate>>,
    today: Option<NaiveDate>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, CalendarSelectionChange>>,
    style: Option<StyleResolver<CalendarStyle>>,
    framed: bool,
}

impl<VM> Calendar<VM> {
    pub fn new(
        display_month: impl Into<Value<NaiveDate>>,
        selected: impl Into<Value<Option<NaiveDate>>>,
    ) -> Self {
        Self {
            display_month: display_month.into(),
            selected: selected.into(),
            today: Some(chrono::Local::now().date_naive()),
            disabled: Value::Static(false),
            on_change: None,
            style: None,
            framed: true,
        }
    }

    pub fn today(mut self, today: impl Into<Option<NaiveDate>>) -> Self {
        self.today = today.into();
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, CalendarSelectionChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CalendarStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| CalendarStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CalendarStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    fn unframed(mut self) -> Self {
        self.framed = false;
        self
    }
}

impl<VM: 'static> From<Calendar<VM>> for Element<VM> {
    fn from(calendar: Calendar<VM>) -> Self {
        match calendar.display_month {
            Value::Static(month) => {
                let style = resolve_input_control_style(
                    calendar.style.as_ref(),
                    CalendarStyle::default_for_theme,
                );
                calendar_element(
                    month,
                    calendar.selected.resolve(),
                    calendar.today,
                    calendar.disabled.resolve(),
                    calendar.on_change,
                    style,
                    calendar.framed,
                )
            }
            Value::Signal(month) => {
                let style = resolve_input_control_style(
                    calendar.style.as_ref(),
                    CalendarStyle::default_for_theme,
                );
                calendar_element(
                    month.get_untracked(),
                    calendar.selected.resolve_untracked(),
                    calendar.today,
                    calendar.disabled.resolve_untracked(),
                    calendar.on_change,
                    style,
                    calendar.framed,
                )
            }
        }
    }
}

pub struct DatePicker<VM> {
    controller: TextController,
    selected: Value<Option<NaiveDate>>,
    display_month: Value<NaiveDate>,
    open: Value<bool>,
    disabled: Value<bool>,
    validation: Value<ValidationVisualState>,
    placeholder: Value<String>,
    on_change: Option<ValueCommand<VM, DatePickerChange>>,
    on_month_change: Option<ValueCommand<VM, NaiveDate>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<DatePickerStyle>>,
}

impl<VM> DatePicker<VM> {
    pub fn new(
        controller: impl Into<TextController>,
        selected: impl Into<Value<Option<NaiveDate>>>,
        display_month: impl Into<Value<NaiveDate>>,
    ) -> Self {
        Self {
            controller: controller.into(),
            selected: selected.into(),
            display_month: display_month.into(),
            open: Value::Static(false),
            disabled: Value::Static(false),
            validation: Value::Static(ValidationVisualState::default()),
            placeholder: Value::Static("Select date".to_string()),
            on_change: None,
            on_month_change: None,
            on_open_change: None,
            style: None,
        }
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn validation(mut self, validation: impl Into<Value<ValidationVisualState>>) -> Self {
        self.validation = validation.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, DatePickerChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_month_change(mut self, command: ValueCommand<VM, NaiveDate>) -> Self {
        self.on_month_change = Some(command);
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut DatePickerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| DatePickerStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> DatePickerStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<DatePicker<VM>> for Element<VM> {
    fn from(picker: DatePicker<VM>) -> Self {
        let DatePicker {
            controller,
            selected,
            display_month,
            open,
            disabled,
            validation,
            placeholder,
            on_change,
            on_month_change,
            on_open_change,
            style,
        } = picker;
        let style = resolve_input_control_style(style.as_ref(), DatePickerStyle::default_for_theme);

        let parse_controller = controller.clone();
        let typed_change = on_change.clone().map(|command| {
            ValueCommand::new_with_context(move |vm, _: TextChangeSet, ctx| {
                let text = parse_controller.text();
                command.execute_with_context(
                    vm,
                    DatePickerChange {
                        date: NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok(),
                        text,
                    },
                    ctx,
                );
            })
        });

        let trigger = picker_input_trigger(
            controller.clone(),
            placeholder,
            validation,
            disabled.clone(),
            style.width,
            ICON_CALENDAR,
            open.clone(),
            on_open_change.clone(),
            typed_change,
        );

        let calendar_command = {
            let controller = controller.clone();
            let on_change = on_change.clone();
            let on_month_change = on_month_change.clone();
            let on_open_change = on_open_change.clone();
            ValueCommand::new_with_context(move |vm, change: CalendarSelectionChange, ctx| {
                match change.trigger {
                    CalendarChangeTrigger::PreviousMonth | CalendarChangeTrigger::NextMonth => {
                        if let Some(command) = on_month_change.as_ref() {
                            command.execute_with_context(vm, change.display_month, ctx);
                        }
                    }
                    _ => {
                        let text = format_date(change.date);
                        controller.set_text(text.clone());
                        if let Some(command) = on_change.as_ref() {
                            command.execute_with_context(
                                vm,
                                DatePickerChange {
                                    date: Some(change.date),
                                    text,
                                },
                                ctx,
                            );
                        }
                        if let Some(command) = on_open_change.as_ref() {
                            command.execute_with_context(vm, false, ctx);
                        }
                    }
                }
            })
        };

        let content = Calendar::new(display_month, selected)
            .style_full(move |_| style.calendar.clone())
            .disable(disabled.clone())
            .on_change(calendar_command)
            .unframed();
        let popover = Popover::new(trigger)
            .content(picker_popover_content(content))
            .open(open)
            .disable(disabled);
        if let Some(command) = on_open_change {
            popover.on_open_change(command).into()
        } else {
            popover.into()
        }
    }
}

pub struct TimePicker<VM> {
    controller: TextController,
    selected: Value<Option<NaiveTime>>,
    open: Value<bool>,
    disabled: Value<bool>,
    validation: Value<ValidationVisualState>,
    placeholder: Value<String>,
    minute_step: u32,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<TimePickerStyle>>,
}

impl<VM> TimePicker<VM> {
    pub fn new(
        controller: impl Into<TextController>,
        selected: impl Into<Value<Option<NaiveTime>>>,
    ) -> Self {
        Self {
            controller: controller.into(),
            selected: selected.into(),
            open: Value::Static(false),
            disabled: Value::Static(false),
            validation: Value::Static(ValidationVisualState::default()),
            placeholder: Value::Static("Select time".to_string()),
            minute_step: 1,
            on_change: None,
            on_open_change: None,
            style: None,
        }
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
        self
    }

    pub fn minute_step(mut self, step: u32) -> Self {
        self.minute_step = step.clamp(1, 60);
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn validation(mut self, validation: impl Into<Value<ValidationVisualState>>) -> Self {
        self.validation = validation.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, TimePickerChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut TimePickerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| TimePickerStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> TimePickerStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<TimePicker<VM>> for Element<VM> {
    fn from(picker: TimePicker<VM>) -> Self {
        let TimePicker {
            controller,
            selected,
            open,
            disabled,
            validation,
            placeholder,
            minute_step,
            on_change,
            on_open_change,
            style,
        } = picker;
        let style = resolve_input_control_style(style.as_ref(), TimePickerStyle::default_for_theme);

        let parse_controller = controller.clone();
        let typed_change = on_change.clone().map(|command| {
            ValueCommand::new_with_context(move |vm, _: TextChangeSet, ctx| {
                let text = parse_controller.text();
                command.execute_with_context(
                    vm,
                    TimePickerChange {
                        time: parse_time(&text),
                        text,
                    },
                    ctx,
                );
            })
        });

        let trigger = picker_input_trigger(
            controller.clone(),
            placeholder,
            validation,
            disabled.clone(),
            style.width,
            ICON_TIME,
            open.clone(),
            on_open_change.clone(),
            typed_change,
        );

        let content = time_picker_content(
            controller,
            selected.resolve(),
            minute_step,
            disabled.resolve(),
            on_change,
            on_open_change.clone(),
            style,
        );
        let popover = Popover::new(trigger)
            .content(picker_popover_content(content))
            .open(open)
            .disable(disabled);
        if let Some(command) = on_open_change {
            popover.on_open_change(command).into()
        } else {
            popover.into()
        }
    }
}

pub struct NumberInput<VM> {
    controller: TextController,
    value: Value<Option<f64>>,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
    disabled: Value<bool>,
    validation: Value<ValidationVisualState>,
    placeholder: Value<String>,
    on_change: Option<ValueCommand<VM, NumberInputChange>>,
    style: Option<StyleResolver<NumberInputStyle>>,
}

impl<VM> NumberInput<VM> {
    pub fn new(
        controller: impl Into<TextController>,
        value: impl Into<Value<Option<f64>>>,
    ) -> Self {
        Self {
            controller: controller.into(),
            value: value.into(),
            min: None,
            max: None,
            step: 1.0,
            disabled: Value::Static(false),
            validation: Value::Static(ValidationVisualState::default()),
            placeholder: Value::Static("0".to_string()),
            on_change: None,
            style: None,
        }
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        };
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn validation(mut self, validation: impl Into<Value<ValidationVisualState>>) -> Self {
        self.validation = validation.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, NumberInputChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut NumberInputStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| NumberInputStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> NumberInputStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<NumberInput<VM>> for Element<VM> {
    fn from(input: NumberInput<VM>) -> Self {
        let NumberInput {
            controller,
            value,
            min,
            max,
            step,
            disabled,
            validation,
            placeholder,
            on_change,
            style,
        } = input;
        let style =
            resolve_input_control_style(style.as_ref(), NumberInputStyle::default_for_theme);

        let text_controller = controller.clone();
        let typed_change = on_change.clone().map(|command| {
            ValueCommand::new_with_context(move |vm, _: TextChangeSet, ctx| {
                let text = text_controller.text();
                command.execute_with_context(
                    vm,
                    NumberInputChange {
                        value: parse_number(&text, min, max),
                        text,
                        trigger: NumberInputChangeTrigger::Text,
                    },
                    ctx,
                );
            })
        });

        let mut field = Input::new(controller.clone())
            .width(style.width)
            .placeholder(placeholder)
            .validation(validation)
            .disable(disabled.clone());
        if let Some(command) = typed_change {
            field = field.on_change_set(command);
        }

        let minus = number_step_button(
            ICON_REMOVE,
            NumberInputChangeTrigger::StepDown,
            -step,
            controller.clone(),
            value.clone(),
            min,
            max,
            disabled.clone(),
            on_change.clone(),
            style.button_width,
        );
        let plus = number_step_button(
            ICON_ADD,
            NumberInputChangeTrigger::StepUp,
            step,
            controller,
            value,
            min,
            max,
            disabled.clone(),
            on_change,
            style.button_width,
        );

        Flex::horizontal()
            .align(Align::Center)
            .gap(dp(6.0))
            .child(minus)
            .child(field)
            .child(plus)
            .into()
    }
}

pub struct ColorPicker<VM> {
    color: Value<Color>,
    open: Value<bool>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, ColorPickerChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    swatches: Vec<Color>,
    style: Option<StyleResolver<ColorPickerStyle>>,
}

impl<VM> ColorPicker<VM> {
    pub fn new(color: impl Into<Value<Color>>) -> Self {
        Self {
            color: color.into(),
            open: Value::Static(false),
            disabled: Value::Static(false),
            on_change: None,
            on_open_change: None,
            swatches: default_swatches(),
            style: None,
        }
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, ColorPickerChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn swatches(mut self, swatches: Vec<Color>) -> Self {
        self.swatches = swatches;
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut ColorPickerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| ColorPickerStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ColorPickerStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<ColorPicker<VM>> for Element<VM> {
    fn from(picker: ColorPicker<VM>) -> Self {
        let ColorPicker {
            color,
            open,
            disabled,
            on_change,
            on_open_change,
            swatches,
            style,
        } = picker;
        let style =
            resolve_input_control_style(style.as_ref(), ColorPickerStyle::default_for_theme);
        let trigger = color_picker_trigger(
            color.clone(),
            disabled.clone(),
            open.clone(),
            on_open_change.clone(),
            style.clone(),
        );
        let content = picker_popover_content(color_picker_content(
            color,
            disabled.resolve(),
            on_change,
            swatches,
            style,
        ));
        let popover = Popover::new(trigger)
            .content(content)
            .open(open)
            .disable(disabled);
        if let Some(command) = on_open_change {
            popover.on_open_change(command).into()
        } else {
            popover.into()
        }
    }
}

pub struct Upload<VM> {
    files: Value<Vec<UploadFile>>,
    disabled: Value<bool>,
    accept_extensions: Vec<String>,
    max_files: Option<usize>,
    max_file_size: Option<u64>,
    title: Value<String>,
    hint: Value<String>,
    on_select: Option<ValueCommand<VM, UploadSelection>>,
    on_remove: Option<ValueCommand<VM, UploadRemove>>,
    style: Option<StyleResolver<UploadStyle>>,
}

impl<VM> Upload<VM> {
    pub fn new(files: impl Into<Value<Vec<UploadFile>>>) -> Self {
        Self {
            files: files.into(),
            disabled: Value::Static(false),
            accept_extensions: Vec::new(),
            max_files: None,
            max_file_size: None,
            title: Value::Static("Drop files here".to_string()),
            hint: Value::Static("or choose files".to_string()),
            on_select: None,
            on_remove: None,
            style: None,
        }
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn accept_extensions(mut self, extensions: &[&str]) -> Self {
        self.accept_extensions = extensions
            .iter()
            .map(|extension| normalize_extension(extension))
            .collect();
        self
    }

    pub fn max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files);
        self
    }

    pub fn max_file_size(mut self, bytes: u64) -> Self {
        self.max_file_size = Some(bytes);
        self
    }

    pub fn title(mut self, title: impl Into<Value<String>>) -> Self {
        self.title = title.into();
        self
    }

    pub fn hint(mut self, hint: impl Into<Value<String>>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn on_select(mut self, command: ValueCommand<VM, UploadSelection>) -> Self {
        self.on_select = Some(command);
        self
    }

    pub fn on_remove(mut self, command: ValueCommand<VM, UploadRemove>) -> Self {
        self.on_remove = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut UploadStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| UploadStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> UploadStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<Upload<VM>> for Element<VM> {
    fn from(upload: Upload<VM>) -> Self {
        let Upload {
            files,
            disabled,
            accept_extensions,
            max_files,
            max_file_size,
            title,
            hint,
            on_select,
            on_remove,
            style,
        } = upload;
        let style = resolve_input_control_style(style.as_ref(), UploadStyle::default_for_theme);

        let disabled_for_click = disabled.clone();
        let dialog_command = {
            let accept_extensions = accept_extensions.clone();
            let on_select = on_select.clone();
            let files = files.clone();
            Command::new_with_context(move |_vm, ctx| {
                if disabled_for_click.resolve() {
                    return;
                }
                let mut options = FileDialogOptions::new().title("Choose files");
                if !accept_extensions.is_empty() {
                    options = options.add_filter("Accepted", &accept_extensions);
                }
                let callback = on_select.clone();
                let accept = accept_extensions.clone();
                let files_for_callback = files.clone();
                let _ =
                    ctx.dialogs().open_files_async(
                        options,
                        ValueCommand::new(
                            move |vm: &mut VM,
                                  result: Result<
                                Option<Vec<PathBuf>>,
                                crate::dialog::DialogError,
                            >| {
                                if let Some(command) = callback.as_ref() {
                                    match result {
                                        Ok(Some(paths)) => command.execute(
                                            vm,
                                            validate_upload_paths(
                                                paths,
                                                &accept,
                                                max_files,
                                                max_file_size,
                                                files_for_callback.resolve().len(),
                                            ),
                                        ),
                                        Ok(None) => {}
                                        Err(error) => command.execute(
                                            vm,
                                            UploadSelection {
                                                files: Vec::new(),
                                                rejected: vec![UploadRejection {
                                                    path: PathBuf::new(),
                                                    reason: error.to_string(),
                                                }],
                                            },
                                        ),
                                    }
                                }
                            },
                        ),
                    );
            })
        };

        let drop_command = on_select.clone().map(|command| {
            let accept = accept_extensions.clone();
            let disabled = disabled.clone();
            let files = files.clone();
            ValueCommand::new(move |vm: &mut VM, event: FileDropEvent| {
                if !disabled.resolve() {
                    command.execute(
                        vm,
                        validate_upload_paths(
                            event.paths,
                            &accept,
                            max_files,
                            max_file_size,
                            files.resolve().len(),
                        ),
                    );
                }
            })
        });

        let mut drop_zone = Flex::vertical()
            .width(style.width)
            .min_height(dp(136.0))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(8.0))
            .center()
            .style_full(input_panel_style)
            .cursor(CursorStyle::Pointer)
            .on_click(dialog_command.clone())
            .child(upload_badge::<VM>())
            .child(Text::new(title).style_full(label_text_style))
            .child(Text::new(hint).style_full(muted_text_style))
            .child(
                Button::new("Choose files")
                    .secondary()
                    .disable(disabled.clone())
                    .on_click(dialog_command),
            );
        if let Some(command) = drop_command {
            drop_zone = drop_zone.on_file_drop(command);
        }

        let list = build_upload_list(
            files.resolve_untracked(),
            on_remove,
            disabled.clone(),
            style.clone(),
        );

        Flex::vertical()
            .width(style.width)
            .gap(dp(10.0))
            .child(drop_zone)
            .child(list)
            .into()
    }
}

fn picker_input_trigger<VM: 'static>(
    controller: TextController,
    placeholder: Value<String>,
    validation: Value<ValidationVisualState>,
    disabled: Value<bool>,
    width: Dp,
    icon: SvgIconId,
    open: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
) -> Element<VM> {
    let toggle = open_toggle_command(open, disabled.clone(), on_open_change);
    let gap = dp(6.0);
    let input_width = (width - FIELD_HEIGHT - gap).max(dp(120.0));
    let mut input = Input::new(controller)
        .width(input_width)
        .height(FIELD_HEIGHT)
        .placeholder(placeholder)
        .validation(validation)
        .disable(disabled.clone());
    if let Some(command) = on_change_set {
        input = input.on_change_set(command);
    }
    if let Some(command) = toggle.clone() {
        input = input.on_click(command);
    }

    let icon: Element<VM> = if let Some(command) = toggle {
        secondary_icon_button(icon, FIELD_HEIGHT, FIELD_HEIGHT, disabled, command)
    } else {
        Stack::new()
            .size(FIELD_HEIGHT, FIELD_HEIGHT)
            .center()
            .style_full(input_icon_surface_style)
            .opacity(disabled_opacity(disabled))
            .child(styled_icon(icon, dp(20.0), |context| {
                let (_, _, _, muted, _, _, _) = mode_colors(context);
                muted
            }))
            .into()
    };

    Flex::horizontal()
        .width(width)
        .height(FIELD_HEIGHT)
        .align(Align::Center)
        .gap(gap)
        .child(input)
        .child(icon)
        .into()
}

fn color_picker_trigger<VM: 'static>(
    color: Value<Color>,
    disabled: Value<bool>,
    open: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: ColorPickerStyle,
) -> Element<VM> {
    let toggle = open_toggle_command(open, disabled.clone(), on_open_change);
    let mut button = Button::new(color_label(color.clone()))
        .width(style.width)
        .height(FIELD_HEIGHT)
        .style_full(color_trigger_accessible_button_style)
        .disable(disabled.clone());
    if let Some(command) = toggle.clone() {
        button = button.on_click(command);
    }

    let mut overlay = Flex::horizontal()
        .width(style.width)
        .height(FIELD_HEIGHT)
        .padding(Insets::symmetric(dp(12.0), dp(0.0)))
        .align(Align::Center)
        .gap(dp(10.0))
        .style_full(input_control_shell_style)
        .cursor(if disabled.resolve() {
            CursorStyle::NotAllowed
        } else {
            CursorStyle::Pointer
        })
        .opacity(disabled_opacity(disabled))
        .child(themed_icon(ICON_COLOR, dp(18.0)))
        .child(color_preview_box::<VM>(color.clone(), dp(24.0)))
        .child(
            Text::new(color_label(color))
                .grow(1.0)
                .style_full(label_text_style),
        )
        .child(themed_icon(ICON_EXPAND, dp(20.0)));
    if let Some(command) = toggle {
        overlay = overlay.on_click(command);
    }

    Stack::new()
        .width(style.width)
        .height(FIELD_HEIGHT)
        .child(button)
        .child(overlay)
        .into()
}

fn open_toggle_command<VM: 'static>(
    open: Value<bool>,
    disabled: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
) -> Option<Command<VM>> {
    on_open_change.map(|command| {
        Command::new_with_context(move |vm, ctx| {
            if disabled.resolve() {
                return;
            }
            command.execute_with_context(vm, !open.resolve(), ctx);
        })
    })
}

fn themed_icon<VM: 'static>(icon: SvgIconId, size: Dp) -> Icon<VM> {
    Icon::internal(icon)
        .size(size)
        .style(move |style, context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            style.color = Value::Static(muted);
            style.size = size;
        })
}

fn styled_icon<VM: 'static>(
    icon: SvgIconId,
    size: Dp,
    color: impl Fn(&StyleContext<'_>) -> Color + Send + Sync + 'static,
) -> Icon<VM> {
    Icon::internal(icon)
        .size(size)
        .style(move |style, context| {
            style.color = Value::Static(color(context));
            style.size = size;
        })
}

fn ghost_icon_button<VM: 'static>(
    icon: SvgIconId,
    size: Dp,
    disabled: impl Into<Value<bool>>,
    command: Command<VM>,
) -> Element<VM> {
    let disabled = disabled.into();
    Stack::new()
        .size(size, size)
        .center()
        .style_full(icon_surface_style)
        .opacity(disabled_opacity(disabled.clone()))
        .cursor(if disabled.resolve() {
            CursorStyle::NotAllowed
        } else {
            CursorStyle::Pointer
        })
        .child(styled_icon(icon, dp(20.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .on_click(guard_disabled_command(disabled, command))
        .into()
}

fn secondary_icon_button<VM: 'static>(
    icon: SvgIconId,
    width: Dp,
    height: Dp,
    disabled: impl Into<Value<bool>>,
    command: Command<VM>,
) -> Element<VM> {
    let disabled = disabled.into();
    Stack::new()
        .width(width)
        .height(height)
        .center()
        .style_full(input_icon_surface_style)
        .opacity(disabled_opacity(disabled.clone()))
        .cursor(if disabled.resolve() {
            CursorStyle::NotAllowed
        } else {
            CursorStyle::Pointer
        })
        .child(styled_icon(icon, dp(20.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .on_click(guard_disabled_command(disabled, command))
        .into()
}

fn guard_disabled_command<VM: 'static>(disabled: Value<bool>, command: Command<VM>) -> Command<VM> {
    Command::new_with_context(move |vm, ctx| {
        if !disabled.resolve() {
            command.execute_with_context(vm, ctx);
        }
    })
}

fn color_preview_box<VM: 'static>(color: Value<Color>, size: Dp) -> Element<VM> {
    Flex::vertical()
        .size(size, size)
        .style_full(move |context| color_preview_value_style(context, color.clone()))
        .into()
}

fn upload_badge<VM: 'static>() -> Element<VM> {
    let icon_box = dp(30.0);
    Flex::vertical()
        .size(dp(44.0), dp(44.0))
        .shrink(0.0)
        .center()
        .style_full(accent_badge_style)
        .child(
            Icon::internal(ICON_UPLOAD)
                .size(icon_box)
                .style(move |style, context| {
                    let (_, primary, _, _, _, _, _) = mode_colors(context);
                    style.color = Value::Static(primary);
                    style.size = icon_box;
                }),
        )
        .into()
}

fn file_badge<VM: 'static>() -> Element<VM> {
    Flex::vertical()
        .size(dp(34.0), dp(34.0))
        .center()
        .style_full(subtle_badge_style)
        .child(themed_icon(ICON_FILE, dp(20.0)))
        .into()
}

fn disabled_opacity(disabled: Value<bool>) -> Value<f32> {
    match disabled {
        Value::Static(disabled) => Value::Static(if disabled { 0.56 } else { 1.0 }),
        Value::Signal(disabled) => {
            Value::Signal(disabled.map(|disabled| if disabled { 0.56 } else { 1.0 }))
        }
    }
}

fn calendar_element<VM: 'static>(
    display_month: NaiveDate,
    selected: Option<NaiveDate>,
    today: Option<NaiveDate>,
    disabled: bool,
    on_change: Option<ValueCommand<VM, CalendarSelectionChange>>,
    style: CalendarStyle,
    framed: bool,
) -> Element<VM> {
    let month = month_start(display_month);
    let mut root = Flex::vertical().width(style.panel_width).gap(dp(10.0));
    if framed {
        root = root
            .padding(Insets::all(PANEL_PADDING))
            .style_full(panel_style);
    }

    root = root.child(
        Flex::horizontal()
            .height(dp(36.0))
            .align(Align::Center)
            .justify(Justify::SpaceBetween)
            .child(calendar_nav_button(
                ICON_PREVIOUS,
                add_months(month, -1),
                CalendarChangeTrigger::PreviousMonth,
                disabled,
                on_change.clone(),
            ))
            .child(
                Text::new(format!(
                    "{} {}",
                    MONTHS[month.month0() as usize],
                    month.year()
                ))
                .style_full(label_text_style),
            )
            .child(calendar_nav_button(
                ICON_NEXT,
                add_months(month, 1),
                CalendarChangeTrigger::NextMonth,
                disabled,
                on_change.clone(),
            )),
    );

    let mut weekday_row = Grid::columns([
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
    ])
    .gap(style.gap);
    for label in WEEKDAYS {
        weekday_row = weekday_row.child(
            Flex::<VM>::vertical()
                .size(style.day_size, dp(22.0))
                .center()
                .child(Text::new(label).style_full(calendar_weekday_text_style)),
        );
    }
    root = root.child(weekday_row);

    let mut days = Grid::columns([
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
    ])
    .gap(style.gap);
    for date in calendar_days(month) {
        let same_month = date.month() == month.month();
        let is_selected = selected == Some(date);
        let is_today = today == Some(date);
        let button = Button::new(date.day().to_string())
            .size(style.day_size, style.day_size)
            .style_full(move |context| {
                calendar_day_button_style(context, is_selected, is_today, same_month)
            })
            .disable(disabled);
        let command = on_change.clone();
        days = days.child(button.on_click(Command::new_with_context(move |vm, ctx| {
            if let Some(command) = command.as_ref() {
                command.execute_with_context(
                    vm,
                    CalendarSelectionChange {
                        date,
                        display_month: month_start(date),
                        trigger: CalendarChangeTrigger::Day,
                    },
                    ctx,
                );
            }
        })));
    }
    root = root.child(days);
    if let Some(today) = today {
        root = root.child(
            Button::new("Today")
                .secondary()
                .height(dp(32.0))
                .style_full(today_button_style)
                .disable(disabled)
                .on_click(Command::new_with_context(move |vm, ctx| {
                    if let Some(command) = on_change.as_ref() {
                        command.execute_with_context(
                            vm,
                            CalendarSelectionChange {
                                date: today,
                                display_month: month_start(today),
                                trigger: CalendarChangeTrigger::Today,
                            },
                            ctx,
                        );
                    }
                })),
        );
    }
    root.into()
}

fn calendar_nav_button<VM: 'static>(
    icon: SvgIconId,
    display_month: NaiveDate,
    trigger: CalendarChangeTrigger,
    disabled: bool,
    on_change: Option<ValueCommand<VM, CalendarSelectionChange>>,
) -> Element<VM> {
    ghost_icon_button(
        icon,
        dp(32.0),
        Value::Static(disabled),
        Command::new_with_context(move |vm, ctx| {
            if let Some(command) = on_change.as_ref() {
                command.execute_with_context(
                    vm,
                    CalendarSelectionChange {
                        date: display_month,
                        display_month,
                        trigger,
                    },
                    ctx,
                );
            }
        }),
    )
}

fn time_picker_content<VM: 'static>(
    controller: TextController,
    selected: Option<NaiveTime>,
    minute_step: u32,
    disabled: bool,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: TimePickerStyle,
) -> Element<VM> {
    let current = selected
        .or_else(|| parse_time(&controller.text()))
        .unwrap_or(NaiveTime::MIN);
    let minute_values = minute_values_for_step(minute_step);
    let hour_values = (0..24).collect::<Vec<_>>();
    let hour_index = value_index(&hour_values, current.hour());
    let minute_index = value_index(&minute_values, current.minute());
    let hour = hour_values[hour_index];
    let minute = minute_values[minute_index];
    let column_width = style.option_width.max(dp(96.0));
    let hour_column = time_wheel_column(
        "Hour",
        TimePickerUnit::Hour,
        &hour_values,
        hour_index,
        hour,
        minute,
        controller.clone(),
        disabled,
        on_change.clone(),
        column_width,
    );
    let minute_column = time_wheel_column(
        "Minute",
        TimePickerUnit::Minute,
        &minute_values,
        minute_index,
        hour,
        minute,
        controller,
        disabled,
        on_change,
        column_width,
    );

    let mut done_button = Button::new("Done")
        .primary()
        .width(style.width)
        .height(dp(36.0))
        .disable(disabled || on_open_change.is_none());
    if let Some(command) = on_open_change {
        done_button = done_button.on_click(Command::new_with_context(move |vm, ctx| {
            command.execute_with_context(vm, false, ctx);
        }));
    }

    Flex::vertical()
        .width(style.width)
        .gap(dp(10.0))
        .child(
            Flex::horizontal()
                .align(Align::Center)
                .gap(dp(8.0))
                .child(themed_icon(ICON_TIME, dp(18.0)))
                .child(Text::new("Select time").style_full(label_text_style)),
        )
        .child(
            Flex::horizontal()
                .width(style.width)
                .align(Align::Center)
                .justify(Justify::Center)
                .gap(dp(12.0))
                .child(hour_column)
                .child(Text::new(":").style_full(time_wheel_separator_style))
                .child(minute_column),
        )
        .child(done_button)
        .into()
}

#[derive(Clone, Copy)]
enum TimePickerUnit {
    Hour,
    Minute,
}

fn time_wheel_column<VM: 'static>(
    label: &'static str,
    unit: TimePickerUnit,
    values: &[u32],
    selected_index: usize,
    current_hour: u32,
    current_minute: u32,
    controller: TextController,
    disabled: bool,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    width: Dp,
) -> Element<VM> {
    let previous_index = previous_index(selected_index, values.len());
    let next_index = next_index(selected_index, values.len());
    let previous = values[previous_index];
    let selected = values[selected_index];
    let next = values[next_index];
    Flex::vertical()
        .width(width)
        .align(Align::Center)
        .gap(dp(6.0))
        .child(Text::new(label).style_full(muted_text_style))
        .child(ghost_icon_button(
            ICON_UP,
            dp(32.0),
            disabled,
            time_wheel_select_command(
                unit,
                previous,
                current_hour,
                current_minute,
                controller.clone(),
                disabled,
                on_change.clone(),
            ),
        ))
        .child(time_wheel_value_button(
            previous,
            false,
            width,
            disabled,
            time_wheel_select_command(
                unit,
                previous,
                current_hour,
                current_minute,
                controller.clone(),
                disabled,
                on_change.clone(),
            ),
        ))
        .child(time_wheel_value_button(
            selected,
            true,
            width,
            disabled,
            time_wheel_select_command(
                unit,
                selected,
                current_hour,
                current_minute,
                controller.clone(),
                disabled,
                on_change.clone(),
            ),
        ))
        .child(time_wheel_value_button(
            next,
            false,
            width,
            disabled,
            time_wheel_select_command(
                unit,
                next,
                current_hour,
                current_minute,
                controller.clone(),
                disabled,
                on_change.clone(),
            ),
        ))
        .child(ghost_icon_button(
            ICON_DOWN,
            dp(32.0),
            disabled,
            time_wheel_select_command(
                unit,
                next,
                current_hour,
                current_minute,
                controller,
                disabled,
                on_change,
            ),
        ))
        .into()
}

fn time_wheel_value_button<VM: 'static>(
    value: u32,
    selected: bool,
    width: Dp,
    disabled: bool,
    command: Command<VM>,
) -> Button<VM> {
    let button = Button::new(format!("{value:02}"))
        .width(width)
        .height(if selected { dp(44.0) } else { dp(34.0) })
        .disable(disabled)
        .on_click(command);
    if selected {
        button
            .primary()
            .style_full(time_wheel_selected_button_style)
    } else {
        button
            .secondary()
            .style_full(time_wheel_option_button_style)
    }
}

fn time_wheel_select_command<VM: 'static>(
    unit: TimePickerUnit,
    value: u32,
    current_hour: u32,
    current_minute: u32,
    controller: TextController,
    disabled: bool,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
) -> Command<VM> {
    Command::new_with_context(move |vm, ctx| {
        if disabled {
            return;
        }
        let (hour, minute) = match unit {
            TimePickerUnit::Hour => (value, current_minute),
            TimePickerUnit::Minute => (current_hour, value),
        };
        let time = NaiveTime::from_hms_opt(hour, minute, 0).unwrap_or(NaiveTime::MIN);
        let text = format_time(time);
        controller.set_text(text.clone());
        if let Some(command) = on_change.as_ref() {
            command.execute_with_context(
                vm,
                TimePickerChange {
                    time: Some(time),
                    text,
                },
                ctx,
            );
        }
    })
}

fn minute_values_for_step(step: u32) -> Vec<u32> {
    let step = step.clamp(1, 60);
    let mut values = Vec::new();
    let mut minute = 0u32;
    while minute < 60 {
        values.push(minute);
        minute += step;
    }
    values
}

fn value_index(values: &[u32], value: u32) -> usize {
    values
        .iter()
        .position(|item| *item == value)
        .or_else(|| {
            values
                .iter()
                .enumerate()
                .min_by_key(|(_, item)| item.abs_diff(value))
                .map(|(index, _)| index)
        })
        .unwrap_or(0)
}

fn previous_index(index: usize, len: usize) -> usize {
    if len == 0 || index == 0 {
        len.saturating_sub(1)
    } else {
        index - 1
    }
}

fn next_index(index: usize, len: usize) -> usize {
    if len == 0 || index + 1 >= len {
        0
    } else {
        index + 1
    }
}

fn number_step_button<VM: 'static>(
    icon: SvgIconId,
    trigger: NumberInputChangeTrigger,
    delta: f64,
    controller: TextController,
    value: Value<Option<f64>>,
    min: Option<f64>,
    max: Option<f64>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, NumberInputChange>>,
    width: Dp,
) -> Element<VM> {
    secondary_icon_button(
        icon,
        width,
        FIELD_HEIGHT,
        disabled,
        Command::new_with_context(move |vm, ctx| {
            let current = parse_number(&controller.text(), min, max)
                .or_else(|| value.resolve())
                .unwrap_or(0.0);
            let next = clamp_number(current + delta, min, max);
            let text = format_number(next);
            controller.set_text(text.clone());
            if let Some(command) = on_change.as_ref() {
                command.execute_with_context(
                    vm,
                    NumberInputChange {
                        value: Some(next),
                        text,
                        trigger,
                    },
                    ctx,
                );
            }
        }),
    )
}

fn color_picker_content<VM: 'static>(
    color: Value<Color>,
    disabled: bool,
    on_change: Option<ValueCommand<VM, ColorPickerChange>>,
    swatches: Vec<Color>,
    style: ColorPickerStyle,
) -> Element<VM> {
    let mut root = Flex::vertical().width(style.width).gap(dp(12.0));
    root = root.child(
        Flex::horizontal()
            .align(Align::Center)
            .gap(dp(10.0))
            .child(color_preview_box::<VM>(color.clone(), dp(44.0)))
            .child(
                Flex::vertical()
                    .gap(dp(2.0))
                    .child(Text::new("Current color").style_full(muted_text_style))
                    .child(Text::new(color_label(color.clone())).style_full(label_text_style)),
            ),
    );

    let mut swatch_row = Flex::horizontal().wrap(Wrap::Wrap).gap(dp(8.0));
    for swatch in swatches {
        let command = on_change.clone();
        swatch_row = swatch_row.child(
            Flex::<VM>::vertical()
                .size(style.swatch_size, style.swatch_size)
                .style_full(move |context| color_swatch_style(context, swatch))
                .cursor(if disabled {
                    CursorStyle::NotAllowed
                } else {
                    CursorStyle::Pointer
                })
                .on_click(Command::new_with_context(move |vm, ctx| {
                    if !disabled {
                        if let Some(command) = command.as_ref() {
                            command.execute_with_context(
                                vm,
                                ColorPickerChange {
                                    color: swatch,
                                    trigger: ColorPickerChangeTrigger::Swatch,
                                },
                                ctx,
                            );
                        }
                    }
                })),
        );
    }
    root = root.child(swatch_row);
    root = root.child(color_slider(
        "R",
        color.clone(),
        ColorPickerChangeTrigger::Red,
        disabled,
        on_change.clone(),
    ));
    root = root.child(color_slider(
        "G",
        color.clone(),
        ColorPickerChangeTrigger::Green,
        disabled,
        on_change.clone(),
    ));
    root = root.child(color_slider(
        "B",
        color.clone(),
        ColorPickerChangeTrigger::Blue,
        disabled,
        on_change.clone(),
    ));
    root = root.child(color_slider(
        "A",
        color,
        ColorPickerChangeTrigger::Alpha,
        disabled,
        on_change,
    ));
    root.into()
}

fn color_slider<VM: 'static>(
    label: &'static str,
    color: Value<Color>,
    trigger: ColorPickerChangeTrigger,
    disabled: bool,
    on_change: Option<ValueCommand<VM, ColorPickerChange>>,
) -> Element<VM> {
    let value = color_channel_value(color.clone(), trigger);
    let color_for_change = color.clone();
    Flex::horizontal()
        .align(Align::Center)
        .gap(dp(8.0))
        .child(
            Text::new(label)
                .width(dp(18.0))
                .style_full(label_text_style),
        )
        .child(
            Slider::new(value, 0.0, 255.0)
                .step(1.0)
                .width(dp(196.0))
                .disable(disabled)
                .on_change(ValueCommand::new_with_context(move |vm, next: f32, ctx| {
                    if let Some(command) = on_change.as_ref() {
                        let mut current = color_for_change.resolve();
                        let channel = next.round().clamp(0.0, 255.0) as u8;
                        match trigger {
                            ColorPickerChangeTrigger::Red => current.r = channel,
                            ColorPickerChangeTrigger::Green => current.g = channel,
                            ColorPickerChangeTrigger::Blue => current.b = channel,
                            ColorPickerChangeTrigger::Alpha => current.a = channel,
                            ColorPickerChangeTrigger::Swatch => {}
                        }
                        command.execute_with_context(
                            vm,
                            ColorPickerChange {
                                color: current,
                                trigger,
                            },
                            ctx,
                        );
                    }
                })),
        )
        .child(
            Text::new(color_channel_label(color, trigger))
                .width(dp(32.0))
                .style_full(muted_text_style),
        )
        .into()
}

fn picker_popover_content<VM: 'static>(content: impl Into<Element<VM>>) -> Element<VM> {
    let content = content.into();
    Stack::new()
        .style_full(picker_popover_content_style)
        .child(content)
        .into()
}

fn build_upload_list<VM: 'static>(
    files: Vec<UploadFile>,
    on_remove: Option<ValueCommand<VM, UploadRemove>>,
    disabled: Value<bool>,
    style: UploadStyle,
) -> Element<VM> {
    let mut list = Flex::vertical().width(style.width).gap(dp(8.0));
    for file in files {
        list = list.child(upload_row(
            file,
            on_remove.clone(),
            disabled.clone(),
            style.clone(),
        ));
    }
    list.into()
}

fn upload_row<VM: 'static>(
    file: UploadFile,
    on_remove: Option<ValueCommand<VM, UploadRemove>>,
    disabled: Value<bool>,
    style: UploadStyle,
) -> Element<VM> {
    let id = file.id.clone();
    let remove = on_remove.map(|command| {
        Command::new_with_context(move |vm, ctx| {
            command.execute_with_context(vm, UploadRemove { id: id.clone() }, ctx);
        })
    });
    let status = upload_status_text(&file.status);
    let status_icon = upload_status_icon(&file.status);
    let mut footer = Flex::horizontal()
        .align(Align::Center)
        .justify(Justify::SpaceBetween)
        .child(
            Flex::horizontal()
                .align(Align::Center)
                .gap(dp(6.0))
                .child(themed_icon(status_icon, dp(16.0)))
                .child(Text::new(status).style_full(muted_text_style)),
        );
    if let Some(command) = remove {
        footer = footer.child(ghost_icon_button(ICON_DELETE, dp(32.0), disabled, command));
    }
    Flex::vertical()
        .width(style.width)
        .padding(Insets::all(dp(12.0)))
        .gap(dp(8.0))
        .style_full(input_panel_style)
        .child(
            Flex::horizontal()
                .align(Align::Center)
                .gap(dp(10.0))
                .child(file_badge::<VM>())
                .child(
                    Flex::vertical()
                        .gap(dp(2.0))
                        .child(Text::new(file.name.clone()).style_full(label_text_style))
                        .child(
                            Text::new(format_size(file.size_bytes)).style_full(muted_text_style),
                        ),
                ),
        )
        .child(ProgressBar::<VM>::new(file.progress()).height(dp(8.0)))
        .child(footer)
        .into()
}

fn validate_upload_paths(
    paths: Vec<PathBuf>,
    accept: &[String],
    max_files: Option<usize>,
    max_file_size: Option<u64>,
    existing_count: usize,
) -> UploadSelection {
    let mut files = Vec::new();
    let mut rejected = Vec::new();
    for path in paths {
        if max_files
            .map(|max| existing_count.saturating_add(files.len()) >= max)
            .unwrap_or(false)
        {
            rejected.push(UploadRejection {
                path,
                reason: format!("Only {} file(s) allowed", max_files.unwrap_or(0)),
            });
            continue;
        }
        if !accept.is_empty() && !path_extension_allowed(&path, accept) {
            rejected.push(UploadRejection {
                path,
                reason: "File type is not accepted".to_string(),
            });
            continue;
        }
        let size = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
        if let (Some(size), Some(max_size)) = (size, max_file_size) {
            if size > max_size {
                rejected.push(UploadRejection {
                    path,
                    reason: format!("File exceeds {}", format_size(Some(max_size))),
                });
                continue;
            }
        }
        let mut file = UploadFile::from_path(path);
        file.size_bytes = size;
        files.push(file);
    }
    UploadSelection { files, rejected }
}

fn month_start(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month0() as i32 + months;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date)
}

fn calendar_days(month: NaiveDate) -> Vec<NaiveDate> {
    let start = month - Duration::days(month.weekday().num_days_from_monday() as i64);
    (0..42)
        .map(|offset| start + Duration::days(offset))
        .collect()
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn parse_time(text: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(text.trim(), "%H:%M").ok()
}

fn format_time(time: NaiveTime) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn parse_number(text: &str, min: Option<f64>, max: Option<f64>) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| clamp_number(value, min, max))
}

fn clamp_number(mut value: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    if let Some(min) = min {
        value = value.max(min);
    }
    if let Some(max) = max {
        value = value.min(max);
    }
    value
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn color_label(color: Value<Color>) -> Value<String> {
    match color {
        Value::Static(color) => Value::Static(format_color(color)),
        Value::Signal(color) => Value::Signal(color.map(format_color)),
    }
}

fn color_channel_value(color: Value<Color>, channel: ColorPickerChangeTrigger) -> Value<f32> {
    let read = move |color: Color| match channel {
        ColorPickerChangeTrigger::Red => color.r as f32,
        ColorPickerChangeTrigger::Green => color.g as f32,
        ColorPickerChangeTrigger::Blue => color.b as f32,
        ColorPickerChangeTrigger::Alpha => color.a as f32,
        ColorPickerChangeTrigger::Swatch => 0.0,
    };
    match color {
        Value::Static(color) => Value::Static(read(color)),
        Value::Signal(color) => Value::Signal(color.map(read)),
    }
}

fn color_channel_label(color: Value<Color>, channel: ColorPickerChangeTrigger) -> Value<String> {
    let read = move |color: Color| {
        match channel {
            ColorPickerChangeTrigger::Red => color.r,
            ColorPickerChangeTrigger::Green => color.g,
            ColorPickerChangeTrigger::Blue => color.b,
            ColorPickerChangeTrigger::Alpha => color.a,
            ColorPickerChangeTrigger::Swatch => 0,
        }
        .to_string()
    };
    match color {
        Value::Static(color) => Value::Static(read(color)),
        Value::Signal(color) => Value::Signal(color.map(read)),
    }
}

fn format_color(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

fn default_swatches() -> Vec<Color> {
    vec![
        Color::hexa(0x0F172AFF),
        Color::hexa(0xEF4444FF),
        Color::hexa(0xF97316FF),
        Color::hexa(0xEAB308FF),
        Color::hexa(0x22C55EFF),
        Color::hexa(0x14B8A6FF),
        Color::hexa(0x3B82F6FF),
        Color::hexa(0x8B5CF6FF),
        Color::hexa(0xEC4899FF),
        Color::hexa(0xF8FAFCFF),
    ]
}

fn normalize_extension(extension: &str) -> String {
    extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn path_extension_allowed(path: &Path, accept: &[String]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            accept
                .iter()
                .any(|allowed| allowed == &normalize_extension(extension))
        })
        .unwrap_or(false)
}

fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "unknown size".to_string();
    };
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    if size as f64 >= MB {
        format!("{:.1} MB", size as f64 / MB)
    } else if size as f64 >= KB {
        format!("{:.1} KB", size as f64 / KB)
    } else {
        format!("{size} B")
    }
}

fn upload_status_text(status: &UploadStatus) -> String {
    match status {
        UploadStatus::Queued => "Queued".to_string(),
        UploadStatus::Uploading { progress } => {
            format!("Uploading {:.0}%", progress.clamp(0.0, 1.0) * 100.0)
        }
        UploadStatus::Complete => "Complete".to_string(),
        UploadStatus::Error(message) => format!("Error: {message}"),
    }
}

fn upload_status_icon(status: &UploadStatus) -> SvgIconId {
    match status {
        UploadStatus::Queued | UploadStatus::Uploading { .. } => ICON_PENDING,
        UploadStatus::Complete => ICON_DONE,
        UploadStatus::Error(_) => ICON_ERROR,
    }
}

fn value_color(
    normal: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> StateValue<Value<Color>> {
    StateValue::interactive(
        Value::Static(normal),
        Value::Static(hovered),
        Value::Static(pressed),
        Value::Static(disabled),
    )
}

fn mode_colors(context: &StyleContext<'_>) -> (bool, Color, Color, Color, Color, Color, Color) {
    let dark = matches!(context.mode, ResolvedThemeMode::Dark);
    let input = InputStyle::default_for_theme(context.theme);
    let select = SelectStyle::default_for_theme(context.theme);
    let primary_button = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Primary);
    (
        dark,
        primary_button.background.normal.resolve(),
        input.text.normal.resolve(),
        input.placeholder.normal.resolve(),
        select.menu_background.resolve(),
        select.selected_option_background.resolve(),
        input.border.normal.resolve(),
    )
}

fn input_control_shell_style(context: &StyleContext<'_>) -> ContainerStyle {
    let input = InputStyle::default_for_theme(context.theme);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(input.background.normal);
    style.surface.border_color = Some(input.border.normal);
    style.surface.border_width = Some(input.border_width);
    style.surface.border_radius = Some(input.radius);
    style.surface.shadow = None;
    style
}

fn input_panel_style(context: &StyleContext<'_>) -> ContainerStyle {
    let input = InputStyle::default_for_theme(context.theme);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(input.background.normal);
    style.surface.border_color = Some(input.border.normal);
    style.surface.border_width = Some(input.border_width);
    style.surface.border_radius = Some(input.radius);
    style.surface.shadow = None;
    style
}

fn icon_surface_style(context: &StyleContext<'_>) -> ContainerStyle {
    let button = icon_button_style(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(button.background.normal);
    style.surface.border_color = Some(button.border.normal);
    style.surface.border_width = Some(button.border_width);
    style.surface.border_radius = Some(button.radius);
    style.surface.shadow = button.surface.shadow;
    style
}

fn input_icon_surface_style(context: &StyleContext<'_>) -> ContainerStyle {
    let button = input_icon_button_style(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(button.background.normal);
    style.surface.border_color = Some(button.border.normal);
    style.surface.border_width = Some(button.border_width);
    style.surface.border_radius = Some(button.radius);
    style.surface.shadow = button.surface.shadow;
    style
}

fn color_trigger_accessible_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Secondary);
    style.foreground = value_color(
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
    );
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = FIELD_HEIGHT;
    style
}

fn picker_popover_content_style(context: &StyleContext<'_>) -> ContainerStyle {
    let popover = PopoverStyle::default_for_theme(context.theme);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(popover.background);
    style.surface.border_radius = Some(popover.radius);
    style.surface.shadow = None;
    style
}

fn icon_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Ghost);
    style.radius = Value::Static(dp(8.0));
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = dp(32.0);
    style
}

fn input_icon_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Secondary);
    style.radius = InputStyle::default_for_theme(context.theme).radius;
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = FIELD_HEIGHT;
    style
}

fn time_option_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Secondary);
    style.radius = Value::Static(dp(8.0));
    style.padding_x = dp(10.0);
    style.padding_y = dp(4.0);
    style.min_height = dp(34.0);
    style.text_style = TextStyle {
        font_family: None,
        size: sp(13.0),
        line_height: Some(sp(18.0)),
        weight: FontWeight::Medium,
        letter_spacing: Some(sp(0.0)),
    };
    style
}

fn time_option_selected_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let (_, primary, _, muted, _, _, _) = mode_colors(context);
    let mut style = time_option_button_style(context);
    style.background = value_color(primary, primary.lighten(0.08), primary.darken(0.08), muted);
    style.foreground = value_color(Color::WHITE, Color::WHITE, Color::WHITE, muted);
    style.border = value_color(primary, primary.lighten(0.08), primary.darken(0.08), muted);
    style
}

fn time_wheel_option_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = time_option_button_style(context);
    style.text_style = TextStyle {
        font_family: None,
        size: sp(15.0),
        line_height: Some(sp(20.0)),
        weight: FontWeight::Medium,
        letter_spacing: Some(sp(0.0)),
    };
    style
}

fn time_wheel_selected_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = time_option_selected_button_style(context);
    style.text_style = TextStyle {
        font_family: None,
        size: sp(22.0),
        line_height: Some(sp(26.0)),
        weight: FontWeight::SemiBold,
        letter_spacing: Some(sp(0.0)),
    };
    style.min_height = dp(44.0);
    style
}

fn time_wheel_separator_style(context: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = label_text_style(context);
    style.typography.size = sp(22.0);
    style.typography.line_height = Some(sp(26.0));
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn calendar_weekday_text_style(context: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_text_style(context);
    style.typography.size = sp(12.0);
    style.typography.line_height = Some(sp(16.0));
    style.typography.weight = FontWeight::Medium;
    style
}

fn calendar_day_button_style(
    context: &StyleContext<'_>,
    selected: bool,
    today: bool,
    same_month: bool,
) -> ButtonStyle {
    let (_, primary, text, muted, _, _, outline) = mode_colors(context);
    let mut style = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Ghost);
    let normal_bg = if selected {
        primary
    } else if today {
        primary.with_alpha_factor(0.12)
    } else {
        Color::TRANSPARENT
    };
    let hover_bg = if selected {
        primary.lighten(0.08)
    } else {
        primary.with_alpha_factor(0.10)
    };
    let foreground = if selected {
        Color::WHITE
    } else if same_month {
        text
    } else {
        muted.with_alpha_factor(0.72)
    };
    let border = if today && !selected {
        primary.with_alpha_factor(0.62)
    } else {
        Color::TRANSPARENT
    };
    style.background = value_color(
        normal_bg,
        hover_bg,
        hover_bg.darken(0.08),
        Color::TRANSPARENT,
    );
    style.foreground = value_color(
        foreground,
        foreground,
        foreground,
        muted.with_alpha_factor(0.55),
    );
    style.border = value_color(
        border,
        primary.with_alpha_factor(0.78),
        primary,
        outline.with_alpha_factor(0.25),
    );
    style.border_width = Value::Static(if today && !selected { dp(1.0) } else { dp(0.0) });
    style.radius = Value::Static(dp(8.0));
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = dp(34.0);
    style.text_style = TextStyle {
        font_family: None,
        size: sp(13.0),
        line_height: Some(sp(18.0)),
        weight: if selected {
            FontWeight::SemiBold
        } else {
            FontWeight::Medium
        },
        letter_spacing: Some(sp(0.0)),
    };
    style
}

fn today_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = time_option_button_style(context);
    style.min_height = dp(34.0);
    style
}

fn panel_style(context: &StyleContext<'_>) -> ContainerStyle {
    let popover = PopoverStyle::default_for_theme(context.theme);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(popover.background);
    style.surface.border_color = Some(popover.border);
    style.surface.border_width = Some(popover.border_width);
    style.surface.border_radius = Some(popover.radius);
    style.surface.shadow = Some(Value::Static(popover.shadow));
    style
}

fn color_preview_style(context: &StyleContext<'_>, color: Color) -> ContainerStyle {
    color_preview_value_style(context, Value::Static(color))
}

fn color_preview_value_style(context: &StyleContext<'_>, color: Value<Color>) -> ContainerStyle {
    let (_, _, _, _, _, _, outline) = mode_colors(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(color);
    style.surface.border_color = Some(Value::Static(outline));
    style.surface.border_width = Some(Value::Static(dp(1.0)));
    style.surface.border_radius = Some(Value::Static(dp(8.0)));
    style.surface.shadow = None;
    style
}

fn color_swatch_style(context: &StyleContext<'_>, color: Color) -> ContainerStyle {
    let mut style = color_preview_style(context, color);
    style.surface.border_radius = Some(Value::Static(dp(7.0)));
    style
}

fn accent_badge_style(context: &StyleContext<'_>) -> ContainerStyle {
    let (_, primary, _, _, _, _, _) = mode_colors(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(Value::Static(primary.with_alpha_factor(0.12)));
    style.surface.border_color = Some(Value::Static(primary.with_alpha_factor(0.24)));
    style.surface.border_width = Some(Value::Static(dp(1.0)));
    style.surface.border_radius = Some(Value::Static(dp(8.0)));
    style.surface.shadow = None;
    style
}

fn subtle_badge_style(context: &StyleContext<'_>) -> ContainerStyle {
    let (_, _, _, _, _, surface_low, outline) = mode_colors(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(Value::Static(surface_low));
    style.surface.border_color = Some(Value::Static(outline));
    style.surface.border_width = Some(Value::Static(dp(1.0)));
    style.surface.border_radius = Some(Value::Static(dp(8.0)));
    style.surface.shadow = None;
    style
}

fn label_text_style(context: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(context.theme);
    style.typography = TextStyle {
        font_family: None,
        size: sp(14.0),
        line_height: Some(sp(18.0)),
        weight: FontWeight::Medium,
        letter_spacing: Some(sp(0.0)),
    };
    style
}

fn muted_text_style(context: &StyleContext<'_>) -> TextWidgetStyle {
    let (_, _, _, muted, _, _, _) = mode_colors(context);
    let mut style = label_text_style(context);
    style.color = Value::Static(muted);
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_month_has_six_weeks() {
        let days = calendar_days(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert_eq!(days.len(), 42);
        assert_eq!(days[0], NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
    }

    #[test]
    fn upload_validation_rejects_extensions() {
        let selection = validate_upload_paths(
            vec![PathBuf::from("a.exe")],
            &["png".to_string()],
            None,
            None,
            0,
        );
        assert!(selection.files.is_empty());
        assert_eq!(selection.rejected.len(), 1);
    }

    #[test]
    fn upload_validation_counts_existing_files() {
        let selection = validate_upload_paths(
            vec![PathBuf::from("a.png"), PathBuf::from("b.png")],
            &["png".to_string()],
            Some(2),
            None,
            1,
        );
        assert_eq!(selection.files.len(), 1);
        assert_eq!(selection.rejected.len(), 1);
    }

    #[test]
    fn number_clamp_respects_bounds() {
        assert_eq!(parse_number("42", Some(10.0), Some(20.0)), Some(20.0));
    }

    #[test]
    fn time_parser_accepts_hh_mm() {
        assert_eq!(parse_time("09:30"), NaiveTime::from_hms_opt(9, 30, 0));
        assert_eq!(parse_time("25:00"), None);
    }

    #[test]
    fn color_label_formats_rgba_hex() {
        assert_eq!(format_color(Color::rgba(12, 34, 56, 78)), "#0C22384E");
    }
}
