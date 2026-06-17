use std::sync::Arc;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, Align, Insets, LayoutStyle, Overflow, ScrollbarStyle, Value};
use crate::ui::unit::{dp, Dp};

use super::common::{
    ChildSource, CursorStyle, DataGridCellState, DataGridHeaderState, DataGridResizeHandleState,
    DataGridRootState, FocusScopeOptions, InteractionHandlers, LifecycleEventHandlers,
    MediaEventHandlers, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::r#virtual::{ItemLayout, ItemSource, VirtualList};
use super::style::palette::palette_from_theme;
use super::style::{StyleResolver, StyleSheet, WidgetSurfaceStyle};
use super::{
    ContainerStyle, ContextMenuDescriptor, Flex, GestureRecognizer, LongPressEvent, MenuItem,
    MenuItemState, Stack, Text, TextWidgetStyle,
};

pub type Table<T, VM> = DataGrid<T, VM>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridSelectionMode {
    None,
    Single,
    Multiple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridDensity {
    Compact,
    Regular,
    Spacious,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridColumnPin {
    None,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridSortDirection {
    Ascending,
    Descending,
}

impl DataGridSortDirection {
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridSort {
    pub column_key: WidgetKey,
    pub direction: DataGridSortDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridSelectionTrigger {
    Click,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridSortTrigger {
    HeaderClick,
    Keyboard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridSelectionChange {
    pub selected_keys: Vec<WidgetKey>,
    pub focused_key: Option<WidgetKey>,
    pub anchor_key: Option<WidgetKey>,
    pub changed_key: Option<WidgetKey>,
    pub trigger: DataGridSelectionTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridSortChange {
    pub sort: Vec<DataGridSort>,
    pub changed_column: WidgetKey,
    pub trigger: DataGridSortTrigger,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataGridColumnWidthChange {
    pub column_key: WidgetKey,
    pub width: Dp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridColumnReorderEvent {
    pub from_index: usize,
    pub to_index: usize,
    pub column_key: WidgetKey,
    pub target_key: WidgetKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridCellAction {
    pub row_index: usize,
    pub row_key: WidgetKey,
    pub column_index: usize,
    pub column_key: WidgetKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGridCellEditCommit {
    pub row_index: usize,
    pub row_key: WidgetKey,
    pub column_index: usize,
    pub column_key: WidgetKey,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct DataGridCellContext<T> {
    pub row_index: usize,
    pub row_key: WidgetKey,
    pub column_index: usize,
    pub column_key: WidgetKey,
    pub row: T,
    pub selected: bool,
    pub disabled: bool,
    pub editing: bool,
}

#[derive(Clone, Debug)]
pub struct DataGridHeaderContext {
    pub column_index: usize,
    pub column_key: WidgetKey,
    pub label: String,
    pub sort_direction: Option<DataGridSortDirection>,
    pub sort_priority: Option<usize>,
}

#[derive(Clone)]
pub struct DataGridRow<T> {
    key: Option<WidgetKey>,
    value: T,
    disabled: Value<bool>,
}

impl<T> DataGridRow<T> {
    pub fn new(value: T) -> Self {
        Self {
            key: None,
            value,
            disabled: Value::Static(false),
        }
    }

    pub fn keyed(key: impl Into<WidgetKey>, value: T) -> Self {
        Self::new(value).key(key)
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }
}

impl<T> From<T> for DataGridRow<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Clone)]
pub struct DataGridSection<T, VM> {
    header: Element<VM>,
    rows: Vec<DataGridRow<T>>,
}

impl<T, VM> DataGridSection<T, VM> {
    pub fn new(header: impl Into<Element<VM>>, rows: Vec<DataGridRow<T>>) -> Self {
        Self {
            header: header.into(),
            rows,
        }
    }
}

pub struct DataGridColumn<T, VM> {
    key: WidgetKey,
    label: Value<String>,
    render: Arc<dyn Fn(DataGridCellContext<T>) -> Element<VM> + Send + Sync>,
    header: Option<Arc<dyn Fn(DataGridHeaderContext) -> Element<VM> + Send + Sync>>,
    text_value: Option<Arc<dyn Fn(&T) -> String + Send + Sync>>,
    width: Value<Dp>,
    min_width: Dp,
    max_width: Option<Dp>,
    sortable: bool,
    resizable: bool,
    reorderable: bool,
    editable: bool,
    pin: DataGridColumnPin,
    align: Align,
}

impl<T, VM> Clone for DataGridColumn<T, VM> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            label: self.label.clone(),
            render: self.render.clone(),
            header: self.header.clone(),
            text_value: self.text_value.clone(),
            width: self.width.clone(),
            min_width: self.min_width,
            max_width: self.max_width,
            sortable: self.sortable,
            resizable: self.resizable,
            reorderable: self.reorderable,
            editable: self.editable,
            pin: self.pin,
            align: self.align,
        }
    }
}

impl<T, VM> DataGridColumn<T, VM> {
    pub fn new(
        key: impl Into<WidgetKey>,
        label: impl Into<Value<String>>,
        render: impl Fn(DataGridCellContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            render: Arc::new(render),
            header: None,
            text_value: None,
            width: Value::Static(dp(160.0)),
            min_width: dp(56.0),
            max_width: None,
            sortable: false,
            resizable: true,
            reorderable: true,
            editable: false,
            pin: DataGridColumnPin::None,
            align: Align::Stretch,
        }
    }

    pub fn width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.width = width.into();
        self
    }

    pub fn min_width(mut self, width: Dp) -> Self {
        self.min_width = width.max(Dp::ZERO);
        self
    }

    pub fn max_width(mut self, width: Dp) -> Self {
        self.max_width = Some(width.max(Dp::ZERO));
        self
    }

    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    pub fn pin(mut self, pin: DataGridColumnPin) -> Self {
        self.pin = pin;
        self
    }

    pub fn text_value(mut self, value: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        self.text_value = Some(Arc::new(value));
        self
    }

    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn header(
        mut self,
        render: impl Fn(DataGridHeaderContext) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        self.header = Some(Arc::new(render));
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataGridStyle {
    pub surface: WidgetSurfaceStyle,
    pub header_height: Dp,
    pub compact_row_height: Dp,
    pub regular_row_height: Dp,
    pub spacious_row_height: Dp,
    pub cell_padding: Insets,
    pub header_background: Value<Color>,
    pub header_text: Value<Color>,
    pub row_background: Value<Color>,
    pub zebra_background: Value<Color>,
    pub row_hover_background: Value<Color>,
    pub row_selected_background: Value<Color>,
    pub cell_focused_border: Value<Color>,
    pub cell_editing_background: Value<Color>,
    pub grid_line: Value<Color>,
    pub resize_handle: Value<Color>,
    pub sort_indicator: Value<Color>,
    pub scrollbar: ScrollbarStyle,
}

impl DataGridStyle {
    pub fn default_for_theme(theme: &crate::ui::theme::Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle {
                background: Some(Value::Static(theme.colors.surface)),
                border_color: Some(Value::Static(theme.colors.outline_muted)),
                border_width: Some(Value::Static(theme.border.thin)),
                border_radius: Some(Value::Static(theme.radius.md)),
                ..WidgetSurfaceStyle::default()
            },
            header_height: theme.spacing.xl + theme.spacing.sm,
            compact_row_height: theme.spacing.xl,
            regular_row_height: theme.spacing.xl + theme.spacing.sm,
            spacious_row_height: theme.spacing.xxl,
            cell_padding: Insets::symmetric(theme.spacing.md, theme.spacing.xs + theme.spacing.xxs),
            header_background: Value::Static(theme.colors.surface_low),
            header_text: Value::Static(palette.on_surface),
            row_background: Value::Static(Color::TRANSPARENT),
            zebra_background: Value::Static(theme.colors.surface_low.with_alpha_factor(0.48)),
            row_hover_background: Value::Static(
                theme.colors.primary_container.with_alpha_factor(0.34),
            ),
            row_selected_background: Value::Static(
                theme.colors.primary_container.with_alpha_factor(0.62),
            ),
            cell_focused_border: Value::Static(theme.colors.focus_ring),
            cell_editing_background: Value::Static(theme.colors.surface),
            grid_line: Value::Static(theme.colors.outline_muted),
            resize_handle: Value::Static(theme.colors.outline),
            sort_indicator: Value::Static(theme.colors.primary),
            scrollbar: ContainerStyle::default_for_theme(theme).scrollbar,
        }
    }

    pub fn row_height(&self, density: DataGridDensity) -> Dp {
        match density {
            DataGridDensity::Compact => self.compact_row_height,
            DataGridDensity::Regular => self.regular_row_height,
            DataGridDensity::Spacious => self.spacious_row_height,
        }
    }
}

enum DataGridVirtualRow<T, VM> {
    Section(Element<VM>),
    Row {
        source_index: usize,
        key: WidgetKey,
        value: T,
        disabled: Value<bool>,
    },
}

struct DataGridRowSource<T, VM> {
    rows: Arc<[DataGridVirtualRow<T, VM>]>,
}

impl<T: Clone, VM> Clone for DataGridVirtualRow<T, VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Section(header) => Self::Section(header.clone()),
            Self::Row {
                source_index,
                key,
                value,
                disabled,
            } => Self::Row {
                source_index: *source_index,
                key: key.clone(),
                value: value.clone(),
                disabled: disabled.clone(),
            },
        }
    }
}

impl<T, VM> Clone for DataGridRowSource<T, VM> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
        }
    }
}

impl<T, VM> ItemSource<DataGridVirtualRow<T, VM>> for DataGridRowSource<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn item(&self, index: usize) -> Option<DataGridVirtualRow<T, VM>> {
        self.rows.get(index).cloned()
    }

    fn key(&self, index: usize) -> Option<WidgetKey> {
        match self.rows.get(index)? {
            DataGridVirtualRow::Section(_) => Some(WidgetKey::from(format!("section-{index}"))),
            DataGridVirtualRow::Row { key, .. } => Some(key.clone()),
        }
    }
}

pub struct DataGrid<T, VM> {
    rows: Vec<DataGridVirtualRow<T, VM>>,
    columns: Vec<DataGridColumn<T, VM>>,
    selected_keys: Value<Vec<WidgetKey>>,
    selection_mode: DataGridSelectionMode,
    sort: Value<Vec<DataGridSort>>,
    density: DataGridDensity,
    row_height: Option<Dp>,
    loading: Value<bool>,
    empty_view: Option<Element<VM>>,
    loading_view: Option<Element<VM>>,
    item_layout: Option<ItemLayout>,
    style: Option<StyleResolver<DataGridStyle>>,
    on_selection_change: Option<ValueCommand<VM, DataGridSelectionChange>>,
    on_sort_change: Option<ValueCommand<VM, DataGridSortChange>>,
    on_column_width_change: Option<ValueCommand<VM, DataGridColumnWidthChange>>,
    on_column_reorder: Option<ValueCommand<VM, DataGridColumnReorderEvent>>,
    on_cell_action: Option<ValueCommand<VM, DataGridCellAction>>,
    on_cell_edit_commit: Option<ValueCommand<VM, DataGridCellEditCommit>>,
    context_menu: Vec<MenuItemState<VM>>,
    layout: LayoutStyle,
    focusable: Option<bool>,
    tab_index: Option<i32>,
    focus_scope: Option<FocusScopeOptions>,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    lifecycle_events: LifecycleEventHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    key: Option<WidgetKey>,
}

impl<T, VM: 'static> DataGrid<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    pub fn new<I>(rows: Vec<I>, columns: Vec<DataGridColumn<T, VM>>) -> Self
    where
        I: Into<DataGridRow<T>>,
    {
        let rows = rows
            .into_iter()
            .map(Into::into)
            .enumerate()
            .map(|(index, row)| DataGridVirtualRow::Row {
                source_index: index,
                key: row.key.unwrap_or_else(|| WidgetKey::from(index)),
                value: row.value,
                disabled: row.disabled,
            })
            .collect();
        Self::from_rows(rows, columns)
    }

    pub fn sections(
        sections: Vec<DataGridSection<T, VM>>,
        columns: Vec<DataGridColumn<T, VM>>,
    ) -> Self {
        let mut rows = Vec::new();
        let mut row_index = 0usize;
        for section in sections {
            rows.push(DataGridVirtualRow::Section(section.header));
            for row in section.rows {
                rows.push(DataGridVirtualRow::Row {
                    source_index: row_index,
                    key: row.key.unwrap_or_else(|| WidgetKey::from(row_index)),
                    value: row.value,
                    disabled: row.disabled,
                });
                row_index += 1;
            }
        }
        Self::from_rows(rows, columns)
    }

    fn from_rows(
        rows: Vec<DataGridVirtualRow<T, VM>>,
        columns: Vec<DataGridColumn<T, VM>>,
    ) -> Self {
        let interactions = InteractionHandlers {
            cursor_style: Some(Value::Static(CursorStyle::Pointer)),
            ..Default::default()
        };
        Self {
            rows,
            columns,
            selected_keys: Value::Static(Vec::new()),
            selection_mode: DataGridSelectionMode::Single,
            sort: Value::Static(Vec::new()),
            density: DataGridDensity::Regular,
            row_height: None,
            loading: Value::Static(false),
            empty_view: None,
            loading_view: None,
            item_layout: None,
            style: None,
            on_selection_change: None,
            on_sort_change: None,
            on_column_width_change: None,
            on_column_reorder: None,
            on_cell_action: None,
            on_cell_edit_commit: None,
            context_menu: Vec::new(),
            layout: LayoutStyle::default(),
            focusable: None,
            tab_index: None,
            focus_scope: None,
            visual: VisualStyle::default(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            key: None,
        }
    }

    pub fn column(mut self, column: DataGridColumn<T, VM>) -> Self {
        self.columns.push(column);
        self
    }

    pub fn selection_mode(mut self, mode: DataGridSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn selected_keys(mut self, keys: impl Into<Value<Vec<WidgetKey>>>) -> Self {
        self.selected_keys = keys.into();
        self
    }

    pub fn sort(mut self, sort: impl Into<Value<Vec<DataGridSort>>>) -> Self {
        self.sort = sort.into();
        self
    }

    pub fn density(mut self, density: DataGridDensity) -> Self {
        self.density = density;
        self
    }

    pub fn row_height(mut self, row_height: Dp) -> Self {
        self.row_height = Some(row_height.max(Dp::ZERO));
        self
    }

    pub fn item_layout(mut self, layout: ItemLayout) -> Self {
        self.item_layout = Some(layout);
        self
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        let row_height = self.row_height.unwrap_or(dp(40.0));
        self.item_layout = Some(ItemLayout::Fixed {
            item_extent: row_height,
            spacing: Dp::ZERO,
            overscan,
        });
        self
    }

    pub fn loading(mut self, loading: impl Into<Value<bool>>) -> Self {
        self.loading = loading.into();
        self
    }

    pub fn empty(mut self, view: impl Into<Element<VM>>) -> Self {
        self.empty_view = Some(view.into());
        self
    }

    pub fn loading_view(mut self, view: impl Into<Element<VM>>) -> Self {
        self.loading_view = Some(view.into());
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut DataGridStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| DataGridStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> DataGridStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn on_selection_change(
        mut self,
        command: ValueCommand<VM, DataGridSelectionChange>,
    ) -> Self {
        self.on_selection_change = Some(command);
        self
    }

    pub fn on_sort_change(mut self, command: ValueCommand<VM, DataGridSortChange>) -> Self {
        self.on_sort_change = Some(command);
        self
    }

    pub fn on_column_width_change(
        mut self,
        command: ValueCommand<VM, DataGridColumnWidthChange>,
    ) -> Self {
        self.on_column_width_change = Some(command);
        self
    }

    pub fn on_column_reorder(
        mut self,
        command: ValueCommand<VM, DataGridColumnReorderEvent>,
    ) -> Self {
        self.on_column_reorder = Some(command);
        self
    }

    pub fn on_cell_action(mut self, command: ValueCommand<VM, DataGridCellAction>) -> Self {
        self.on_cell_action = Some(command);
        self
    }

    pub fn on_cell_edit_commit(
        mut self,
        command: ValueCommand<VM, DataGridCellEditCommit>,
    ) -> Self {
        self.on_cell_edit_commit = Some(command);
        self
    }

    pub fn context_menu(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.context_menu = items.into_iter().map(MenuItemState::from_public).collect();
        self
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        set_layout_lengths(&mut self.layout, width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.height, height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_width, width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_height, height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_width, width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_height, height);
        self
    }

    pub fn margin(mut self, margin: impl Into<Value<Insets>>) -> Self {
        self.layout.margin = margin.into();
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.layout.shrink = shrink.into();
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.layout.align_self = Some(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.layout.position_type = crate::ui::layout::PositionType::Absolute;
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.left, value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.top, value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.right, value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.bottom, value);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = Some(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.tab_index = Some(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.focus_scope = Some(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.focus_scope = Some(
            self.focus_scope
                .take()
                .unwrap_or_default()
                .auto_focus_first(auto_focus_first),
        );
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_update = Some(command);
        self
    }
}

impl<T, VM> From<DataGrid<T, VM>> for Element<VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn from(value: DataGrid<T, VM>) -> Self {
        value.into_element()
    }
}

impl<T, VM> DataGrid<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn into_element(self) -> Element<VM> {
        if self.loading.resolve() {
            return self
                .loading_view
                .unwrap_or_else(|| Stack::new().child(Text::new("Loading...")).into());
        }
        if self
            .rows
            .iter()
            .all(|row| matches!(row, DataGridVirtualRow::Section(_)))
        {
            return self
                .empty_view
                .unwrap_or_else(|| Stack::new().child(Text::new("No rows")).into());
        }

        let grid_id = WidgetId::next();
        let style_resolver = self.style.clone();
        let style = resolve_data_grid_style_for_layout(style_resolver.as_ref());
        let row_height = self
            .row_height
            .unwrap_or_else(|| style.row_height(self.density));
        let item_layout = self.item_layout.unwrap_or(ItemLayout::Fixed {
            item_extent: row_height,
            spacing: Dp::ZERO,
            overscan: 2,
        });
        let selected_keys = self.selected_keys.clone();
        let sort = self.sort.clone();
        let columns = Arc::new(ordered_columns(self.columns));
        let total_width = columns
            .iter()
            .map(|column| resolved_column_width(column))
            .fold(Dp::ZERO, |acc, width| acc + width);
        let body_id = WidgetId::next();
        let row_keys: Arc<[WidgetKey]> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                DataGridVirtualRow::Row { key, .. } => Some(key.clone()),
                DataGridVirtualRow::Section(_) => None,
            })
            .collect::<Vec<_>>()
            .into();
        let row_count = row_keys.len();
        let row_disabled: Arc<[bool]> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                DataGridVirtualRow::Row { disabled, .. } => Some(disabled.resolve()),
                DataGridVirtualRow::Section(_) => None,
            })
            .collect::<Vec<_>>()
            .into();
        let source = DataGridRowSource {
            rows: self.rows.into(),
        };
        let style = Arc::new(style);
        let header = build_header(
            grid_id,
            body_id,
            columns.clone(),
            sort.clone(),
            style.clone(),
            style_resolver.clone(),
            self.on_sort_change.clone(),
            self.on_column_width_change.clone(),
            self.on_column_reorder.clone(),
            total_width,
        );
        let selection_mode = self.selection_mode;
        let on_selection_change = self.on_selection_change.clone();
        let on_cell_action = self.on_cell_action.clone();
        let on_cell_edit_commit = self.on_cell_edit_commit.clone();
        let context_menu = Arc::new(self.context_menu);
        let row_style = style.clone();
        let row_columns = columns.clone();
        let row_selected_keys = selected_keys.clone();
        let row_style_resolver = style_resolver.clone();
        let body_style_resolver = style_resolver.clone();
        let mut virtual_rows: Element<VM> =
            VirtualList::new(source, move |_visible, row| match row {
                DataGridVirtualRow::Section(header) => {
                    let mut section = header.clone();
                    section.layout.width =
                        Some(Value::Static(crate::ui::layout::Length::Px(total_width)));
                    section.layout.height =
                        Some(Value::Static(crate::ui::layout::Length::Px(row_height)));
                    section
                }
                DataGridVirtualRow::Row {
                    source_index,
                    key,
                    value,
                    disabled,
                } => build_data_row(
                    grid_id,
                    body_id,
                    _visible,
                    *source_index,
                    key.clone(),
                    value.clone(),
                    disabled.clone(),
                    row_columns.clone(),
                    row_selected_keys.clone(),
                    selection_mode,
                    row_keys.clone(),
                    row_disabled.clone(),
                    row_style.clone(),
                    row_style_resolver.clone(),
                    row_height,
                    on_selection_change.clone(),
                    on_cell_action.clone(),
                    on_cell_edit_commit.clone(),
                    context_menu.clone(),
                    total_width,
                ),
            })
            .item_layout(item_layout)
            .content_cross_extent(total_width)
            .width(pct(100.0))
            .overflow_x(Overflow::Scroll)
            .overflow_y(Overflow::Scroll)
            .grow(1.0)
            .style_full_with_style_sheet({
                let style_resolver = body_style_resolver;
                move |context, style_sheet, visual, state| {
                    let style = resolve_data_grid_style_with_sheet(
                        style_resolver.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        state,
                    );
                    let mut container = ContainerStyle::default_for_theme(context.theme);
                    container.scrollbar = style.scrollbar;
                    container.surface.background = style.surface.background.clone();
                    container
                }
            })
            .into();
        virtual_rows.id = body_id;

        let mut root: Element<VM> = Flex::vertical()
            .child(header)
            .child(virtual_rows)
            .align(Align::Stretch)
            .into();
        root.id = grid_id;
        root.key = self.key;
        apply_outer_layout(&mut root.layout, self.layout);
        root.focus.focusable = self.focusable;
        root.focus.tab_index = self.tab_index;
        root.focus.scope = self.focus_scope;
        root.visual = self.visual;
        root.interactions = self.interactions;
        root.lifecycle_events = self.lifecycle_events;
        root.media_events = self.media_events;
        root.data_grid_root = Some(DataGridRootState {
            grid_id,
            row_count,
            column_count: columns.len(),
            selection_mode,
            selected_keys,
        });
        apply_data_grid_root_style(&mut root, style_resolver);
        root
    }
}

fn build_header<T, VM: 'static>(
    grid_id: WidgetId,
    scroll_container_id: WidgetId,
    columns: Arc<Vec<DataGridColumn<T, VM>>>,
    sort: Value<Vec<DataGridSort>>,
    style: Arc<DataGridStyle>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
    on_sort_change: Option<ValueCommand<VM, DataGridSortChange>>,
    on_column_width_change: Option<ValueCommand<VM, DataGridColumnWidthChange>>,
    on_column_reorder: Option<ValueCommand<VM, DataGridColumnReorderEvent>>,
    total_width: Dp,
) -> Element<VM>
where
    T: Clone + Send + Sync + 'static,
{
    let mut header = Flex::horizontal()
        .height(style.header_height)
        .width(total_width);
    let pin_offsets = column_pin_offsets(columns.as_ref());
    for (column_index, column) in columns.iter().enumerate() {
        let width = resolved_column_width(column);
        let sort_state = sort.resolve();
        let sort_position = sort_state
            .iter()
            .position(|entry| entry.column_key == column.key);
        let sort_direction = sort_position.map(|index| sort_state[index].direction);
        let label = column.label.resolve();
        let context = DataGridHeaderContext {
            column_index,
            column_key: column.key.clone(),
            label: label.clone(),
            sort_direction,
            sort_priority: sort_position.map(|index| index + 1),
        };
        let mut child = column
            .header
            .as_ref()
            .map(|render| render(context))
            .unwrap_or_else(|| {
                let suffix = match sort_direction {
                    Some(DataGridSortDirection::Ascending) => " ↑",
                    Some(DataGridSortDirection::Descending) => " ↓",
                    None => "",
                };
                Text::new(format!("{label}{suffix}")).into()
            });
        apply_default_data_grid_text_style(
            &mut child,
            style_resolver.clone(),
            DataGridTextRole::Header,
        );
        let mut cell: Element<VM> = Stack::new()
            .child(child)
            .width(width)
            .height(style.header_height)
            .padding(style.cell_padding)
            .into();
        let header_focusable = Some(column.sortable || column.reorderable);
        let header_tab_index = Some(if column_index == 0 { 0 } else { -1 });
        let header_cursor = Some(Value::Static(if column.sortable {
            CursorStyle::Pointer
        } else {
            CursorStyle::Default
        }));
        let header_state = DataGridHeaderState {
            grid_id,
            scroll_container_id,
            column_index,
            column_key: column.key.clone(),
            label,
            pin: column.pin,
            pin_offset: pin_offsets[column_index],
            sortable: column.sortable,
            resizable: column.resizable,
            reorderable: column.reorderable,
            sort: sort.clone(),
            width,
            min_width: column.min_width,
            max_width: column.max_width,
            on_sort_change: on_sort_change.clone(),
            on_column_width_change: on_column_width_change.clone(),
            on_column_reorder: on_column_reorder.clone(),
        };
        if column.resizable {
            let mut handle: Element<VM> = Stack::new()
                .width(dp(6.0))
                .height(style.header_height)
                .position_absolute()
                .right(dp(0.0))
                .cursor(CursorStyle::EwResize)
                .into();
            handle.background = Some(Value::Static(Color::TRANSPARENT));
            handle.data_grid_resize_handle = Some(DataGridResizeHandleState {
                grid_id,
                column_index,
                column_key: column.key.clone(),
                width,
                min_width: column.min_width,
                max_width: column.max_width,
                on_column_width_change: on_column_width_change.clone(),
            });
            cell = Stack::new()
                .child(cell)
                .child(handle)
                .width(width)
                .height(style.header_height)
                .into();
            cell.visual.border_color = Some(style.grid_line.clone());
            cell.visual.border_width = Some(Value::Static(dp(0.5)));
            apply_data_grid_cell_container_style(&mut cell, style_resolver.clone());
            cell.focus.focusable = header_focusable;
            cell.focus.tab_index = header_tab_index;
            cell.interactions.cursor_style = header_cursor;
            cell.data_grid_header = Some(header_state);
        } else {
            cell.visual.border_color = Some(style.grid_line.clone());
            cell.visual.border_width = Some(Value::Static(dp(0.5)));
            apply_data_grid_cell_container_style(&mut cell, style_resolver.clone());
            cell.focus.focusable = header_focusable;
            cell.focus.tab_index = header_tab_index;
            cell.interactions.cursor_style = header_cursor;
            cell.data_grid_header = Some(header_state);
        }
        header = header.child(cell);
    }
    let mut header: Element<VM> = header.into();
    apply_data_grid_header_container_style(&mut header, style_resolver);
    header
}

