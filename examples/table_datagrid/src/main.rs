use std::cmp::Ordering;

use tgui::prelude::*;

#[derive(Clone, PartialEq)]
struct Employee {
    id: &'static str,
    name: String,
    role: String,
    team: String,
    status: String,
}

impl Employee {
    fn new(
        id: &'static str,
        name: &'static str,
        role: &'static str,
        team: &'static str,
        status: &'static str,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            role: role.to_string(),
            team: team.to_string(),
            status: status.to_string(),
        }
    }
}

struct AppVm {
    rows: State<Vec<Employee>>,
    selected: State<Vec<WidgetKey>>,
    sort: State<Vec<DataGridSort>>,
    column_order: State<Vec<String>>,
    name_width: State<Dp>,
    role_width: State<Dp>,
    team_width: State<Dp>,
    status_width: State<Dp>,
    status: State<String>,
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            rows: ctx.state(vec![
                Employee::new(
                    "e-001",
                    "Ada Torres",
                    "Product Lead",
                    "Platform",
                    "Planning",
                ),
                Employee::new("e-002", "Mika Chen", "Designer", "Experience", "Reviewing"),
                Employee::new(
                    "e-003",
                    "Nora Patel",
                    "Researcher",
                    "Experience",
                    "Interviewing",
                ),
                Employee::new(
                    "e-004",
                    "Owen Blake",
                    "Runtime Engineer",
                    "Core",
                    "Implementing",
                ),
                Employee::new("e-005", "Li Wei", "Rendering Engineer", "Core", "Profiling"),
                Employee::new(
                    "e-006",
                    "Sam Rivera",
                    "Platform Engineer",
                    "Desktop",
                    "Testing",
                ),
                Employee::new(
                    "e-007",
                    "Iris Morgan",
                    "QA Engineer",
                    "Desktop",
                    "Validating",
                ),
                Employee::new("e-008", "Jun Park", "Data Engineer", "Tools", "Shipping"),
            ]),
            selected: ctx.state(vec![WidgetKey::from("e-001")]),
            sort: ctx.state(Vec::new()),
            column_order: ctx.state(vec![
                "id".to_string(),
                "name".to_string(),
                "role".to_string(),
                "team".to_string(),
                "status".to_string(),
            ]),
            name_width: ctx.state(dp(190.0)),
            role_width: ctx.state(dp(190.0)),
            team_width: ctx.state(dp(160.0)),
            status_width: ctx.state(dp(180.0)),
            status: ctx.state(
                "Click headers, drag column edges, reorder columns, or right-click cells."
                    .to_string(),
            ),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(14.0))
            .style_full(root_style)
            .child(Text::new("Table / DataGrid").style_full(title_style))
            .child(Text::new(self.summary_text()).style_full(muted_style))
            .child(
                DataGrid::new(self.sorted_rows(), self.columns())
                    .width(pct(100.0))
                    .height(dp(420.0))
                    .density(DataGridDensity::Regular)
                    .selection_mode(DataGridSelectionMode::Multiple)
                    .selected_keys(self.selected.signal())
                    .sort(self.sort.signal())
                    .row_height(dp(42.0))
                    .overscan(4)
                    .context_menu(vec![
                        MenuItem::new("Mark reviewed").on_select(Command::new(Self::mark_reviewed)),
                        MenuItem::new("Clear selection")
                            .on_select(Command::new(Self::clear_selection)),
                    ])
                    .on_selection_change(ValueCommand::new(Self::select_rows))
                    .on_sort_change(ValueCommand::new(Self::sort_rows))
                    .on_column_width_change(ValueCommand::new(Self::resize_column))
                    .on_column_reorder(ValueCommand::new(Self::reorder_column))
                    .on_cell_action(ValueCommand::new(Self::open_cell))
                    .on_cell_edit_commit(ValueCommand::new(Self::commit_cell_edit)),
            )
            .child(Text::new(self.status.signal()).style_full(status_style))
            .into()
    }
}

