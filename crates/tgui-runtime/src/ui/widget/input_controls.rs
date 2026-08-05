use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Timelike};

use crate::dialog::FileDialogOptions;
use crate::foundation::binding::{InvalidationSignal, State, TextChangeSet, TextController};
use crate::foundation::color::Color;
use crate::foundation::form::ValidationVisualState;
use crate::foundation::view_model::{Command, CommandEffect, ValueCommand};
use crate::theme::{Density, FontWeight, ResolvedThemeMode, StyleContext, Theme, WidgetState};
use crate::ui::layout::{fr, pct, Align, Insets, Justify, Length, Value, Wrap};
use crate::ui::theme::{StateValue, TextStyle};
use crate::ui::unit::{dp, sp, Dp};

use super::common::ButtonVariantKind;
use super::icon::SvgIconId;
use super::popover::PopoverOpenHandle;
use super::style::{
    ButtonStyle, ContainerStyle, InputStyle, PopoverStyle, SelectStyle, StyleResolver,
    TextWidgetStyle,
};
use super::{
    Button, CursorStyle, Element, FileDropEvent, Flex, For, Grid, Icon, Input, Popover,
    ProgressBar, Slider, Stack, Text, WidgetId,
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct AdvancedInputMetrics {
    control_height: Dp,
    control_gap: Dp,
    upload_drop_min_height: Dp,
    upload_drop_padding: Dp,
    upload_section_gap: Dp,
    upload_row_padding: Dp,
    upload_row_gap: Dp,
    upload_badge_size: Dp,
    upload_file_badge_size: Dp,
    upload_action_size: Dp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PickerContentMetrics {
    panel_padding: Dp,
    section_gap: Dp,
    inline_gap: Dp,
    header_height: Dp,
    action_size: Dp,
    button_height: Dp,
    weekday_height: Dp,
    time_option_height: Dp,
    time_selected_height: Dp,
    color_preview_size: Dp,
}

type InputControlWidthResolver = Arc<dyn Fn(&StyleContext<'_>) -> Dp + Send + Sync>;

#[derive(Clone)]
struct InputControlValue<T> {
    value: Value<T>,
    local: Option<State<T>>,
}

impl<T> InputControlValue<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    fn new(value: Value<T>) -> Self {
        match value {
            Value::Static(value) => {
                let local = State::new(value, InvalidationSignal::new());
                Self {
                    value: Value::Signal(local.signal()),
                    local: Some(local),
                }
            }
            Value::Signal(signal) => Self {
                value: Value::Signal(signal),
                local: None,
            },
        }
    }

    fn value(&self) -> Value<T> {
        self.value.clone()
    }

    fn resolve(&self) -> T {
        self.value.resolve()
    }

    fn is_locally_owned(&self) -> bool {
        self.local.is_some()
    }

    fn set_local(&self, value: T) {
        if let Some(local) = self.local.as_ref() {
            local.set(value);
        }
    }

    fn update_local(&self, update: impl FnOnce(&mut T)) {
        if let Some(local) = self.local.as_ref() {
            local.update(update);
        }
    }

    fn is_local(&self) -> bool {
        self.local.is_some()
    }
}

fn singleton_items<T>(value: Value<T>) -> Value<Vec<T>>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    match value {
        Value::Static(value) => Value::Static(vec![value]),
        Value::Signal(signal) => Value::Signal(signal.map(|value| vec![value])),
    }
}

fn advanced_input_metrics(theme: &Theme) -> AdvancedInputMetrics {
    match theme.density {
        Density::Compact => AdvancedInputMetrics {
            control_height: dp(32.0),
            control_gap: theme.spacing.xs,
            upload_drop_min_height: dp(112.0),
            upload_drop_padding: theme.spacing.md - theme.spacing.xs,
            upload_section_gap: theme.spacing.sm,
            upload_row_padding: theme.spacing.sm,
            upload_row_gap: theme.spacing.sm - theme.spacing.xxs,
            upload_badge_size: dp(36.0),
            upload_file_badge_size: dp(28.0),
            upload_action_size: dp(28.0),
        },
        Density::Comfortable => AdvancedInputMetrics {
            control_height: dp(40.0),
            control_gap: theme.spacing.sm - theme.spacing.xxs,
            upload_drop_min_height: dp(136.0),
            upload_drop_padding: theme.spacing.md + theme.spacing.xxs,
            upload_section_gap: theme.spacing.sm + theme.spacing.xxs,
            upload_row_padding: theme.spacing.md - theme.spacing.xs,
            upload_row_gap: theme.spacing.sm,
            upload_badge_size: dp(44.0),
            upload_file_badge_size: dp(34.0),
            upload_action_size: dp(32.0),
        },
        Density::Spacious => AdvancedInputMetrics {
            control_height: dp(48.0),
            control_gap: theme.spacing.sm,
            upload_drop_min_height: dp(160.0),
            upload_drop_padding: theme.spacing.lg,
            upload_section_gap: theme.spacing.md - theme.spacing.xs,
            upload_row_padding: theme.spacing.md,
            upload_row_gap: theme.spacing.sm + theme.spacing.xs,
            upload_badge_size: dp(52.0),
            upload_file_badge_size: dp(40.0),
            upload_action_size: dp(40.0),
        },
    }
}

fn picker_content_metrics(theme: &Theme) -> PickerContentMetrics {
    match theme.density {
        Density::Compact => PickerContentMetrics {
            panel_padding: dp(8.0),
            section_gap: dp(8.0),
            inline_gap: dp(6.0),
            header_height: dp(32.0),
            action_size: dp(28.0),
            button_height: dp(30.0),
            weekday_height: dp(20.0),
            time_option_height: dp(30.0),
            time_selected_height: dp(38.0),
            color_preview_size: dp(40.0),
        },
        Density::Comfortable => PickerContentMetrics {
            panel_padding: dp(12.0),
            section_gap: dp(10.0),
            inline_gap: dp(8.0),
            header_height: dp(36.0),
            action_size: dp(32.0),
            button_height: dp(34.0),
            weekday_height: dp(22.0),
            time_option_height: dp(34.0),
            time_selected_height: dp(44.0),
            color_preview_size: dp(44.0),
        },
        Density::Spacious => PickerContentMetrics {
            panel_padding: dp(16.0),
            section_gap: dp(12.0),
            inline_gap: dp(10.0),
            header_height: dp(40.0),
            action_size: dp(36.0),
            button_height: dp(38.0),
            weekday_height: dp(24.0),
            time_option_height: dp(38.0),
            time_selected_height: dp(48.0),
            color_preview_size: dp(48.0),
        },
    }
}

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
        let metrics = advanced_input_metrics(theme);
        Self {
            width: dp(180.0),
            button_width: metrics.control_height,
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
    pub(crate) fn default_for_theme(theme: &Theme) -> Self {
        Self {
            width: match theme.density {
                Density::Compact => dp(420.0),
                Density::Comfortable => dp(460.0),
                Density::Spacious => dp(500.0),
            },
        }
    }
}

fn resolve_input_control_style_for_context<T: Clone>(
    style: Option<&StyleResolver<T>>,
    context: &StyleContext<'_>,
    default: impl Fn(&Theme) -> T,
) -> T {
    let base = default(context.theme);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
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
            id: upload_file_id_for_path(&path),
            path,
            name,
            size_bytes,
            status: UploadStatus::Queued,
        }
    }

    pub fn progress(&self) -> f32 {
        match &self.status {
            UploadStatus::Queued => 0.0,
            UploadStatus::Uploading { progress } => normalized_upload_progress(*progress),
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
        calendar_element(
            InputControlValue::new(calendar.display_month),
            InputControlValue::new(calendar.selected),
            calendar.today,
            calendar.disabled,
            calendar.on_change,
            calendar.style,
            calendar.framed,
        )
    }
}

pub struct DatePicker<VM> {
    controller: TextController,
    selected: Value<Option<NaiveDate>>,
    display_month: Value<NaiveDate>,
    open: Value<bool>,
    disabled: Value<bool>,
    validation: Value<ValidationVisualState>,
    label: Value<String>,
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
            label: Value::Static("Date".to_string()),
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

