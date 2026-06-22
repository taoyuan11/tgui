use std::cmp::Ordering;
use std::time::Duration;

use tgui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskStatus {
    Queued,
    Running,
    Blocked,
    Done,
    Failed,
}

impl TaskStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Blocked => "Blocked",
            Self::Done => "Done",
            Self::Failed => "Failed",
        }
    }

    fn tone(self) -> BadgeTone {
        match self {
            Self::Queued => BadgeTone::Neutral,
            Self::Running => BadgeTone::Primary,
            Self::Blocked => BadgeTone::Warning,
            Self::Done => BadgeTone::Success,
            Self::Failed => BadgeTone::Error,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Task {
    id: String,
    title: String,
    owner: String,
    status: TaskStatus,
    progress: u32,
    updated: String,
}

struct TaskConsole {
    form: Form,
    title: TextFormField,
    owner: TextFormField,
    tasks: State<Vec<Task>>,
    selected: State<Vec<WidgetKey>>,
    sort: State<Vec<DataGridSort>>,
    filter: State<String>,
    activity: State<String>,
    next_id: State<u32>,
    toast_queue: ToastQueue<Self>,
}

impl ViewModel for TaskConsole {
    fn new(ctx: &ViewModelContext) -> Self {
        let form = Form::new(ctx);
        let title = form
            .text_field("title", "")
            .validator(|value| required(value, "Task title is required"));
        let owner = form
            .text_field("owner", "Platform")
            .validator(|value| required(value, "Owner is required"));

        Self {
            form,
            title,
            owner,
            tasks: ctx.state(seed_tasks()),
            selected: ctx.state(vec![WidgetKey::from("task-101")]),
            sort: ctx.state(Vec::new()),
            filter: ctx.state("All".to_string()),
            activity: ctx.state("Ready for release validation.".to_string()),
            next_id: ctx.state(106),
            toast_queue: ToastQueue::new(ctx),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .child(
                Flex::vertical()
                    .size(pct(100.0), pct(100.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(14.0))
                    .style_full(root_style)
                    .child(header())
                    .child(self.summary_strip())
                    .child(self.creation_panel())
                    .child(self.filter_bar())
                    .child(self.task_grid())
                    .child(Text::new(self.activity.signal()).style_full(status_text_style)),
            )
            .child(ToastHost::new(self.toast_queue.clone()))
            .into()
    }
}

impl TaskConsole {
    fn summary_strip(&self) -> Element<Self> {
        let summary = self.tasks.signal().map(|tasks| {
            let running = tasks
                .iter()
                .filter(|task| task.status == TaskStatus::Running)
                .count();
            let blocked = tasks
                .iter()
                .filter(|task| task.status == TaskStatus::Blocked)
                .count();
            let done = tasks
                .iter()
                .filter(|task| task.status == TaskStatus::Done)
                .count();
            format!(
                "{} total  |  {} running  |  {} blocked  |  {} done",
                tasks.len(),
                running,
                blocked,
                done
            )
        });

        Flex::horizontal()
            .width(pct(100.0))
            .gap(dp(10.0))
            .wrap(Wrap::Wrap)
            .child(Badge::text("0.3 gate").tone(BadgeTone::Primary))
            .child(Badge::text("CI").tone(BadgeTone::Success))
            .child(Badge::text("Security").tone(BadgeTone::Warning))
            .child(Text::new(summary).style_full(muted_text_style))
            .into()
    }

    fn creation_panel(&self) -> Element<Self> {
        Flex::vertical()
            .width(pct(100.0))
            .padding(Insets::all(dp(16.0)))
            .gap(dp(10.0))
            .style_full(panel_style)
            .child(Text::new("Create task").style_full(section_title_style))
            .child(
                Flex::horizontal()
                    .gap(dp(10.0))
                    .wrap(Wrap::Wrap)
                    .child(
                        Input::new(self.title.controller())
                            .width(dp(280.0))
                            .placeholder("Task title")
                            .validation(self.title.validation_state()),
                    )
                    .child(
                        Input::new(self.owner.controller())
                            .width(dp(180.0))
                            .placeholder("Owner")
                            .validation(self.owner.validation_state()),
                    )
                    .child(
                        Button::new("Create")
                            .primary()
                            .on_click(Command::new(Self::create_task)),
                    )
                    .child(
                        Button::new("Reset")
                            .secondary()
                            .on_click(Command::new(Self::reset_form)),
                    ),
            )
            .child(Text::new(self.form_error_text()).style_full(error_text_style))
            .into()
    }

    fn filter_bar(&self) -> Element<Self> {
        Flex::horizontal()
            .width(pct(100.0))
            .gap(dp(8.0))
            .wrap(Wrap::Wrap)
            .child(filter_button("All", self.filter.get() == "All"))
            .child(filter_button("Running", self.filter.get() == "Running"))
            .child(filter_button("Blocked", self.filter.get() == "Blocked"))
            .child(filter_button("Done", self.filter.get() == "Done"))
            .child(
                Button::new("Advance selected")
                    .primary()
                    .on_click(Command::new(Self::advance_selected)),
            )
            .child(
                Button::new("Run background")
                    .secondary()
                    .on_click(Command::new_with_context(Self::run_background)),
            )
            .child(
                Button::new("Fail selected")
                    .danger()
                    .on_click(Command::new(Self::fail_selected)),
            )
            .child(
                Button::new("Confirm clear")
                    .secondary()
                    .on_click(Command::new_with_context(Self::confirm_clear_done)),
            )
            .child(
                Button::new("Notify")
                    .secondary()
                    .on_click(Command::new_with_context(Self::notify_summary)),
            )
            .into()
    }

    fn task_grid(&self) -> Element<Self> {
        DataGrid::new(self.visible_rows(), self.columns())
            .width(pct(100.0))
            .height(dp(360.0))
            .density(DataGridDensity::Compact)
            .selection_mode(DataGridSelectionMode::Multiple)
            .selected_keys(self.selected.signal())
            .sort(self.sort.signal())
            .row_height(dp(44.0))
            .overscan(4)
            .empty(Text::new("No tasks match this filter.").style_full(muted_text_style))
            .context_menu(vec![
                MenuItem::new("Advance").on_select(Command::new(Self::advance_selected)),
                MenuItem::new("Mark failed").on_select(Command::new(Self::fail_selected)),
                MenuItem::new("Clear selection").on_select(Command::new(Self::clear_selection)),
            ])
            .on_selection_change(ValueCommand::new(Self::select_rows))
            .on_sort_change(ValueCommand::new(Self::sort_rows))
            .on_cell_edit_commit(ValueCommand::new(Self::commit_cell_edit))
            .into()
    }

    fn visible_rows(&self) -> Vec<DataGridRow<Task>> {
        let filter = self.filter.get();
        let mut rows = self
            .tasks
            .get()
            .into_iter()
            .filter(|task| match filter.as_str() {
                "Running" => task.status == TaskStatus::Running,
                "Blocked" => task.status == TaskStatus::Blocked,
                "Done" => task.status == TaskStatus::Done,
                _ => true,
            })
            .collect::<Vec<_>>();

        for sort in self.sort.get().iter().rev() {
            rows.sort_by(|left, right| compare_task(left, right, sort));
        }

        rows.into_iter()
            .map(|task| DataGridRow::keyed(task.id.clone(), task))
            .collect()
    }

    fn columns(&self) -> Vec<DataGridColumn<Task, Self>> {
        vec![
            DataGridColumn::new("id", "ID".to_string(), |ctx: DataGridCellContext<Task>| {
                Text::new(ctx.row.id.clone()).into()
            })
            .width(dp(92.0))
            .min_width(dp(80.0))
            .pin(DataGridColumnPin::Start)
            .resizable(false)
            .reorderable(false),
            DataGridColumn::new(
                "title",
                "Task".to_string(),
                |ctx: DataGridCellContext<Task>| Text::new(ctx.row.title.clone()).into(),
            )
            .width(dp(260.0))
            .min_width(dp(180.0))
            .sortable(true)
            .text_value(|task| task.title.clone())
            .editable(true),
            DataGridColumn::new(
                "owner",
                "Owner".to_string(),
                |ctx: DataGridCellContext<Task>| Text::new(ctx.row.owner.clone()).into(),
            )
            .width(dp(150.0))
            .sortable(true)
            .text_value(|task| task.owner.clone())
            .editable(true),
            DataGridColumn::new(
                "status",
                "Status".to_string(),
                |ctx: DataGridCellContext<Task>| {
                    Badge::text(ctx.row.status.label())
                        .tone(ctx.row.status.tone())
                        .into()
                },
            )
            .width(dp(130.0))
            .sortable(true)
            .text_value(|task| task.status.label().to_string()),
            DataGridColumn::new(
                "progress",
                "Progress".to_string(),
                |ctx: DataGridCellContext<Task>| {
                    Flex::vertical()
                        .gap(dp(4.0))
                        .child(
                            ProgressBar::<TaskConsole>::new(ctx.row.progress as f32 / 100.0)
                                .height(dp(8.0)),
                        )
                        .child(
                            Text::new(format!("{}%", ctx.row.progress)).style_full(tiny_text_style),
                        )
                        .into()
                },
            )
            .width(dp(150.0))
            .sortable(true)
            .text_value(|task| format!("{:03}", task.progress)),
            DataGridColumn::new(
                "updated",
                "Updated".to_string(),
                |ctx: DataGridCellContext<Task>| Text::new(ctx.row.updated.clone()).into(),
            )
            .width(dp(150.0))
            .sortable(true)
            .pin(DataGridColumnPin::End),
        ]
    }

    fn form_error_text(&self) -> String {
        let snapshot = self.form.snapshot();
        if snapshot.is_valid() {
            return "New tasks start queued at 0% and can be advanced from the grid.".to_string();
        }
        let title = self
            .title
            .first_error()
            .get()
            .unwrap_or_else(|| "Fix validation errors before creating a task.".to_string());
        let owner = self.owner.first_error().get().unwrap_or_default();
        if owner.is_empty() {
            title
        } else {
            format!("{title} {owner}")
        }
    }

    fn create_task(&mut self) {
        let snapshot = self.form.submit();
        if !snapshot.is_valid() {
            self.activity
                .set("Task form has validation errors.".to_string());
            self.toast_queue.push(
                Toast::new("Title and owner are required.")
                    .title("Validation failed")
                    .kind(ToastKind::Warning),
            );
            return;
        }

        let id_number = self.next_id.get();
        self.next_id.set(id_number + 1);
        let task = Task {
            id: format!("task-{id_number}"),
            title: snapshot
                .get::<String>("title")
                .unwrap_or_else(|| "Untitled task".to_string())
                .trim()
                .to_string(),
            owner: snapshot
                .get::<String>("owner")
                .unwrap_or_else(|| "Unassigned".to_string())
                .trim()
                .to_string(),
            status: TaskStatus::Queued,
            progress: 0,
            updated: "just now".to_string(),
        };
        self.tasks.update(|tasks| tasks.push(task.clone()));
        self.title.set_text("");
        self.form.clear_errors();
        self.activity.set(format!("Created {}.", task.id));
        self.toast_queue.push(
            Toast::new(format!("{} is ready to run.", task.title))
                .title("Task created")
                .kind(ToastKind::Success),
        );
    }

    fn reset_form(&mut self) {
        self.form.reset();
        self.activity.set("Form reset.".to_string());
    }

    fn select_rows(&mut self, change: DataGridSelectionChange) {
        let count = change.selected_keys.len();
        self.selected.set(change.selected_keys);
        self.activity.set(format!("{count} task(s) selected."));
    }

    fn sort_rows(&mut self, change: DataGridSortChange) {
        self.sort.set(change.sort);
        self.activity.set("Grid sort updated.".to_string());
    }

    fn commit_cell_edit(&mut self, commit: DataGridCellEditCommit) {
        let value = commit.value.trim().to_string();
        let row_key = commit.row_key;
        let column_key = commit.column_key;
        let row_label = format!("{row_key:?}");
        self.tasks.update(|tasks| {
            if let Some(task) = tasks
                .iter_mut()
                .find(|task| WidgetKey::from(&task.id) == row_key)
            {
                if column_key == WidgetKey::from("title") {
                    task.title = value.clone();
                } else if column_key == WidgetKey::from("owner") {
                    task.owner = value.clone();
                }
                task.updated = "edited".to_string();
            }
        });
        self.activity.set(format!("Edited {row_label}."));
    }

    fn advance_selected(&mut self) {
        let selected = self.selected_ids();
        if selected.is_empty() {
            self.activity
                .set("Select at least one task first.".to_string());
            return;
        }
        let mut completed = 0usize;
        self.tasks.update(|tasks| {
            for task in tasks
                .iter_mut()
                .filter(|task| selected.contains(&WidgetKey::from(&task.id)))
            {
                match task.status {
                    TaskStatus::Done | TaskStatus::Failed => {}
                    TaskStatus::Blocked => {
                        task.status = TaskStatus::Running;
                        task.updated = "unblocked".to_string();
                    }
                    _ => {
                        task.status = TaskStatus::Running;
                        task.progress = (task.progress + 25).min(100);
                        task.updated = "advanced".to_string();
                        if task.progress == 100 {
                            task.status = TaskStatus::Done;
                            task.updated = "completed".to_string();
                            completed += 1;
                        }
                    }
                }
            }
        });
        self.activity
            .set(format!("Advanced {} selected task(s).", selected.len()));
        if completed > 0 {
            self.toast_queue.push(
                Toast::new(format!("{completed} task(s) reached 100%."))
                    .title("Completed")
                    .kind(ToastKind::Success),
            );
        }
    }

    fn run_background(&mut self, ctx: &CommandContext<Self>) {
        let selected = self.selected_ids();
        if selected.is_empty() {
            self.activity
                .set("Select at least one task first.".to_string());
            self.toast_queue.push(
                Toast::new("Choose rows before launching a background run.")
                    .title("No selection")
                    .kind(ToastKind::Warning),
            );
            return;
        }

        self.tasks.update(|tasks| {
            for task in tasks
                .iter_mut()
                .filter(|task| selected.contains(&WidgetKey::from(&task.id)))
            {
                if !matches!(task.status, TaskStatus::Done | TaskStatus::Failed) {
                    task.status = TaskStatus::Running;
                    task.updated = "worker queued".to_string();
                }
            }
        });
        self.activity.set(format!(
            "Background run started for {} task(s).",
            selected.len()
        ));
        self.toast_queue.push(
            Toast::new("Simulated worker will report progress shortly.")
                .title("Background run")
                .kind(ToastKind::Info),
        );

        ctx.tasks().spawn_blocking(
            {
                let selected = selected.clone();
                move || {
                    std::thread::sleep(Duration::from_millis(350));
                    selected
                }
            },
            |app, selected, ctx| {
                let mut completed = 0usize;
                app.tasks.update(|tasks| {
                    for task in tasks
                        .iter_mut()
                        .filter(|task| selected.contains(&WidgetKey::from(&task.id)))
                    {
                        if matches!(task.status, TaskStatus::Done | TaskStatus::Failed) {
                            continue;
                        }
                        task.progress = (task.progress + 35).min(100);
                        task.status = if task.progress == 100 {
                            completed += 1;
                            TaskStatus::Done
                        } else {
                            TaskStatus::Running
                        };
                        task.updated = "worker tick".to_string();
                    }
                });
                app.activity.set(format!(
                    "Background worker reported progress for {} task(s).",
                    selected.len()
                ));
                app.toast_queue.push(
                    Toast::new(format!(
                        "{} task(s) received worker progress.",
                        selected.len()
                    ))
                    .title("Progress updated")
                    .kind(ToastKind::Success),
                );
                if completed > 0 {
                    let _ = ctx.notifications().send(
                        NotificationOptions::new("Task Console")
                            .body(format!("{completed} task(s) completed in the background."))
                            .app_name("Task Console"),
                    );
                }
            },
        );
    }

    fn fail_selected(&mut self) {
        let selected = self.selected_ids();
        if selected.is_empty() {
            self.activity
                .set("Select at least one task first.".to_string());
            return;
        }
        self.tasks.update(|tasks| {
            for task in tasks
                .iter_mut()
                .filter(|task| selected.contains(&WidgetKey::from(&task.id)))
            {
                task.status = TaskStatus::Failed;
                task.updated = "failed".to_string();
            }
        });
        self.toast_queue.push(
            Toast::new(format!("{} task(s) marked failed.", selected.len()))
                .title("Attention")
                .kind(ToastKind::Error),
        );
        self.activity.set("Failure state applied.".to_string());
    }

    fn clear_selection(&mut self) {
        self.selected.set(Vec::new());
        self.activity.set("Selection cleared.".to_string());
    }

    fn confirm_clear_done(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.dialogs().show_message(
            MessageDialogOptions::new()
                .title("Clear completed tasks")
                .description("Remove all completed tasks from the console?")
                .level(MessageDialogLevel::Warning)
                .buttons(MessageDialogButtons::YesNo),
        );
        match result {
            Ok(MessageDialogResult::Yes) => {
                self.tasks
                    .update(|tasks| tasks.retain(|task| task.status != TaskStatus::Done));
                self.activity.set("Completed tasks cleared.".to_string());
                self.toast_queue.push(
                    Toast::new("Completed tasks were removed.")
                        .title("Cleared")
                        .kind(ToastKind::Info),
                );
            }
            Ok(_) => self.activity.set("Clear cancelled.".to_string()),
            Err(error) => self.activity.set(format!("Dialog failed: {error}")),
        }
    }

    fn notify_summary(&mut self, ctx: &CommandContext<Self>) {
        let tasks = self.tasks.get();
        let running = tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Running)
            .count();
        let blocked = tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Blocked)
            .count();
        let result = ctx.notifications().send(
            NotificationOptions::new("Task Console")
                .body(format!("{running} running, {blocked} blocked."))
                .app_name("Task Console"),
        );
        self.activity.set(match result {
            Ok(id) => format!("Notification sent: {id}"),
            Err(error) => format!("Notification failed: {error}"),
        });
    }

    fn set_filter(&mut self, value: &str) {
        self.filter.set(value.to_string());
        self.selected.set(Vec::new());
        self.activity.set(format!("Filter set to {value}."));
    }

    fn selected_ids(&self) -> Vec<WidgetKey> {
        self.selected.get()
    }
}

fn required(value: &str, message: &str) -> ValidationErrors {
    if value.trim().is_empty() {
        ValidationErrors::single(message)
    } else {
        ValidationErrors::none()
    }
}

fn seed_tasks() -> Vec<Task> {
    vec![
        Task {
            id: "task-101".to_string(),
            title: "Review release checklist".to_string(),
            owner: "Release".to_string(),
            status: TaskStatus::Running,
            progress: 75,
            updated: "09:20".to_string(),
        },
        Task {
            id: "task-102".to_string(),
            title: "Audit media URL defaults".to_string(),
            owner: "Security".to_string(),
            status: TaskStatus::Done,
            progress: 100,
            updated: "10:05".to_string(),
        },
        Task {
            id: "task-103".to_string(),
            title: "Smoke test IME candidate placement".to_string(),
            owner: "Desktop".to_string(),
            status: TaskStatus::Blocked,
            progress: 35,
            updated: "11:12".to_string(),
        },
        Task {
            id: "task-104".to_string(),
            title: "Record audio packaging notes".to_string(),
            owner: "Media".to_string(),
            status: TaskStatus::Queued,
            progress: 0,
            updated: "12:40".to_string(),
        },
        Task {
            id: "task-105".to_string(),
            title: "Add scene snapshot regression".to_string(),
            owner: "Rendering".to_string(),
            status: TaskStatus::Failed,
            progress: 20,
            updated: "13:15".to_string(),
        },
    ]
}

fn compare_task(left: &Task, right: &Task, sort: &DataGridSort) -> Ordering {
    let ordering = if sort.column_key == WidgetKey::from("title") {
        left.title.cmp(&right.title)
    } else if sort.column_key == WidgetKey::from("owner") {
        left.owner.cmp(&right.owner)
    } else if sort.column_key == WidgetKey::from("status") {
        left.status.label().cmp(right.status.label())
    } else if sort.column_key == WidgetKey::from("progress") {
        left.progress.cmp(&right.progress)
    } else if sort.column_key == WidgetKey::from("updated") {
        left.updated.cmp(&right.updated)
    } else {
        left.id.cmp(&right.id)
    };
    match sort.direction {
        DataGridSortDirection::Ascending => ordering,
        DataGridSortDirection::Descending => ordering.reverse(),
    }
}

fn header() -> Element<TaskConsole> {
    Flex::vertical()
        .gap(dp(4.0))
        .child(Text::new("Task Console").style_full(title_text_style))
        .child(Text::new("0.3 production readiness workflow").style_full(muted_text_style))
        .into()
}

fn filter_button(label: &'static str, active: bool) -> Button<TaskConsole> {
    let button = Button::new(label).on_click(Command::new(move |app: &mut TaskConsole| {
        app.set_filter(label);
    }));
    if active {
        button.primary()
    } else {
        button.secondary()
    }
}

fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xF8FAFCFF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x111827FF).into(),
    });
    style
}

fn panel_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style.surface.border_color = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xCBD5E1FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x334155FF).into(),
    });
    style
}

fn title_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(28.0);
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn section_title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(17.0);
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn muted_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(14.0);
    style.color = match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0x475569FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0xCBD5E1FF).into(),
    };
    style
}

fn status_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_text_style(ctx);
    style.typography.weight = FontWeight::Medium;
    style
}

fn tiny_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_text_style(ctx);
    style.typography.size = sp(11.0);
    style
}

fn error_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_text_style(ctx);
    style.color = Color::hexa(0xB91C1CFF).into();
    style
}

fn main() -> tgui::core::Result<()> {
    Application::new()
        .app_id("dev.tgui.examples.task-console")
        .title("Task Console")
        .window_size(dp(1120.0), dp(760.0))
        .with_view_model(TaskConsole::new)
        .root_view(TaskConsole::view)
        .run()
}