#[allow(clippy::too_many_arguments)]
fn build_data_row<T, VM: 'static>(
    grid_id: WidgetId,
    scroll_container_id: WidgetId,
    virtual_row_index: usize,
    row_index: usize,
    row_key: WidgetKey,
    row: T,
    disabled: Value<bool>,
    columns: Arc<Vec<DataGridColumn<T, VM>>>,
    selected_keys: Value<Vec<WidgetKey>>,
    selection_mode: DataGridSelectionMode,
    sibling_keys: Arc<[WidgetKey]>,
    sibling_disabled: Arc<[bool]>,
    style: Arc<DataGridStyle>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
    row_height: Dp,
    on_selection_change: Option<ValueCommand<VM, DataGridSelectionChange>>,
    on_cell_action: Option<ValueCommand<VM, DataGridCellAction>>,
    on_cell_edit_commit: Option<ValueCommand<VM, DataGridCellEditCommit>>,
    context_menu: Arc<Vec<MenuItemState<VM>>>,
    total_width: Dp,
) -> Element<VM>
where
    T: Clone + Send + Sync + 'static,
{
    let selected = selected_keys.resolve().contains(&row_key);
    let disabled_now = disabled.resolve();
    let mut row_element = Flex::horizontal().width(total_width).height(row_height);
    let pin_offsets = column_pin_offsets(columns.as_ref());
    for (column_index, column) in columns.iter().enumerate() {
        let column_width = resolved_column_width(column);
        let edit_value = column
            .text_value
            .as_ref()
            .map(|value| value(&row))
            .unwrap_or_default();
        let context = DataGridCellContext {
            row_index,
            row_key: row_key.clone(),
            column_index,
            column_key: column.key.clone(),
            row: row.clone(),
            selected,
            disabled: disabled_now,
            editing: false,
        };
        let mut child = (column.render)(context);
        apply_default_data_grid_text_style(
            &mut child,
            style_resolver.clone(),
            DataGridTextRole::Cell,
        );
        let mut cell: Element<VM> = Stack::new()
            .child(child)
            .width(column_width)
            .height(row_height)
            .padding(style.cell_padding)
            .align_self(column.align)
            .into();
        cell.visual.border_color = Some(style.grid_line.clone());
        cell.visual.border_width = Some(Value::Static(dp(0.5)));
        apply_data_grid_cell_container_style(&mut cell, style_resolver.clone());
        cell.key = Some(WidgetKey::from(format!("{row_key:?}:{:?}", column.key)));
        cell.focus.focusable = Some(!disabled_now);
        cell.focus.tab_index = Some(if row_index == 0 && column_index == 0 {
            0
        } else {
            -1
        });
        cell.interactions.cursor_style = Some(Value::Static(if disabled_now {
            CursorStyle::Default
        } else {
            CursorStyle::Pointer
        }));
        cell.data_grid_cell = Some(DataGridCellState {
            grid_id,
            scroll_container_id,
            virtual_row_index,
            row_index,
            column_index,
            row_key: row_key.clone(),
            column_key: column.key.clone(),
            pin: column.pin,
            pin_offset: pin_offsets[column_index],
            selected_keys: selected_keys.clone(),
            selection_mode,
            disabled: disabled.clone(),
            editable: column.editable && column.text_value.is_some(),
            edit_value,
            on_selection_change: on_selection_change.clone(),
            on_cell_action: on_cell_action.clone(),
            on_cell_edit_commit: on_cell_edit_commit.clone(),
            sibling_keys: sibling_keys.clone(),
            sibling_disabled: sibling_disabled.clone(),
        });
        if !context_menu.is_empty() {
            let on_show = ValueCommand::new(|_: &mut VM, _: LongPressEvent| {});
            cell.interactions.gesture = Some(match cell.interactions.gesture.take() {
                Some(existing) => existing.on_long_press(on_show),
                None => GestureRecognizer::new().on_long_press(on_show),
            });
            cell.context_menu = Some(Box::new(ContextMenuDescriptor {
                items: context_menu.as_ref().to_vec(),
                on_open_change: None,
                disabled: Value::Static(false),
                style: None,
            }));
        }
        row_element = row_element.child(cell);
    }
    let mut row_element: Element<VM> = row_element.into();
    apply_data_grid_row_container_style(&mut row_element, style_resolver, selected, row_index);
    row_element
}