    /// Sets the name announced for the editable date field.
    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = label.into();
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
            label,
            placeholder,
            on_change,
            on_month_change,
            on_open_change,
            style,
        } = picker;
        let selected = InputControlValue::new(selected);
        let display_month = InputControlValue::new(display_month);
        let (controlled_open, open_handle) = picker_open_state(open);
        let can_toggle = open_handle.is_some() || on_open_change.is_some();
        let trigger_style = style.clone();

        let parse_controller = controller.clone();
        let typed_selected = selected.clone();
        let typed_display_month = display_month.clone();
        let typed_change_command = on_change.clone();
        let typed_change = Some(ValueCommand::new_with_context(
            move |vm, _: TextChangeSet, ctx| {
                let text = parse_controller.text();
                let date = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok();
                typed_selected.set_local(date);
                if let Some(date) = date {
                    typed_display_month.set_local(month_start(date));
                }
                if let Some(command) = typed_change_command.as_ref() {
                    command.execute_with_context(vm, DatePickerChange { date, text }, ctx);
                }
            },
        ));

        let (trigger, return_focus_to) = picker_input_trigger(
            controller.clone(),
            placeholder,
            validation,
            disabled.clone(),
            label,
            Arc::new(move |context| {
                resolve_input_control_style_for_context(
                    trigger_style.as_ref(),
                    context,
                    DatePickerStyle::default_for_theme,
                )
                .width
            }),
            ICON_CALENDAR,
            "Open date picker",
            can_toggle,
            typed_change,
        );

        let calendar_command = {
            let controller = controller.clone();
            let selected = selected.clone();
            let display_month = display_month.clone();
            let on_change = on_change.clone();
            let on_month_change = on_month_change.clone();
            let on_open_change = on_open_change.clone();
            let open_handle = open_handle.clone();
            ValueCommand::new_with_context(move |vm, change: CalendarSelectionChange, ctx| {
                display_month.set_local(change.display_month);
                match change.trigger {
                    CalendarChangeTrigger::PreviousMonth | CalendarChangeTrigger::NextMonth => {
                        if let Some(command) = on_month_change.as_ref() {
                            command.execute_with_context(vm, change.display_month, ctx);
                        }
                    }
                    _ => {
                        selected.set_local(Some(change.date));
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
                        if let Some(handle) = open_handle.as_ref() {
                            handle.set(false);
                        }
                        if let Some(command) = on_open_change.as_ref() {
                            command.execute_with_context(vm, false, ctx);
                        }
                    }
                }
            })
        };

        let content_style = style.clone();
        let content = Calendar::new(display_month.value(), selected.value())
            .style_full(move |context| {
                resolve_input_control_style_for_context(
                    content_style.as_ref(),
                    context,
                    DatePickerStyle::default_for_theme,
                )
                .calendar
            })
            .disable(disabled.clone())
            .on_change(calendar_command)
            .unframed();
        picker_popover(
            trigger,
            picker_popover_content(content),
            controlled_open,
            open_handle,
            disabled,
            on_open_change,
            return_focus_to,
        )
    }
}

pub struct TimePicker<VM> {
    controller: TextController,
    selected: Value<Option<NaiveTime>>,
    open: Value<bool>,
    disabled: Value<bool>,
    validation: Value<ValidationVisualState>,
    label: Value<String>,
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
            label: Value::Static("Time".to_string()),
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

    /// Sets the name announced for the editable time field.
    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = label.into();
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
            label,
            placeholder,
            minute_step,
            on_change,
            on_open_change,
            style,
        } = picker;
        let selected = InputControlValue::new(selected);
        let (controlled_open, open_handle) = picker_open_state(open);
        let can_toggle = open_handle.is_some() || on_open_change.is_some();
        let trigger_style = style.clone();

        let parse_controller = controller.clone();
        let typed_selected = selected.clone();
        let typed_change_command = on_change.clone();
        let typed_change = Some(ValueCommand::new_with_context(
            move |vm, _: TextChangeSet, ctx| {
                let text = parse_controller.text();
                let time = parse_time(&text);
                typed_selected.set_local(time);
                if let Some(command) = typed_change_command.as_ref() {
                    command.execute_with_context(vm, TimePickerChange { time, text }, ctx);
                }
            },
        ));

        let (trigger, return_focus_to) = picker_input_trigger(
            controller.clone(),
            placeholder,
            validation,
            disabled.clone(),
            label,
            Arc::new(move |context| {
                resolve_input_control_style_for_context(
                    trigger_style.as_ref(),
                    context,
                    TimePickerStyle::default_for_theme,
                )
                .width
            }),
            ICON_TIME,
            "Open time picker",
            can_toggle,
            typed_change,
        );

        let content = time_picker_content(
            controller,
            selected,
            minute_step,
            disabled.clone(),
            on_change,
            open_handle.clone(),
            on_open_change.clone(),
            style,
        );
        picker_popover(
            trigger,
            picker_popover_content(content),
            controlled_open,
            open_handle,
            disabled,
            on_open_change,
            return_focus_to,
        )
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
        let (min, max) = normalize_number_bounds(min, max);
        let decrement = number_step_command(
            NumberInputChangeTrigger::StepDown,
            -step,
            controller.clone(),
            value.clone(),
            min,
            max,
            disabled.clone(),
            on_change.clone(),
        );
        let increment = number_step_command(
            NumberInputChangeTrigger::StepUp,
            step,
            controller.clone(),
            value.clone(),
            min,
            max,
            disabled.clone(),
            on_change.clone(),
        );
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

        let field_style = style.clone();
        let mut field = Input::new(controller.clone())
            .runtime_layout(move |layout, context, _, _| {
                let resolved = resolve_input_control_style_for_context(
                    field_style.as_ref(),
                    context,
                    NumberInputStyle::default_for_theme,
                );
                if layout.width.is_none() {
                    layout.width = Some(Value::Static(Length::Px(resolved.width)));
                }
            })
            .placeholder(placeholder)
            .validation(validation)
            .disable(disabled.clone())
            .number_input_behavior(increment.clone(), decrement.clone(), min, max, step);
        if let Some(command) = typed_change {
            field = field.on_change_set(command);
        }
        let minus = number_step_button(
            ICON_REMOVE,
            NumberInputChangeTrigger::StepDown,
            disabled.clone(),
            decrement,
            style.clone(),
        );
        let plus = number_step_button(
            ICON_ADD,
            NumberInputChangeTrigger::StepUp,
            disabled.clone(),
            increment,
            style.clone(),
        );

        Flex::horizontal()
            .align(Align::Center)
            .runtime_layout(move |layout, container, context, _, _| {
                let resolved = resolve_input_control_style_for_context(
                    style.as_ref(),
                    context,
                    NumberInputStyle::default_for_theme,
                );
                let metrics = advanced_input_metrics(context.theme);
                layout.width = Some(Value::Static(Length::Px(
                    resolved.width + resolved.button_width * 2.0 + metrics.control_gap * 2.0,
                )));
                layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
                container.gap = Value::Static(Length::Px(metrics.control_gap));
            })
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
        let color = InputControlValue::new(color);
        let (controlled_open, open_handle) = picker_open_state(open);
        let can_toggle = open_handle.is_some() || on_open_change.is_some();
        let trigger_style = style.clone();
        let (trigger, return_focus_to) = color_picker_trigger(
            color.value(),
            disabled.clone(),
            can_toggle,
            Arc::new(move |context| {
                resolve_input_control_style_for_context(
                    trigger_style.as_ref(),
                    context,
                    ColorPickerStyle::default_for_theme,
                )
                .width
            }),
        );
        let content = picker_popover_content(color_picker_content(
            color,
            disabled.clone(),
            on_change,
            swatches,
            style,
        ));
        picker_popover(
            trigger,
            content,
            controlled_open,
            open_handle,
            disabled,
            on_open_change,
            return_focus_to,
        )
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
        let files = InputControlValue::new(files);
        let can_select = files.is_local() || on_select.is_some();
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
                let _ = ctx.dialogs().open_files_async(
                    options,
                    ValueCommand::new_with_context(
                        move |vm: &mut VM,
                              result: Result<Option<Vec<PathBuf>>, crate::dialog::DialogError>,
                              ctx| {
                            let selection = match result {
                                Ok(Some(paths)) => Some(validate_upload_paths(
                                    paths,
                                    &accept,
                                    max_files,
                                    max_file_size,
                                    files_for_callback.resolve().len(),
                                )),
                                Ok(None) => None,
                                Err(error) => Some(UploadSelection {
                                    files: Vec::new(),
                                    rejected: vec![UploadRejection {
                                        path: PathBuf::new(),
                                        reason: error.to_string(),
                                    }],
                                }),
                            };
                            if let Some(selection) = selection {
                                append_upload_selection(&files_for_callback, &selection);
                                if let Some(command) = callback.as_ref() {
                                    command.execute_with_context(vm, selection, ctx);
                                }
                            }
                        },
                    ),
                );
            })
        };

        let drop_command = {
            let accept = accept_extensions.clone();
            let disabled = disabled.clone();
            let files = files.clone();
            ValueCommand::new_with_context(move |vm: &mut VM, event: FileDropEvent, ctx| {
                if !disabled.resolve() {
                    let selection = validate_upload_paths(
                        event.paths,
                        &accept,
                        max_files,
                        max_file_size,
                        files.resolve().len(),
                    );
                    append_upload_selection(&files, &selection);
                    if let Some(command) = on_select.as_ref() {
                        command.execute_with_context(vm, selection, ctx);
                    }
                }
            })
        };

        let drop_zone_style = style.clone();
        let disabled_for_surface = disabled.clone();
        let drop_zone_cursor = if can_select {
            disabled_cursor(disabled.clone())
        } else {
            Value::Static(CursorStyle::Default)
        };
        let mut drop_zone = Flex::vertical()
            .center()
            .runtime_layout(move |layout, container, context, _, _| {
                let resolved = resolve_input_control_style_for_context(
                    drop_zone_style.as_ref(),
                    context,
                    UploadStyle::default_for_theme,
                );
                let metrics = advanced_input_metrics(context.theme);
                layout.width = Some(Value::Static(Length::Px(resolved.width)));
                layout.min_height = Some(Value::Static(Length::Px(metrics.upload_drop_min_height)));
                container.padding = Some(Value::Static(Insets::all(metrics.upload_drop_padding)));
                container.gap = Value::Static(Length::Px(metrics.upload_row_gap));
            })
            .style_full_with_style_sheet(move |context, _, _, mut state| {
                state.disabled = disabled_for_surface.resolve();
                upload_drop_zone_style(context, state)
            })
            .cursor(drop_zone_cursor)
            .child(upload_badge::<VM>())
            .child(Text::new(title).style_full(label_text_style))
            .child(Text::new(hint).style_full(muted_text_style));
        let choose_disabled = if can_select {
            disabled.clone()
        } else {
            Value::Static(true)
        };
        let mut choose_button = Button::new("Choose files")
            .secondary()
            .disable(choose_disabled)
            .focusable(can_select);
        if can_select {
            drop_zone = drop_zone
                .on_click(dialog_command.clone())
                .on_file_drop(drop_command);
            choose_button = choose_button.on_click(dialog_command);
        }
        drop_zone = drop_zone.child(choose_button);

        let list = build_upload_list(files, on_remove, disabled.clone(), style.clone());

        Flex::vertical()
            .runtime_layout(move |layout, container, context, _, _| {
                let resolved = resolve_input_control_style_for_context(
                    style.as_ref(),
                    context,
                    UploadStyle::default_for_theme,
                );
                let metrics = advanced_input_metrics(context.theme);
                layout.width = Some(Value::Static(Length::Px(resolved.width)));
                container.gap = Value::Static(Length::Px(metrics.upload_section_gap));
            })
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
    label: Value<String>,
    width: InputControlWidthResolver,
    icon: SvgIconId,
    toggle_label: &'static str,
    can_toggle: bool,
    on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
) -> (Element<VM>, WidgetId) {
    let input_width = width.clone();
    let mut input = Input::new(controller)
        .runtime_layout(move |layout, context, _, _| {
            let metrics = advanced_input_metrics(context.theme);
            let width = input_width(context);
            if layout.width.is_none() {
                layout.width = Some(Value::Static(Length::Px(
                    (width - metrics.control_height - metrics.control_gap).max(dp(120.0)),
                )));
            }
            if layout.height.is_none() {
                layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
            }
        })
        .label(label)
        .placeholder(placeholder)
        .validation(validation)
        .disable(disabled.clone());
    if let Some(command) = on_change_set {
        input = input.on_change_set(command);
    }
    let input: Element<VM> = input.into();
    let return_focus_to = input.id;
    let icon = picker_icon_button(icon, toggle_label, disabled, can_toggle);

    let trigger = Flex::horizontal()
        .align(Align::Center)
        .runtime_layout(move |layout, container, context, _, _| {
            let metrics = advanced_input_metrics(context.theme);
            layout.width = Some(Value::Static(Length::Px(width(context))));
            layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
            container.gap = Value::Static(Length::Px(metrics.control_gap));
        })
        .child(input)
        .child(icon)
        .into();
    (trigger, return_focus_to)
}