impl AppVm {
    fn summary_text(&self) -> String {
        let selected = self.selected.get();
        let sort = self.sort.get();
        let sort_text = if sort.is_empty() {
            "unsorted".to_string()
        } else {
            sort.iter()
                .map(|entry| format!("{:?} {:?}", entry.column_key, entry.direction))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!("{} selected, sort: {sort_text}", selected.len())
    }

    fn sorted_rows(&self) -> Vec<DataGridRow<Employee>> {
        let mut rows = self.rows.get();
        for sort in self.sort.get().iter().rev() {
            rows.sort_by(|left, right| compare_employee(left, right, sort));
        }
        rows.into_iter()
            .map(|employee| DataGridRow::keyed(employee.id, employee))
            .collect()
    }

    fn columns(&self) -> Vec<DataGridColumn<Employee, Self>> {
        self.column_order
            .get()
            .into_iter()
            .filter_map(|key| self.column_for(&key))
            .collect()
    }

    fn column_for(&self, key: &str) -> Option<DataGridColumn<Employee, Self>> {
        match key {
            "id" => Some(
                DataGridColumn::new(
                    "id",
                    "ID".to_string(),
                    |ctx: DataGridCellContext<Employee>| Text::new(ctx.row.id).into(),
                )
                .width(dp(86.0))
                .min_width(dp(72.0))
                .resizable(false)
                .reorderable(false)
                .pin(DataGridColumnPin::Start),
            ),
            "name" => Some(
                DataGridColumn::new(
                    "name",
                    "Name".to_string(),
                    |ctx: DataGridCellContext<Employee>| Text::new(ctx.row.name).into(),
                )
                .width(self.name_width.signal())
                .min_width(dp(140.0))
                .max_width(dp(260.0))
                .sortable(true)
                .text_value(|row| row.name.clone())
                .editable(true),
            ),
            "role" => Some(
                DataGridColumn::new(
                    "role",
                    "Role".to_string(),
                    |ctx: DataGridCellContext<Employee>| Text::new(ctx.row.role).into(),
                )
                .width(self.role_width.signal())
                .min_width(dp(140.0))
                .max_width(dp(280.0))
                .sortable(true)
                .text_value(|row| row.role.clone())
                .editable(true),
            ),
            "team" => Some(
                DataGridColumn::new(
                    "team",
                    "Team".to_string(),
                    |ctx: DataGridCellContext<Employee>| Text::new(ctx.row.team).into(),
                )
                .width(self.team_width.signal())
                .min_width(dp(120.0))
                .max_width(dp(220.0))
                .sortable(true),
            ),
            "status" => Some(
                DataGridColumn::new(
                    "status",
                    "Status".to_string(),
                    |ctx: DataGridCellContext<Employee>| Text::new(ctx.row.status).into(),
                )
                .width(self.status_width.signal())
                .min_width(dp(140.0))
                .max_width(dp(240.0))
                .sortable(true)
                .pin(DataGridColumnPin::End)
                .text_value(|row| row.status.clone())
                .editable(true),
            ),
            _ => None,
        }
    }

    fn select_rows(&mut self, change: DataGridSelectionChange) {
        let count = change.selected_keys.len();
        self.selected.set(change.selected_keys);
        self.status.set(format!(
            "Selection via {:?}; focused={:?}; selected={count}",
            change.trigger, change.focused_key
        ));
    }

    fn sort_rows(&mut self, change: DataGridSortChange) {
        let count = change.sort.len();
        self.sort.set(change.sort);
        self.status.set(format!(
            "Sort changed by {:?}; descriptors={count}",
            change.trigger
        ));
    }

    fn resize_column(&mut self, change: DataGridColumnWidthChange) {
        if change.column_key == WidgetKey::from("name") {
            self.name_width.set(change.width);
        } else if change.column_key == WidgetKey::from("role") {
            self.role_width.set(change.width);
        } else if change.column_key == WidgetKey::from("team") {
            self.team_width.set(change.width);
        } else if change.column_key == WidgetKey::from("status") {
            self.status_width.set(change.width);
        }
        self.status.set(format!(
            "Column {:?} resized to {}",
            change.column_key, change.width
        ));
    }

    fn reorder_column(&mut self, event: DataGridColumnReorderEvent) {
        self.column_order.update(|order| {
            if event.from_index >= order.len() || event.to_index >= order.len() {
                return;
            }
            let column = order.remove(event.from_index);
            order.insert(event.to_index, column);
        });
        self.status.set(format!(
            "Column {:?} moved before/onto {:?}",
            event.column_key, event.target_key
        ));
    }

    fn open_cell(&mut self, action: DataGridCellAction) {
        self.status.set(format!(
            "Cell action row={} column={:?}",
            action.row_index, action.column_key
        ));
    }

    fn commit_cell_edit(&mut self, commit: DataGridCellEditCommit) {
        self.rows.update(|rows| {
            if let Some(row) = rows
                .iter_mut()
                .find(|row| WidgetKey::from(row.id) == commit.row_key)
            {
                let value = format!("{}*", commit.value.trim_end_matches('*'));
                if commit.column_key == WidgetKey::from("name") {
                    row.name = value;
                } else if commit.column_key == WidgetKey::from("role") {
                    row.role = value;
                } else if commit.column_key == WidgetKey::from("status") {
                    row.status = value;
                }
            }
        });
        self.status.set(format!(
            "Committed edit for row {:?}, column {:?}",
            commit.row_key, commit.column_key
        ));
    }

    fn mark_reviewed(&mut self) {
        let selected = self.selected.get();
        self.rows.update(|rows| {
            for row in rows {
                if selected.iter().any(|key| key == &WidgetKey::from(row.id)) {
                    row.status = "Reviewed".to_string();
                }
            }
        });
        self.status
            .set("Marked selected rows as reviewed".to_string());
    }

    fn clear_selection(&mut self) {
        self.selected.set(Vec::new());
        self.status.set("Selection cleared".to_string());
    }
}

fn compare_employee(left: &Employee, right: &Employee, sort: &DataGridSort) -> Ordering {
    let ordering = if sort.column_key == WidgetKey::from("name") {
        left.name.cmp(&right.name)
    } else if sort.column_key == WidgetKey::from("role") {
        left.role.cmp(&right.role)
    } else if sort.column_key == WidgetKey::from("team") {
        left.team.cmp(&right.team)
    } else if sort.column_key == WidgetKey::from("status") {
        left.status.cmp(&right.status)
    } else {
        left.id.cmp(right.id)
    };
    match sort.direction {
        DataGridSortDirection::Ascending => ordering,
        DataGridSortDirection::Descending => ordering.reverse(),
    }
}

fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xF8FAFCFF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x0B1120FF).into(),
    });
    style
}

fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(26.0);
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn muted_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(14.0);
    style.color = match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0x475569FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0xCBD5E1FF).into(),
    };
    style
}

fn status_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_style(ctx);
    style.typography.weight = FontWeight::Medium;
    style
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui Table / DataGrid")
        .window_size(dp(900.0), dp(620.0))
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