#[derive(Clone, Copy)]
enum DataGridTextRole {
    Header,
    Cell,
}

fn resolve_data_grid_style(
    style: Option<&StyleResolver<DataGridStyle>>,
    context: &StyleContext<'_>,
) -> DataGridStyle {
    let style_sheet = StyleSheet::default();
    let visual = VisualStyle::default();
    resolve_data_grid_style_with_sheet(
        style,
        context,
        &style_sheet,
        &visual,
        WidgetState::default(),
    )
}

fn resolve_data_grid_style_with_sheet(
    style: Option<&StyleResolver<DataGridStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> DataGridStyle {
    let mut base = DataGridStyle::default_for_theme(context.theme);
    context.theme.components.data_grid.apply(&mut base, context);
    style_sheet.apply_data_grid(&mut base, context, visual);
    style_sheet.apply_data_grid_state(&mut base, context, visual, state);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_data_grid_style_for_layout(
    style: Option<&StyleResolver<DataGridStyle>>,
) -> DataGridStyle {
    let theme = crate::ui::theme::Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_data_grid_style(style, &context)
}

fn apply_container_style<VM>(
    element: &mut Element<VM>,
    resolver: impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
        + Send
        + Sync
        + 'static,
) {
    if let WidgetKind::Container { style, .. } = &mut element.kind {
        *style = Some(StyleResolver::full_with_style_sheet(resolver));
    }
}

fn apply_data_grid_root_style<VM>(
    element: &mut Element<VM>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
) {
    apply_container_style(element, move |context, style_sheet, visual, state| {
        let style = resolve_data_grid_style_with_sheet(
            style_resolver.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.scrollbar = style.scrollbar;
        container.surface.background = style.surface.background;
        container.surface.background_brush = style.surface.background_brush;
        container.surface.background_image = style.surface.background_image;
        container.surface.background_blur = style.surface.background_blur;
        container.surface.shadow = style.surface.shadow;
        container.surface.opacity = style.surface.opacity;
        container.surface.offset = style.surface.offset;
        container
    });
}

fn apply_data_grid_header_container_style<VM>(
    element: &mut Element<VM>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
) {
    apply_container_style(element, move |context, style_sheet, visual, state| {
        let style = resolve_data_grid_style_with_sheet(
            style_resolver.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.background = Some(style.header_background);
        container
    });
}

fn apply_data_grid_row_container_style<VM>(
    element: &mut Element<VM>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
    selected: bool,
    row_index: usize,
) {
    apply_container_style(element, move |context, style_sheet, visual, state| {
        let style = resolve_data_grid_style_with_sheet(
            style_resolver.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        let background = if selected {
            style.row_selected_background
        } else if row_index % 2 == 1 {
            style.zebra_background
        } else {
            style.row_background
        };
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.background = Some(background);
        container
    });
}

fn apply_data_grid_cell_container_style<VM>(
    element: &mut Element<VM>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
) {
    apply_container_style(element, move |context, style_sheet, visual, state| {
        let style = resolve_data_grid_style_with_sheet(
            style_resolver.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.border_color = Some(style.grid_line);
        container.surface.border_width = Some(Value::Static(dp(0.5)));
        container
    });
}

fn apply_default_data_grid_text_style<VM>(
    element: &mut Element<VM>,
    style_resolver: Option<StyleResolver<DataGridStyle>>,
    role: DataGridTextRole,
) {
    match &mut element.kind {
        WidgetKind::Text { text } if text.style.is_none() => {
            text.style = Some(StyleResolver::full(move |context| {
                let style = resolve_data_grid_style(style_resolver.as_ref(), context);
                let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
                text_style.color = match role {
                    DataGridTextRole::Header | DataGridTextRole::Cell => style.header_text,
                };
                text_style
            }));
        }
        WidgetKind::Container { children, .. } => {
            for child_source in children {
                match child_source {
                    ChildSource::Static(children) => {
                        for child in children {
                            apply_default_data_grid_text_style(child, style_resolver.clone(), role);
                        }
                    }
                    ChildSource::Show { child, .. } => {
                        apply_default_data_grid_text_style(child, style_resolver.clone(), role);
                    }
                    ChildSource::Switch {
                        cases, fallback, ..
                    } => {
                        for child in cases {
                            apply_default_data_grid_text_style(child, style_resolver.clone(), role);
                        }
                        if let Some(child) = fallback {
                            apply_default_data_grid_text_style(child, style_resolver.clone(), role);
                        }
                    }
                    ChildSource::KeyedFor(_) => {}
                    ChildSource::Dynamic(_) => {}
                }
            }
        }
        _ => {}
    }
}

fn ordered_columns<T, VM>(columns: Vec<DataGridColumn<T, VM>>) -> Vec<DataGridColumn<T, VM>> {
    let mut start = Vec::new();
    let mut middle = Vec::new();
    let mut end = Vec::new();
    for column in columns {
        match column.pin {
            DataGridColumnPin::Start => start.push(column),
            DataGridColumnPin::None => middle.push(column),
            DataGridColumnPin::End => end.push(column),
        }
    }
    start.extend(middle);
    start.extend(end);
    start
}

fn apply_outer_layout(target: &mut LayoutStyle, source: LayoutStyle) {
    target.width = source.width;
    target.height = source.height;
    target.min_width = source.min_width;
    target.min_height = source.min_height;
    target.max_width = source.max_width;
    target.max_height = source.max_height;
    target.aspect_ratio = source.aspect_ratio;
    target.padding = source.padding;
    target.margin = source.margin;
    target.grow = source.grow;
    target.shrink = source.shrink;
    target.basis = source.basis;
    target.position_type = source.position_type;
    target.left = source.left;
    target.top = source.top;
    target.right = source.right;
    target.bottom = source.bottom;
    target.align_self = source.align_self;
    target.justify_self = source.justify_self;
    target.column_start = source.column_start;
    target.row_start = source.row_start;
    target.column_span = source.column_span;
    target.row_span = source.row_span;
}

fn column_pin_offsets<T, VM>(columns: &[DataGridColumn<T, VM>]) -> Vec<Dp> {
    let mut offsets = vec![Dp::ZERO; columns.len()];
    let mut start_offset = Dp::ZERO;
    for (index, column) in columns.iter().enumerate() {
        if column.pin == DataGridColumnPin::Start {
            offsets[index] = start_offset;
            start_offset += resolved_column_width(column);
        }
    }
    let mut end_offset = Dp::ZERO;
    for (index, column) in columns.iter().enumerate().rev() {
        if column.pin == DataGridColumnPin::End {
            offsets[index] = end_offset;
            end_offset += resolved_column_width(column);
        }
    }
    offsets
}

fn resolved_column_width<T, VM>(column: &DataGridColumn<T, VM>) -> Dp {
    let mut width = column.width.resolve().max(column.min_width);
    if let Some(max_width) = column.max_width {
        width = width.min(max_width);
    }
    width
}