fn color_picker_trigger<VM: 'static>(
    color: Value<Color>,
    disabled: Value<bool>,
    can_toggle: bool,
    width: InputControlWidthResolver,
) -> (Element<VM>, WidgetId) {
    let mut button = Button::new(color_label(color.clone()))
        .width(pct(100.0))
        .height(pct(100.0))
        .style_full(color_trigger_accessible_button_style)
        .disable(disabled.clone())
        .focusable(can_toggle)
        .cursor(disabled_cursor(disabled.clone()));
    if can_toggle {
        // Pointer presses toggle through the Popover trigger ancestor. The command also makes
        // this real button activatable by Enter/Space and AccessKit's Click action; those paths
        // use the same ancestor toggle before dispatching this no-op command.
        button = button.on_click(Command::new(|_: &mut VM| {}).effect(CommandEffect::NoUiChange));
    }
    let button: Element<VM> = button.into();
    let return_focus_to = button.id;

    let overlay = Flex::horizontal()
        .width(pct(100.0))
        .height(pct(100.0))
        .align(Align::Center)
        .runtime_layout(move |_layout, container, context, _, _| {
            let metrics = advanced_input_metrics(context.theme);
            let input = component_input_style(context);
            container.padding = Some(Value::Static(Insets::symmetric(input.padding_x, Dp::ZERO)));
            container.gap = Value::Static(Length::Px(metrics.control_gap));
        })
        .style_full(input_control_shell_style)
        .cursor(disabled_cursor(disabled.clone()))
        .opacity(disabled_opacity(disabled))
        .child(themed_icon(ICON_COLOR, dp(18.0)))
        .child(color_preview_box::<VM>(color.clone(), dp(24.0)))
        .child(
            Text::new(color_label(color))
                .grow(1.0)
                .style_full(label_text_style),
        )
        .child(themed_icon(ICON_EXPAND, dp(20.0)));

    let trigger = Stack::new()
        .runtime_layout(move |layout, _, context, _, _| {
            let metrics = advanced_input_metrics(context.theme);
            layout.width = Some(Value::Static(Length::Px(width(context))));
            layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
        })
        .child(button)
        .child(overlay)
        .into();
    (trigger, return_focus_to)
}

fn picker_open_state(open: Value<bool>) -> (Option<Value<bool>>, Option<PopoverOpenHandle>) {
    match open {
        Value::Static(open) => (None, Some(PopoverOpenHandle::new(open))),
        Value::Signal(open) => (Some(Value::Signal(open)), None),
    }
}

fn picker_popover<VM: 'static>(
    trigger: Element<VM>,
    content: Element<VM>,
    controlled_open: Option<Value<bool>>,
    open_handle: Option<PopoverOpenHandle>,
    disabled: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    return_focus_to: WidgetId,
) -> Element<VM> {
    let mut popover = Popover::new(trigger)
        .content(content)
        .disable(disabled)
        .return_focus_to(return_focus_to);
    if let Some(open) = controlled_open {
        popover = popover.open(open);
    }
    if let Some(handle) = open_handle {
        popover = popover.open_handle(handle);
    }
    if let Some(command) = on_open_change {
        popover = popover.on_open_change(command);
    }
    popover.into()
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

fn picker_icon_button<VM: 'static>(
    icon: SvgIconId,
    label: &'static str,
    disabled: Value<bool>,
    can_toggle: bool,
) -> Element<VM> {
    let mut button = Button::new(label)
        .runtime_layout(move |layout, context, _, _| {
            let size = advanced_input_metrics(context.theme).control_height;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .secondary()
        .style_full(input_icon_button_style)
        .disable(disabled.clone())
        .focusable(can_toggle)
        .cursor(disabled_cursor(disabled));
    if can_toggle {
        // Popover toggling is owned by the trigger ancestor. A no-op click command keeps this
        // descendant in the normal Enter/Space activation index.
        button = button.on_click(Command::new(|_: &mut VM| {}).effect(CommandEffect::NoUiChange));
    }
    Stack::new()
        .center()
        .runtime_layout(move |layout, _, context, _, _| {
            let size = advanced_input_metrics(context.theme).control_height;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .child(button)
        .child(styled_icon(icon, dp(20.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .into()
}

fn ghost_icon_button<VM: 'static>(
    button_id: WidgetId,
    icon: SvgIconId,
    label: impl Into<Value<String>>,
    size: Dp,
    disabled: impl Into<Value<bool>>,
    interactive: bool,
    command: Command<VM>,
) -> Element<VM> {
    let disabled = disabled.into();
    let mut button = Button::new(label)
        .size(size, size)
        .ghost()
        .style_full(icon_button_style)
        .disable(disabled.clone())
        .focusable(interactive)
        .cursor(if interactive {
            disabled_cursor(disabled.clone())
        } else {
            Value::Static(CursorStyle::Default)
        });
    if interactive {
        button = button.on_click(command);
    }
    let mut button: Element<VM> = button.into();
    button.id = button_id;
    Stack::new()
        .size(size, size)
        .center()
        .child(button)
        .child(styled_icon(icon, dp(20.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .into()
}

fn upload_action_button<VM: 'static>(
    icon: SvgIconId,
    accessible_label: impl Into<Value<String>>,
    disabled: Value<bool>,
    command: Command<VM>,
) -> Element<VM> {
    let button = Button::new("Remove file")
        .runtime_layout(move |layout, context, _, _| {
            let size = advanced_input_metrics(context.theme).upload_action_size;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .ghost()
        .style_full(upload_action_button_style)
        .disable(disabled.clone())
        .cursor(disabled_cursor(disabled.clone()))
        .on_click(command);
    let mut button: Element<VM> = button.into();
    button.visual.accessibility_label = Some(accessible_label.into());
    Stack::new()
        .center()
        .runtime_layout(move |layout, _, context, _, _| {
            let size = advanced_input_metrics(context.theme).upload_action_size;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .child(button)
        .child(styled_icon(icon, dp(18.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .into()
}

fn color_preview_box<VM: 'static>(color: Value<Color>, size: Dp) -> Element<VM> {
    Flex::vertical()
        .size(size, size)
        .style_full(move |context| color_preview_value_style(context, color.clone()))
        .into()
}

fn picker_color_preview_box<VM: 'static>(color: Value<Color>) -> Element<VM> {
    Flex::vertical()
        .runtime_layout(move |layout, _, context, _, _| {
            let size = picker_content_metrics(context.theme).color_preview_size;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .style_full(move |context| color_preview_value_style(context, color.clone()))
        .into()
}

fn upload_badge<VM: 'static>() -> Element<VM> {
    let icon_box = dp(24.0);
    Flex::vertical()
        .shrink(0.0)
        .center()
        .runtime_layout(move |layout, _, context, _, _| {
            let size = advanced_input_metrics(context.theme).upload_badge_size;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
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
        .center()
        .runtime_layout(move |layout, _, context, _, _| {
            let size = advanced_input_metrics(context.theme).upload_file_badge_size;
            layout.width = Some(Value::Static(Length::Px(size)));
            layout.height = Some(Value::Static(Length::Px(size)));
        })
        .style_full(subtle_badge_style)
        .child(themed_icon(ICON_FILE, dp(18.0)))
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

fn disabled_cursor(disabled: Value<bool>) -> Value<CursorStyle> {
    match disabled {
        Value::Static(disabled) => Value::Static(if disabled {
            CursorStyle::NotAllowed
        } else {
            CursorStyle::Pointer
        }),
        Value::Signal(disabled) => Value::Signal(disabled.map(|disabled| {
            if disabled {
                CursorStyle::NotAllowed
            } else {
                CursorStyle::Pointer
            }
        })),
    }
}

fn calendar_element<VM: 'static>(
    display_month: InputControlValue<NaiveDate>,
    selected: InputControlValue<Option<NaiveDate>>,
    today: Option<NaiveDate>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, CalendarSelectionChange>>,
    style: Option<StyleResolver<CalendarStyle>>,
    framed: bool,
) -> Element<VM> {
    let can_change_month = display_month.is_locally_owned() || on_change.is_some();
    let can_select = (display_month.is_locally_owned() || on_change.is_some())
        && (selected.is_locally_owned() || on_change.is_some());
    let calendar_owner_id = WidgetId::next();
    let calendar_nav_ids = WidgetId::reserve(2);
    let calendar_day_ids = WidgetId::reserve(42);
    let initial_month = month_start(display_month.resolve());
    let initial_days = calendar_days(initial_month);
    let initial_focus_date = selected
        .resolve()
        .filter(|date| initial_days.contains(&Some(*date)))
        .or_else(|| today.filter(|date| initial_days.contains(&Some(*date))))
        .unwrap_or(initial_month);
    let focus_date = InputControlValue::new(Value::Static(initial_focus_date));
    let focus_move = {
        let display_month = display_month.clone();
        let focus_date = focus_date.clone();
        let on_change = on_change.clone();
        ValueCommand::new_with_context(move |vm, target: NaiveDate, ctx| {
            let current_month = month_start(display_month.resolve());
            let next_month = month_start(target);
            focus_date.set_local(target);
            if next_month == current_month {
                return;
            }
            display_month.set_local(next_month);
            if let Some(command) = on_change.as_ref() {
                command.execute_with_context(
                    vm,
                    CalendarSelectionChange {
                        date: target,
                        display_month: next_month,
                        trigger: if next_month < current_month {
                            CalendarChangeTrigger::PreviousMonth
                        } else {
                            CalendarChangeTrigger::NextMonth
                        },
                    },
                    ctx,
                );
            }
        })
    };
    let root_style = style.clone();
    let mut root = Flex::vertical().runtime_layout(move |layout, container, context, _, _| {
        let resolved = resolve_input_control_style_for_context(
            root_style.as_ref(),
            context,
            CalendarStyle::default_for_theme,
        );
        let metrics = picker_content_metrics(context.theme);
        layout.width = Some(Value::Static(Length::Px(resolved.panel_width)));
        container.gap = Value::Static(Length::Px(metrics.section_gap));
        if framed {
            container.padding = Some(Value::Static(Insets::all(metrics.panel_padding)));
        }
    });
    if framed {
        root = root.style_full(panel_style);
    }

    let header_display_month = display_month.clone();
    let header_disabled = disabled.clone();
    let header_change = on_change.clone();
    root = root.child(For::new(
        singleton_items(display_month.value()),
        |_| "calendar-header",
        move |_index, display_month| {
            let month = month_start(*display_month);
            Flex::horizontal()
                .runtime_layout(move |layout, _, context, _, _| {
                    layout.height = Some(Value::Static(Length::Px(
                        picker_content_metrics(context.theme).header_height,
                    )));
                })
                .align(Align::Center)
                .justify(Justify::SpaceBetween)
                .child(calendar_nav_button(
                    calendar_nav_ids,
                    ICON_PREVIOUS,
                    add_months(month, -1),
                    CalendarChangeTrigger::PreviousMonth,
                    header_display_month.clone(),
                    header_disabled.clone(),
                    header_change.clone(),
                    can_change_month,
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
                    calendar_nav_ids.offset(1),
                    ICON_NEXT,
                    add_months(month, 1),
                    CalendarChangeTrigger::NextMonth,
                    header_display_month.clone(),
                    header_disabled.clone(),
                    header_change.clone(),
                    can_change_month,
                ))
        },
    ));

    let weekday_style = style.clone();
    let mut weekday_row = Grid::columns([
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
        fr(1.0),
    ])
    .runtime_layout(move |_, container, context, _, _| {
        let resolved = resolve_input_control_style_for_context(
            weekday_style.as_ref(),
            context,
            CalendarStyle::default_for_theme,
        );
        container.gap = Value::Static(Length::Px(resolved.gap));
    });
    for label in WEEKDAYS {
        let cell_style = style.clone();
        weekday_row = weekday_row.child(
            Flex::<VM>::vertical()
                .runtime_layout(move |layout, _, context, _, _| {
                    let resolved = resolve_input_control_style_for_context(
                        cell_style.as_ref(),
                        context,
                        CalendarStyle::default_for_theme,
                    );
                    layout.width = Some(Value::Static(Length::Px(resolved.day_size)));
                    layout.height = Some(Value::Static(Length::Px(
                        picker_content_metrics(context.theme).weekday_height,
                    )));
                })
                .center()
                .child(Text::new(label).style_full(calendar_weekday_text_style)),
        );
    }
    root = root.child(weekday_row);

    let days_display_month = display_month.clone();
    let days_selected = selected.clone();
    let days_focus_date = focus_date.clone();
    let days_focus_move = focus_move.clone();
    let days_disabled = disabled.clone();
    let days_change = on_change.clone();
    root = root.child(For::new(
        singleton_items(display_month.value()),
        |_| "calendar-days",
        move |_index, display_month| {
            let month = month_start(*display_month);
            let selected = days_selected.resolve();
            let visible_days = calendar_days(month);
            let requested_focus = days_focus_date.resolve();
            let roving_date = if visible_days.contains(&Some(requested_focus)) {
                requested_focus
            } else {
                selected
                    .filter(|date| visible_days.contains(&Some(*date)))
                    .or_else(|| today.filter(|date| visible_days.contains(&Some(*date))))
                    .unwrap_or(month)
            };
            let days_style = style.clone();
            let mut days = Grid::columns([
                fr(1.0),
                fr(1.0),
                fr(1.0),
                fr(1.0),
                fr(1.0),
                fr(1.0),
                fr(1.0),
            ])
            .runtime_layout(move |_, container, context, _, _| {
                let resolved = resolve_input_control_style_for_context(
                    days_style.as_ref(),
                    context,
                    CalendarStyle::default_for_theme,
                );
                container.gap = Value::Static(Length::Px(resolved.gap));
            });
            for (day_index, date) in visible_days.into_iter().enumerate() {
                let same_month = date.is_some_and(|date| date.month() == month.month());
                let is_selected = date.is_some() && selected == date;
                let is_today = date.is_some() && today == date;
                let button_style = style.clone();
                let unavailable = date.is_none();
                let button_disabled = if unavailable {
                    Value::Static(true)
                } else {
                    days_disabled.clone()
                };
                let mut button =
                    Button::new(date.map(|date| date.day().to_string()).unwrap_or_default())
                        .runtime_layout(move |layout, context, _, _| {
                            let resolved = resolve_input_control_style_for_context(
                                button_style.as_ref(),
                                context,
                                CalendarStyle::default_for_theme,
                            );
                            layout.width = Some(Value::Static(Length::Px(resolved.day_size)));
                            layout.height = Some(Value::Static(Length::Px(resolved.day_size)));
                        })
                        .style_full(move |context| {
                            calendar_day_button_style(context, is_selected, is_today, same_month)
                        })
                        .disable(button_disabled.clone())
                        .cursor(disabled_cursor(button_disabled));
                if can_select {
                    if let Some(date) = date {
                        button = button
                            .tab_index(if date == roving_date { 0 } else { -1 })
                            .calendar_day_behavior(
                                calendar_owner_id,
                                date,
                                days_focus_move.clone(),
                            );
                    } else {
                        button = button.tab_index(-1);
                    }
                }
                let display_month = days_display_month.clone();
                let selected = days_selected.clone();
                let focus_date = days_focus_date.clone();
                let command = days_change.clone();
                if can_select {
                    button = button.on_click(Command::new_with_context(move |vm, ctx| {
                        let Some(date) = date else {
                            return;
                        };
                        let next_month = month_start(date);
                        display_month.set_local(next_month);
                        selected.set_local(Some(date));
                        focus_date.set_local(date);
                        if let Some(command) = command.as_ref() {
                            command.execute_with_context(
                                vm,
                                CalendarSelectionChange {
                                    date,
                                    display_month: next_month,
                                    trigger: CalendarChangeTrigger::Day,
                                },
                                ctx,
                            );
                        }
                    }));
                }
                let mut button: Element<VM> = button.into();
                if !can_select {
                    button.interactions = Default::default();
                    button.focus.focusable = Some(false);
                    button.focus.tab_index = Some(-1);
                }
                button.id = calendar_day_ids.offset(day_index);
                if let Some(date) = date {
                    button.visual.accessibility_label = Some(Value::Static(format_date(date)));
                    button.visual.accessibility_selected = Some(Value::Static(is_selected));
                }
                days = days.child(button);
            }
            days
        },
    ));
    if let Some(today) = today {
        let today_display_month = display_month;
        let today_selected = selected;
        let today_focus_date = focus_date;
        let mut today_button = Button::new("Today")
            .secondary()
            .runtime_layout(move |layout, context, _, _| {
                layout.height = Some(Value::Static(Length::Px(
                    picker_content_metrics(context.theme).button_height,
                )));
            })
            .style_full(today_button_style)
            .disable(disabled.clone());
        if can_select {
            today_button =
                today_button
                    .cursor(disabled_cursor(disabled))
                    .on_click(Command::new_with_context(move |vm, ctx| {
                        let display_month = month_start(today);
                        today_display_month.set_local(display_month);
                        today_selected.set_local(Some(today));
                        today_focus_date.set_local(today);
                        if let Some(command) = on_change.as_ref() {
                            command.execute_with_context(
                                vm,
                                CalendarSelectionChange {
                                    date: today,
                                    display_month,
                                    trigger: CalendarChangeTrigger::Today,
                                },
                                ctx,
                            );
                        }
                    }));
        }
        let mut today_button: Element<VM> = today_button.into();
        if !can_select {
            today_button.interactions = Default::default();
            today_button.focus.focusable = Some(false);
            today_button.focus.tab_index = Some(-1);
        }
        root = root.child(today_button);
    }
    root.into()
}

fn calendar_nav_button<VM: 'static>(
    button_id: WidgetId,
    icon: SvgIconId,
    display_month: NaiveDate,
    trigger: CalendarChangeTrigger,
    month_state: InputControlValue<NaiveDate>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, CalendarSelectionChange>>,
    interactive: bool,
) -> Element<VM> {
    let label = match trigger {
        CalendarChangeTrigger::PreviousMonth => "Previous month",
        CalendarChangeTrigger::NextMonth => "Next month",
        CalendarChangeTrigger::Day | CalendarChangeTrigger::Today => "Change month",
    };
    ghost_icon_button(
        button_id,
        icon,
        label,
        dp(32.0),
        disabled,
        interactive,
        Command::new_with_context(move |vm, ctx| {
            month_state.set_local(display_month);
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
    selected: InputControlValue<Option<NaiveTime>>,
    minute_step: u32,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    open_handle: Option<PopoverOpenHandle>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<TimePickerStyle>>,
) -> Element<VM> {
    let hour_ids = TimeWheelColumnIds::new(24);
    let minute_ids = TimeWheelColumnIds::new(60);
    let done_style = style.clone();
    let mut done_button = Button::new("Done")
        .primary()
        .runtime_layout(move |layout, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                done_style.as_ref(),
                context,
                TimePickerStyle::default_for_theme,
            );
            layout.width = Some(Value::Static(Length::Px(resolved.width)));
            layout.height = Some(Value::Static(Length::Px(
                picker_content_metrics(context.theme).button_height,
            )));
        })
        .disable(if open_handle.is_none() && on_open_change.is_none() {
            Value::Static(true)
        } else {
            disabled.clone()
        })
        .cursor(disabled_cursor(
            if open_handle.is_none() && on_open_change.is_none() {
                Value::Static(true)
            } else {
                disabled.clone()
            },
        ));
    if open_handle.is_some() || on_open_change.is_some() {
        let disabled_for_done = disabled.clone();
        done_button = done_button.on_click(Command::new_with_context(move |vm, ctx| {
            if disabled_for_done.resolve() {
                return;
            }
            if let Some(handle) = open_handle.as_ref() {
                handle.set(false);
            }
            if let Some(command) = on_open_change.as_ref() {
                command.execute_with_context(vm, false, ctx);
            }
        }));
    }

    let root_style = style.clone();
    let row_style = style;
    let wheel_controller = controller;
    let wheel_selected = selected.clone();
    let wheel_disabled = disabled;
    let wheel_change = on_change;
    Flex::vertical()
        .runtime_layout(move |layout, container, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                root_style.as_ref(),
                context,
                TimePickerStyle::default_for_theme,
            );
            layout.width = Some(Value::Static(Length::Px(resolved.width)));
            container.gap = Value::Static(Length::Px(
                picker_content_metrics(context.theme).section_gap,
            ));
        })
        .child(
            Flex::horizontal()
                .align(Align::Center)
                .runtime_layout(move |_, container, context, _, _| {
                    container.gap =
                        Value::Static(Length::Px(picker_content_metrics(context.theme).inline_gap));
                })
                .child(themed_icon(ICON_TIME, dp(18.0)))
                .child(Text::new("Select time").style_full(label_text_style)),
        )
        .child(For::new(
            singleton_items(selected.value()),
            |_| "time-wheel",
            move |_index, selected| {
                time_wheel_row(
                    wheel_controller.clone(),
                    *selected,
                    wheel_selected.clone(),
                    minute_step,
                    wheel_disabled.clone(),
                    wheel_change.clone(),
                    row_style.clone(),
                    hour_ids,
                    minute_ids,
                )
            },
        ))
        .child(done_button)
        .into()
}

fn time_wheel_row<VM: 'static>(
    controller: TextController,
    selected: Option<NaiveTime>,
    selected_state: InputControlValue<Option<NaiveTime>>,
    minute_step: u32,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    style: Option<StyleResolver<TimePickerStyle>>,
    hour_ids: TimeWheelColumnIds,
    minute_ids: TimeWheelColumnIds,
) -> Element<VM> {
    let current = selected
        .or_else(|| parse_time(&controller.text()))
        .unwrap_or(NaiveTime::MIN);
    let mut minute_values = minute_values_for_step(minute_step);
    if let Err(index) = minute_values.binary_search(&current.minute()) {
        minute_values.insert(index, current.minute());
    }
    let hour_values = (0..24).collect::<Vec<_>>();
    let hour_index = value_index(&hour_values, current.hour());
    let minute_index = value_index(&minute_values, current.minute());
    let hour = hour_values[hour_index];
    let minute = minute_values[minute_index];
    let hour_column = time_wheel_column(
        "Hour",
        TimePickerUnit::Hour,
        &hour_values,
        hour_index,
        hour,
        minute,
        controller.clone(),
        selected_state.clone(),
        disabled.clone(),
        on_change.clone(),
        style.clone(),
        hour_ids,
    );
    let minute_column = time_wheel_column(
        "Minute",
        TimePickerUnit::Minute,
        &minute_values,
        minute_index,
        hour,
        minute,
        controller,
        selected_state,
        disabled,
        on_change,
        style.clone(),
        minute_ids,
    );
    let row_style = style;
    Flex::horizontal()
        .align(Align::Center)
        .justify(Justify::Center)
        .runtime_layout(move |layout, container, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                row_style.as_ref(),
                context,
                TimePickerStyle::default_for_theme,
            );
            layout.width = Some(Value::Static(Length::Px(resolved.width)));
            container.gap = Value::Static(Length::Px(
                picker_content_metrics(context.theme).section_gap,
            ));
        })
        .child(hour_column)
        .child(Text::new(":").style_full(time_wheel_separator_style))
        .child(minute_column)
        .into()
}

#[derive(Clone, Copy)]
enum TimePickerUnit {
    Hour,
    Minute,
}

#[derive(Clone, Copy)]
struct TimeWheelColumnIds {
    previous: WidgetId,
    next: WidgetId,
    values: WidgetId,
}

impl TimeWheelColumnIds {
    fn new(value_count: usize) -> Self {
        Self {
            previous: WidgetId::next(),
            next: WidgetId::next(),
            values: WidgetId::reserve(value_count),
        }
    }

    fn value(self, value: u32) -> WidgetId {
        self.values.offset(value as usize)
    }
}

fn time_wheel_column<VM: 'static>(
    label: &'static str,
    unit: TimePickerUnit,
    values: &[u32],
    selected_index: usize,
    current_hour: u32,
    current_minute: u32,
    controller: TextController,
    selected_state: InputControlValue<Option<NaiveTime>>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
    style: Option<StyleResolver<TimePickerStyle>>,
    ids: TimeWheelColumnIds,
) -> Element<VM> {
    let previous_index = previous_index(selected_index, values.len());
    let next_index = next_index(selected_index, values.len());
    let previous = values[previous_index];
    let selected_value = values[selected_index];
    let next = values[next_index];
    let navigation_disabled = if values.len() <= 1 {
        Value::Static(true)
    } else {
        disabled.clone()
    };
    let column_style = style.clone();
    let mut column = Flex::vertical()
        .runtime_layout(move |layout, container, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                column_style.as_ref(),
                context,
                TimePickerStyle::default_for_theme,
            );
            layout.width = Some(Value::Static(Length::Px(
                resolved.option_width.max(dp(96.0)),
            )));
            container.gap =
                Value::Static(Length::Px(picker_content_metrics(context.theme).inline_gap));
        })
        .align(Align::Center)
        .child(Text::new(label).style_full(muted_text_style))
        .child(ghost_icon_button(
            ids.previous,
            ICON_UP,
            format!("Previous {label}"),
            dp(32.0),
            navigation_disabled.clone(),
            true,
            time_wheel_select_command(
                unit,
                previous,
                current_hour,
                current_minute,
                controller.clone(),
                selected_state.clone(),
                navigation_disabled.clone(),
                on_change.clone(),
            ),
        ));
    if previous_index != selected_index {
        column = column.child(time_wheel_value_button(
            ids.value(previous),
            unit,
            previous,
            false,
            style.clone(),
            disabled.clone(),
            time_wheel_select_command(
                unit,
                previous,
                current_hour,
                current_minute,
                controller.clone(),
                selected_state.clone(),
                disabled.clone(),
                on_change.clone(),
            ),
        ));
    }
    column = column.child(time_wheel_value_button(
        ids.value(selected_value),
        unit,
        selected_value,
        true,
        style.clone(),
        disabled.clone(),
        time_wheel_select_command(
            unit,
            selected_value,
            current_hour,
            current_minute,
            controller.clone(),
            selected_state.clone(),
            disabled.clone(),
            on_change.clone(),
        ),
    ));
    if next_index != selected_index && next_index != previous_index {
        column = column.child(time_wheel_value_button(
            ids.value(next),
            unit,
            next,
            false,
            style,
            disabled.clone(),
            time_wheel_select_command(
                unit,
                next,
                current_hour,
                current_minute,
                controller.clone(),
                selected_state.clone(),
                disabled.clone(),
                on_change.clone(),
            ),
        ));
    }
    column
        .child(ghost_icon_button(
            ids.next,
            ICON_DOWN,
            format!("Next {label}"),
            dp(32.0),
            navigation_disabled.clone(),
            true,
            time_wheel_select_command(
                unit,
                next,
                current_hour,
                current_minute,
                controller,
                selected_state,
                navigation_disabled,
                on_change,
            ),
        ))
        .into()
}

fn time_wheel_value_button<VM: 'static>(
    button_id: WidgetId,
    unit: TimePickerUnit,
    value: u32,
    selected: bool,
    style: Option<StyleResolver<TimePickerStyle>>,
    disabled: Value<bool>,
    command: Command<VM>,
) -> Element<VM> {
    let button = Button::new(format!("{value:02}"))
        .runtime_layout(move |layout, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                style.as_ref(),
                context,
                TimePickerStyle::default_for_theme,
            );
            let metrics = picker_content_metrics(context.theme);
            layout.width = Some(Value::Static(Length::Px(
                resolved.option_width.max(dp(96.0)),
            )));
            layout.height = Some(Value::Static(Length::Px(if selected {
                metrics.time_selected_height
            } else {
                metrics.time_option_height
            })));
        })
        .disable(disabled.clone())
        .cursor(disabled_cursor(disabled))
        .on_click(command);
    let button = if selected {
        button
            .primary()
            .style_full(time_wheel_selected_button_style)
    } else {
        button
            .secondary()
            .style_full(time_wheel_option_button_style)
    };
    let unit_label = match unit {
        TimePickerUnit::Hour => "Hour",
        TimePickerUnit::Minute => "Minute",
    };
    let mut button: Element<VM> = button.into();
    button.id = button_id;
    button.visual.accessibility_label = Some(Value::Static(format!("{unit_label} {value:02}")));
    button.visual.accessibility_selected = Some(Value::Static(selected));
    button
}

fn time_wheel_select_command<VM: 'static>(
    unit: TimePickerUnit,
    value: u32,
    current_hour: u32,
    current_minute: u32,
    controller: TextController,
    selected_state: InputControlValue<Option<NaiveTime>>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, TimePickerChange>>,
) -> Command<VM> {
    Command::new_with_context(move |vm, ctx| {
        if disabled.resolve() {
            return;
        }
        let (hour, minute) = match unit {
            TimePickerUnit::Hour => (value, current_minute),
            TimePickerUnit::Minute => (current_hour, value),
        };
        let time = NaiveTime::from_hms_opt(hour, minute, 0).unwrap_or(NaiveTime::MIN);
        let text = format_time(time);
        controller.set_text(text.clone());
        selected_state.set_local(Some(time));
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
    disabled: Value<bool>,
    command: Command<VM>,
    style: Option<StyleResolver<NumberInputStyle>>,
) -> Element<VM> {
    let button_style = style.clone();
    let button = Button::new(match trigger {
        NumberInputChangeTrigger::StepDown => "Decrease value",
        NumberInputChangeTrigger::StepUp => "Increase value",
        NumberInputChangeTrigger::Text => "Change value",
    })
    .runtime_layout(move |layout, context, _, _| {
        let resolved = resolve_input_control_style_for_context(
            button_style.as_ref(),
            context,
            NumberInputStyle::default_for_theme,
        );
        let metrics = advanced_input_metrics(context.theme);
        layout.width = Some(Value::Static(Length::Px(resolved.button_width)));
        layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
    })
    .secondary()
    .style_full(number_step_button_style)
    .disable(disabled.clone())
    .cursor(disabled_cursor(disabled.clone()))
    .on_click(command);
    Stack::new()
        .center()
        .runtime_layout(move |layout, _, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                style.as_ref(),
                context,
                NumberInputStyle::default_for_theme,
            );
            let metrics = advanced_input_metrics(context.theme);
            layout.width = Some(Value::Static(Length::Px(resolved.button_width)));
            layout.height = Some(Value::Static(Length::Px(metrics.control_height)));
        })
        .child(button)
        .child(styled_icon(icon, dp(18.0), |context| {
            let (_, _, _, muted, _, _, _) = mode_colors(context);
            muted
        }))
        .into()
}

fn number_step_command<VM: 'static>(
    trigger: NumberInputChangeTrigger,
    delta: f64,
    controller: TextController,
    value: Value<Option<f64>>,
    min: Option<f64>,
    max: Option<f64>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, NumberInputChange>>,
) -> Command<VM> {
    Command::new_with_context(move |vm, ctx| {
        if disabled.resolve() {
            return;
        }
        let current_text = controller.text();
        let current = parse_number(&current_text, min, max)
            .or_else(|| value.resolve().filter(|value| value.is_finite()))
            .unwrap_or(0.0);
        let next = clamp_number(current + delta, min, max);
        let precision = stepped_number_precision(&current_text, delta, next, min, max);
        let text = format_number(next, precision);
        if next == current && text == current_text {
            return;
        }
        let emitted_value = parse_number(&text, min, max).unwrap_or(next);
        controller.set_text(text.clone());
        if let Some(command) = on_change.as_ref() {
            command.execute_with_context(
                vm,
                NumberInputChange {
                    value: Some(emitted_value),
                    text,
                    trigger,
                },
                ctx,
            );
        }
    })
}

fn color_picker_content<VM: 'static>(
    color: InputControlValue<Color>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, ColorPickerChange>>,
    swatches: Vec<Color>,
    style: Option<StyleResolver<ColorPickerStyle>>,
) -> Element<VM> {
    let root_style = style.clone();
    let mut root = Flex::vertical().runtime_layout(move |layout, container, context, _, _| {
        let resolved = resolve_input_control_style_for_context(
            root_style.as_ref(),
            context,
            ColorPickerStyle::default_for_theme,
        );
        layout.width = Some(Value::Static(Length::Px(resolved.width)));
        container.gap = Value::Static(Length::Px(
            picker_content_metrics(context.theme).section_gap,
        ));
    });
    root = root.child(
        Flex::horizontal()
            .align(Align::Center)
            .runtime_layout(move |_, container, context, _, _| {
                container.gap = Value::Static(Length::Px(
                    picker_content_metrics(context.theme).section_gap,
                ));
            })
            .child(picker_color_preview_box::<VM>(color.value()))
            .child(
                Flex::vertical()
                    .gap(dp(2.0))
                    .child(Text::new("Current color").style_full(muted_text_style))
                    .child(Text::new(color_label(color.value())).style_full(label_text_style)),
            ),
    );

    let mut swatch_row =
        Flex::horizontal()
            .wrap(Wrap::Wrap)
            .runtime_layout(move |_, container, context, _, _| {
                container.gap =
                    Value::Static(Length::Px(picker_content_metrics(context.theme).inline_gap));
            });
    for swatch in swatches {
        let color = color.clone();
        let command = on_change.clone();
        let swatch_style = style.clone();
        swatch_row = swatch_row.child(
            Button::new(format_color(swatch))
                .runtime_layout(move |layout, context, _, _| {
                    let resolved = resolve_input_control_style_for_context(
                        swatch_style.as_ref(),
                        context,
                        ColorPickerStyle::default_for_theme,
                    );
                    layout.width = Some(Value::Static(Length::Px(resolved.swatch_size)));
                    layout.height = Some(Value::Static(Length::Px(resolved.swatch_size)));
                })
                .style_full(move |context| color_swatch_button_style(context, swatch))
                .disable(disabled.clone())
                .cursor(disabled_cursor(disabled.clone()))
                .on_click(Command::new_with_context(move |vm, ctx| {
                    color.set_local(swatch);
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
                })),
        );
    }
    root = root.child(swatch_row);
    root = root.child(color_slider(
        "R",
        color.clone(),
        ColorPickerChangeTrigger::Red,
        disabled.clone(),
        on_change.clone(),
    ));
    root = root.child(color_slider(
        "G",
        color.clone(),
        ColorPickerChangeTrigger::Green,
        disabled.clone(),
        on_change.clone(),
    ));
    root = root.child(color_slider(
        "B",
        color.clone(),
        ColorPickerChangeTrigger::Blue,
        disabled.clone(),
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
    color: InputControlValue<Color>,
    trigger: ColorPickerChangeTrigger,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, ColorPickerChange>>,
) -> Element<VM> {
    let value = color_channel_value(color.value(), trigger);
    let color_for_change = color.clone();
    Flex::horizontal()
        .align(Align::Center)
        .runtime_layout(move |_, container, context, _, _| {
            container.gap =
                Value::Static(Length::Px(picker_content_metrics(context.theme).inline_gap));
        })
        .child(
            Text::new(label)
                .width(dp(18.0))
                .style_full(label_text_style),
        )
        .child(
            Slider::new(value, 0.0, 255.0)
                .label(color_channel_accessible_label(trigger))
                .step(1.0)
                .grow(1.0)
                .disable(disabled)
                .on_change(ValueCommand::new_with_context(move |vm, next: f32, ctx| {
                    let mut current = color_for_change.resolve();
                    let channel = next.round().clamp(0.0, 255.0) as u8;
                    match trigger {
                        ColorPickerChangeTrigger::Red => current.r = channel,
                        ColorPickerChangeTrigger::Green => current.g = channel,
                        ColorPickerChangeTrigger::Blue => current.b = channel,
                        ColorPickerChangeTrigger::Alpha => current.a = channel,
                        ColorPickerChangeTrigger::Swatch => {}
                    }
                    color_for_change.set_local(current);
                    if let Some(command) = on_change.as_ref() {
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
            Text::new(color_channel_label(color.value(), trigger))
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
    files: InputControlValue<Vec<UploadFile>>,
    on_remove: Option<ValueCommand<VM, UploadRemove>>,
    disabled: Value<bool>,
    style: Option<StyleResolver<UploadStyle>>,
) -> Element<VM> {
    let list_style = style.clone();
    let list = Flex::vertical().runtime_layout(move |layout, container, context, _, _| {
        let resolved = resolve_input_control_style_for_context(
            list_style.as_ref(),
            context,
            UploadStyle::default_for_theme,
        );
        let metrics = advanced_input_metrics(context.theme);
        layout.width = Some(Value::Static(Length::Px(resolved.width)));
        container.gap = Value::Static(Length::Px(metrics.upload_row_gap));
    });
    let files_for_rows = files.clone();
    list.child(For::new(
        files.value(),
        |file| file.id.as_str().to_string(),
        move |_index, file| {
            upload_row(
                file.clone(),
                files_for_rows.clone(),
                on_remove.clone(),
                disabled.clone(),
                style.clone(),
            )
        },
    ))
    .into()
}

fn upload_row<VM: 'static>(
    file: UploadFile,
    files: InputControlValue<Vec<UploadFile>>,
    on_remove: Option<ValueCommand<VM, UploadRemove>>,
    disabled: Value<bool>,
    style: Option<StyleResolver<UploadStyle>>,
) -> Element<VM> {
    let id = file.id.clone();
    let remove = (files.is_local() || on_remove.is_some()).then(|| {
        Command::new_with_context(move |vm, ctx| {
            files.update_local(|files| files.retain(|file| file.id != id));
            if let Some(command) = on_remove.as_ref() {
                command.execute_with_context(vm, UploadRemove { id: id.clone() }, ctx);
            }
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
        footer = footer.child(upload_action_button(
            ICON_DELETE,
            format!("Remove {}", file.name),
            disabled,
            command,
        ));
    }
    let row_style = style.clone();
    Flex::vertical()
        .runtime_layout(move |layout, container, context, _, _| {
            let resolved = resolve_input_control_style_for_context(
                row_style.as_ref(),
                context,
                UploadStyle::default_for_theme,
            );
            let metrics = advanced_input_metrics(context.theme);
            layout.width = Some(Value::Static(Length::Px(resolved.width)));
            container.padding = Some(Value::Static(Insets::all(metrics.upload_row_padding)));
            container.gap = Value::Static(Length::Px(metrics.upload_row_gap));
        })
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
        .child(
            ProgressBar::<VM>::new(file.progress())
                .label(format!("Upload progress for {}", file.name))
                .height(dp(8.0)),
        )
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
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                rejected.push(UploadRejection {
                    path,
                    reason: format!("Unable to read file metadata: {error}"),
                });
                continue;
            }
        };
        if !metadata.is_file() {
            rejected.push(UploadRejection {
                path,
                reason: "Only files can be uploaded".to_string(),
            });
            continue;
        }
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
        let size = metadata.len();
        if let Some(max_size) = max_file_size {
            if size > max_size {
                rejected.push(UploadRejection {
                    path,
                    reason: format!("File exceeds {}", format_size(Some(max_size))),
                });
                continue;
            }
        }
        let mut file = UploadFile::from_path(path);
        file.size_bytes = Some(size);
        files.push(file);
    }
    UploadSelection { files, rejected }
}

fn append_upload_selection(
    files: &InputControlValue<Vec<UploadFile>>,
    selection: &UploadSelection,
) {
    if selection.files.is_empty() {
        return;
    }
    files.update_local(|current| {
        for selected in &selection.files {
            if let Some(existing) = current.iter_mut().find(|file| file.id == selected.id) {
                *existing = selected.clone();
            } else {
                current.push(selected.clone());
            }
        }
    });
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

fn calendar_days(month: NaiveDate) -> Vec<Option<NaiveDate>> {
    let month = month_start(month);
    let leading_days = i64::from(month.weekday().num_days_from_monday());
    (0..42)
        .map(|offset| month.checked_add_signed(Duration::days(i64::from(offset) - leading_days)))
        .collect()
}

fn upload_file_id_for_path(path: &std::path::Path) -> UploadFileId {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let bytes = path.as_os_str().as_encoded_bytes();
    let mut id = String::with_capacity("path:".len() + bytes.len() * 2);
    id.push_str("path:");
    for byte in bytes {
        id.push(HEX[(byte >> 4) as usize] as char);
        id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    UploadFileId::new(id)
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

fn normalize_number_bounds(min: Option<f64>, max: Option<f64>) -> (Option<f64>, Option<f64>) {
    let min = min.filter(|value| value.is_finite());
    let max = max.filter(|value| value.is_finite());
    match (min, max) {
        (Some(min), Some(max)) if min > max => (Some(max), Some(min)),
        bounds => bounds,
    }
}

fn clamp_number(mut value: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let (min, max) = normalize_number_bounds(min, max);
    if let Some(min) = min {
        value = value.max(min);
    }
    if let Some(max) = max {
        value = value.min(max);
    }
    value
}

pub(crate) fn normalize_number_input_accessibility_value(
    value: f64,
    current_text: &str,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let (min, max) = normalize_number_bounds(min, max);
    let value = clamp_number(value, min, max);
    let precision = decimal_places(current_text)
        .max(decimal_places(&step.abs().to_string()))
        .max(decimal_places(&value.to_string()))
        .min(15);
    Some(format_number(value, precision))
}

fn stepped_number_precision(
    current_text: &str,
    step: f64,
    next: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> usize {
    let mut precision = decimal_places(current_text).max(decimal_places(&step.abs().to_string()));
    if min == Some(next) {
        precision = precision.max(decimal_places(&next.to_string()));
    }
    if max == Some(next) {
        precision = precision.max(decimal_places(&next.to_string()));
    }
    precision.min(15)
}

fn decimal_places(text: &str) -> usize {
    let text = text.trim();
    if text.is_empty() {
        return 0;
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let (mantissa, exponent) = unsigned
        .split_once('e')
        .or_else(|| unsigned.split_once('E'))
        .map(|(mantissa, exponent)| (mantissa, exponent.parse::<i32>().unwrap_or_default()))
        .unwrap_or((unsigned, 0));
    let fraction_digits = mantissa
        .split_once('.')
        .map(|(_, fraction)| fraction.len())
        .unwrap_or(0) as i32;
    fraction_digits.saturating_sub(exponent).max(0) as usize
}

fn format_number(value: f64, precision: usize) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    if precision == 0 {
        format!("{value:.0}")
    } else {
        format!("{value:.precision$}")
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

fn color_channel_accessible_label(channel: ColorPickerChangeTrigger) -> &'static str {
    match channel {
        ColorPickerChangeTrigger::Red => "Red channel",
        ColorPickerChangeTrigger::Green => "Green channel",
        ColorPickerChangeTrigger::Blue => "Blue channel",
        ColorPickerChangeTrigger::Alpha => "Alpha channel",
        ColorPickerChangeTrigger::Swatch => "Color",
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
            format!(
                "Uploading {:.0}%",
                normalized_upload_progress(*progress) * 100.0
            )
        }
        UploadStatus::Complete => "Complete".to_string(),
        UploadStatus::Error(message) => format!("Error: {message}"),
    }
}

fn normalized_upload_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
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

fn component_input_style(context: &StyleContext<'_>) -> InputStyle {
    let mut style = InputStyle::default_for_density(context.theme, context.density);
    context.theme.components.input.apply(&mut style, context);
    style
}

fn component_button_style(context: &StyleContext<'_>, variant: ButtonVariantKind) -> ButtonStyle {
    let mut style = ButtonStyle::default_for_density(context.theme, context.density, variant);
    context.theme.components.button.apply(&mut style, context);
    style
}

fn component_select_style(context: &StyleContext<'_>) -> SelectStyle {
    let mut style = SelectStyle::default_for_density(context.theme, context.density);
    context.theme.components.select.apply(&mut style, context);
    style
}

fn mode_colors(context: &StyleContext<'_>) -> (bool, Color, Color, Color, Color, Color, Color) {
    let dark = matches!(context.mode, ResolvedThemeMode::Dark);
    let input = component_input_style(context);
    let select = component_select_style(context);
    let primary_button = component_button_style(context, ButtonVariantKind::Primary);
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
    let input = component_input_style(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(input.background.normal);
    style.surface.border_color = Some(input.border.normal);
    style.surface.border_width = Some(input.border_width);
    style.surface.border_radius = Some(input.radius);
    style.surface.shadow = None;
    style
}

fn input_panel_style(context: &StyleContext<'_>) -> ContainerStyle {
    let input = component_input_style(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(input.background.normal);
    style.surface.border_color = Some(input.border.normal);
    style.surface.border_width = Some(input.border_width);
    style.surface.border_radius = Some(input.radius);
    style.surface.shadow = None;
    style
}

fn upload_drop_zone_style(context: &StyleContext<'_>, state: WidgetState) -> ContainerStyle {
    let input = component_input_style(context);
    let background = if state.disabled {
        context.theme.colors.disabled
    } else if state.pressed {
        context.theme.colors.primary_container.darken(0.04)
    } else if state.hovered {
        context.theme.colors.primary_container
    } else {
        input.background.normal.resolve()
    };
    let border = if state.hovered || state.pressed {
        context.theme.colors.primary.with_alpha_factor(0.64)
    } else {
        input.border.normal.resolve()
    };
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(Value::Static(background));
    style.surface.border_color = Some(Value::Static(border));
    style.surface.border_width = Some(input.border_width);
    style.surface.border_radius = Some(Value::Static(context.theme.radius.lg));
    style.surface.shadow = None;
    style
}

fn color_trigger_accessible_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Secondary);
    style.foreground = value_color(
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
    );
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = advanced_input_metrics(context.theme).control_height;
    style
}

fn picker_popover_content_style(context: &StyleContext<'_>) -> ContainerStyle {
    let popover = PopoverStyle::default_for_theme(context.theme);
    let select = component_select_style(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(select.menu_background);
    style.surface.border_radius = Some(popover.radius);
    style.surface.shadow = None;
    style
}

fn icon_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Ghost);
    style.foreground = transparent_button_foreground();
    style.radius = Value::Static(dp(8.0));
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = dp(32.0);
    style
}

fn input_icon_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Secondary);
    style.foreground = transparent_button_foreground();
    style.radius = component_input_style(context).radius;
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = advanced_input_metrics(context.theme).control_height;
    style
}

fn upload_action_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Ghost);
    style.foreground = transparent_button_foreground();
    style.radius = Value::Static(context.theme.radius.lg);
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = advanced_input_metrics(context.theme).upload_action_size;
    style
}

fn number_step_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Secondary);
    style.foreground = transparent_button_foreground();
    style.radius = component_input_style(context).radius;
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = advanced_input_metrics(context.theme).control_height;
    style
}

fn transparent_button_foreground() -> StateValue<Value<Color>> {
    value_color(
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
    )
}

fn time_option_button_style(context: &StyleContext<'_>) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Secondary);
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
    let on_primary = context.theme.colors.on_primary;
    style.foreground = value_color(on_primary, on_primary, on_primary, muted);
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
    let mut style = component_button_style(context, ButtonVariantKind::Ghost);
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
        context.theme.colors.on_primary
    } else if same_month {
        text
    } else {
        muted
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

fn color_swatch_button_style(context: &StyleContext<'_>, color: Color) -> ButtonStyle {
    let mut style = component_button_style(context, ButtonVariantKind::Ghost);
    style.background = value_color(color, color.lighten(0.08), color.darken(0.08), color);
    let border = component_input_style(context).border.normal.resolve();
    style.border = value_color(border, border, border, border.with_alpha_factor(0.5));
    style.foreground = value_color(
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        Color::TRANSPARENT,
    );
    style.border_width = Value::Static(dp(1.0));
    style.radius = Value::Static(dp(7.0));
    style.padding_x = dp(0.0);
    style.padding_y = dp(0.0);
    style.min_height = dp(0.0);
    style
}

fn accent_badge_style(context: &StyleContext<'_>) -> ContainerStyle {
    let (_, primary, _, _, _, _, _) = mode_colors(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(Value::Static(primary.with_alpha_factor(0.12)));
    style.surface.border_color = Some(Value::Static(primary.with_alpha_factor(0.24)));
    style.surface.border_width = Some(Value::Static(dp(1.0)));
    style.surface.border_radius = Some(Value::Static(context.theme.radius.lg));
    style.surface.shadow = None;
    style
}

fn subtle_badge_style(context: &StyleContext<'_>) -> ContainerStyle {
    let (_, _, _, _, _, surface_low, outline) = mode_colors(context);
    let mut style = ContainerStyle::default_for_theme(context.theme);
    style.surface.background = Some(Value::Static(surface_low));
    style.surface.border_color = Some(Value::Static(outline));
    style.surface.border_width = Some(Value::Static(dp(1.0)));
    style.surface.border_radius = Some(Value::Static(context.theme.radius.lg));
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
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TemporaryUploadFile {
        path: PathBuf,
    }

    impl TemporaryUploadFile {
        fn new(name: &str, contents: &[u8]) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);

            let directory = std::env::temp_dir().join(format!(
                "tgui-upload-validation-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&directory).expect("create upload validation directory");
            let path = directory.join(name);
            std::fs::write(&path, contents).expect("write upload validation file");
            Self { path }
        }
    }

    impl Drop for TemporaryUploadFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            if let Some(directory) = self.path.parent() {
                let _ = std::fs::remove_dir(directory);
            }
        }
    }

    fn relative_luminance(color: Color) -> f32 {
        fn linear(channel: u8) -> f32 {
            let channel = f32::from(channel) / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }

    fn contrast_ratio(a: Color, b: Color) -> f32 {
        let (lighter, darker) = {
            let a = relative_luminance(a);
            let b = relative_luminance(b);
            (a.max(b), a.min(b))
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn calendar_month_has_six_weeks() {
        let days = calendar_days(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert_eq!(days.len(), 42);
        assert_eq!(days[0], Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()));
    }

    #[test]
    fn calendar_month_grid_is_defined_at_date_bounds() {
        assert_eq!(calendar_days(NaiveDate::MIN).len(), 42);
        assert_eq!(calendar_days(NaiveDate::MAX).len(), 42);
    }

    #[test]
    fn upload_validation_rejects_extensions() {
        let file = TemporaryUploadFile::new("a.exe", b"application");
        let selection =
            validate_upload_paths(vec![file.path.clone()], &["png".to_string()], None, None, 0);
        assert!(selection.files.is_empty());
        assert_eq!(selection.rejected.len(), 1);
    }

    #[test]
    fn upload_validation_counts_existing_files() {
        let first = TemporaryUploadFile::new("a.png", b"a");
        let second = TemporaryUploadFile::new("b.png", b"b");
        let selection = validate_upload_paths(
            vec![first.path.clone(), second.path.clone()],
            &["png".to_string()],
            Some(2),
            None,
            1,
        );
        assert_eq!(selection.files.len(), 1);
        assert_eq!(selection.rejected.len(), 1);
    }

    #[test]
    fn upload_validation_rejects_metadata_failures() {
        let missing = std::env::temp_dir().join(format!(
            "tgui-upload-missing-{}-{}",
            std::process::id(),
            AtomicU64::new(0).fetch_add(1, Ordering::Relaxed)
        ));
        let selection = validate_upload_paths(vec![missing], &[], None, None, 0);

        assert!(selection.files.is_empty());
        assert_eq!(selection.rejected.len(), 1);
        assert!(selection.rejected[0]
            .reason
            .starts_with("Unable to read file metadata:"));
    }

    #[cfg(unix)]
    #[test]
    fn upload_file_ids_distinguish_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let first = UploadFile::from_path(PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'f', b'i', b'l', b'e', b'-', 0x80,
        ])));
        let second = UploadFile::from_path(PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'f', b'i', b'l', b'e', b'-', 0x81,
        ])));

        assert_ne!(first.id, second.id);
    }

    #[test]
    fn upload_non_finite_progress_falls_back_to_zero() {
        for progress in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let file = UploadFile {
                id: UploadFileId::new("non-finite"),
                path: PathBuf::from("non-finite.bin"),
                name: "non-finite.bin".to_string(),
                size_bytes: None,
                status: UploadStatus::Uploading { progress },
            };
            assert_eq!(file.progress(), 0.0);
            assert_eq!(upload_status_text(&file.status), "Uploading 0%");
        }
    }

    #[test]
    fn number_clamp_respects_bounds() {
        assert_eq!(parse_number("42", Some(10.0), Some(20.0)), Some(20.0));
    }

    #[test]
    fn number_parser_rejects_non_finite_values() {
        assert_eq!(parse_number("NaN", None, None), None);
        assert_eq!(parse_number("inf", None, None), None);
        assert_eq!(parse_number("-inf", Some(-10.0), Some(10.0)), None);
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

    #[test]
    fn selected_time_and_calendar_controls_use_live_on_primary_token() {
        let mut theme = Theme::dark();
        theme.colors.primary = Color::hexa(0xF3F4F6FF);
        theme.colors.on_primary = Color::hexa(0x111827FF);
        let context = StyleContext::from_theme(&theme);

        let time = time_option_selected_button_style(&context);
        assert_eq!(time.foreground.normal.resolve(), theme.colors.on_primary);
        assert_eq!(time.foreground.hovered.resolve(), theme.colors.on_primary);

        let day = calendar_day_button_style(&context, true, false, true);
        assert_eq!(day.foreground.normal.resolve(), theme.colors.on_primary);
        assert_eq!(day.foreground.hovered.resolve(), theme.colors.on_primary);
    }

    #[test]
    fn adjacent_month_calendar_days_keep_normal_text_contrast() {
        for theme in [Theme::light(), Theme::dark()] {
            let context = StyleContext::from_theme(&theme);
            let style = calendar_day_button_style(&context, false, false, false);
            let foreground = style.foreground.normal.resolve();
            assert_eq!(foreground, theme.colors.on_surface_muted);
            assert!(
                contrast_ratio(foreground, theme.colors.surface) >= 4.5,
                "adjacent month text must meet WCAG AA in {:?} mode",
                theme.mode,
            );
        }
    }

    #[test]
    fn upload_drop_zone_uses_regular_control_radius() {
        let mut theme = Theme::light();
        theme.radius.lg = dp(7.0);
        theme.radius.xl = dp(13.0);
        let context = StyleContext::from_theme(&theme);
        let style = upload_drop_zone_style(&context, WidgetState::default());

        assert_eq!(
            style.surface.border_radius,
            Some(Value::Static(theme.radius.lg))
        );
    }
}
