use std::cmp::Ordering;

use crate::app::{App, DemoContact, Employee};
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_TABS_BASIC: &str = r#"Tabs::new(items, app.tabs_selected.signal())
    .overflow_mode(TabsOverflowMode::More)
    .reorderable(true)
    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
        app.tabs_selected.set(key);
    }))"#;

const CODE_TABVIEW_PLACEMENT: &str = r#"TabView::new(items, app.tabs_selected.signal())
    .placement(TabPlacement::Left)
    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
        app.tabs_selected.set(key);
    }))"#;

const CODE_LIST_SELECTION: &str = r#"List::sections(contact_sections(), contact_row)
    .selection_mode(ListSelectionMode::Multiple)
    .selected_keys(app.list_selected_keys.signal())
    .on_selection_change(ValueCommand::new(App::set_list_selection))"#;

const CODE_LIST_STATES: &str = r#"List::sections(rows, contact_row)
    .loading(app.list_loading.signal())
    .loading_view(state_view("Loading contact rows..."))
    .empty(state_view("No contacts"))"#;

const CODE_VIRTUAL_FIXED: &str = r#"VirtualList::new_with_context(app.virtual_rows.clone(), virtual_row)
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(32.0),
        spacing: dp(2.0),
        overscan: 4,
    })"#;

const CODE_VIRTUAL_MEASURED: &str = r#"List::sections(contact_sections(), contact_row)
    .item_layout(ItemLayout::Measured {
        estimate: dp(64.0),
        spacing: dp(4.0),
        overscan: 3,
    })"#;

const CODE_TREE_BASIC: &str = r#"Tree::new(demo_tree_nodes(), tree_row)
    .expanded_keys(app.tree_expanded_keys.signal())
    .selected_keys(app.tree_selected_keys.signal())
    .selection_mode(TreeSelectionMode::Multiple)
    .checkable(true)
    .checked_keys(app.tree_checked_keys.signal())
    .draggable(true)
    .on_expand_change(ValueCommand::new(App::set_tree_expanded))
    .on_selection_change(ValueCommand::new(App::set_tree_selection))
    .on_check_change(ValueCommand::new(App::set_tree_checked))
    .on_drop(ValueCommand::new(App::drop_tree_node))"#;

const CODE_DATAGRID_BASIC: &str = r#"DataGrid::new(sorted_rows(app), columns(app))
    .selection_mode(DataGridSelectionMode::Multiple)
    .selected_keys(app.data_selected.signal())
    .sort(app.data_sort.signal())
    .on_selection_change(ValueCommand::new(App::select_data_rows))
    .on_sort_change(ValueCommand::new(App::sort_data_rows))"#;

const CODE_DATAGRID_COLUMNS: &str = r#"DataGridColumn::new("name", "Name".to_string(), cell)
    .width(app.data_name_width.signal())
    .min_width(dp(140.0))
    .max_width(dp(260.0))
    .sortable(true)
    .editable(true)"#;

const CODE_TABLE_ALIAS: &str = r#"Table::new(sorted_rows(app), columns(app))
    .density(DataGridDensity::Compact)
    .row_height(dp(34.0))"#;

const CODE_DATA_NAVIGATION: &str = r#"Breadcrumb::new(vec![
    BreadcrumbItem::new("Workspace"),
    BreadcrumbItem::new("Components"),
    BreadcrumbItem::new("Data"),
])

Pagination::new(app.pagination_page.signal(), 12usize)
    .page_size(app.pagination_page_size.signal())
    .on_change(...)"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Data",
        "数据页面展示导航、tabs、列表、虚拟滚动、树和表格型数据控件。",
        vec![
            navigation_component(app),
            tabs_component(app),
            list_component(app),
            virtual_list_component(app),
            tree_component(app),
            data_grid_component(app),
        ],
    )
}

fn navigation_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Breadcrumb / Pagination",
        "Breadcrumb 和 Pagination 覆盖常见路径导航与分页控制。",
        vec![UsageDemo::new(
            "data/navigation",
            "路径与分页",
            "分页事件由 ViewModel 回写当前页和 page size。",
            Flex::vertical().gap(dp(12.0)).child(el![
                Breadcrumb::new(vec![
                    BreadcrumbItem::new("Workspace").on_click(Command::new(|app: &mut App| {
                        app.component_status.set("点击了 Workspace".to_string());
                    })),
                    BreadcrumbItem::new("Components"),
                    BreadcrumbItem::new("Data"),
                ]),
                Pagination::new(app.pagination_page.signal(), 12usize)
                    .page_size(app.pagination_page_size.signal())
                    .on_change(ValueCommand::new(|app: &mut App, change: PaginationChange| {
                        app.pagination_page.set(change.page);
                        app.pagination_page_size.set(change.page_size);
                        app.component_status.set(format!(
                            "分页: page={}, page_size={}",
                            change.page, change.page_size
                        ));
                    })),
                Text::new(app.component_status.signal()).style_full(styles::status_style),
            ]),
            CODE_DATA_NAVIGATION,
        )],
    )
}

fn tabs_component(app: &App) -> Element<App> {
    let slider_value = app.slider_value.signal();
    let switch_value = app.switch.signal();
    let checkbox_value = app.checkbox.signal();
    let selected = app.tabs_selected.signal();
    let selected_for_items = selected.clone();
    let selected_for_tabs = selected.clone();
    let selected_for_left = selected.clone();

    demo_section::component_doc(
        app,
        "Tabs / TabView",
        "Tabs 根据选中 key 渲染当前 panel；TabView 是同一组件的别名。",
        vec![
            UsageDemo::new(
                "tabs/basic",
                "溢出和重排",
                "超过可见预算的 tab 会进入 More 菜单，并支持拖拽重排。",
                Flex::vertical()
                    .gap(dp(8.0))
                    .child(app.tabs_order.signal().map(move |order| {
                        let tabs: Element<App> = Tabs::new(
                            demo_tab_items(
                                order,
                                slider_value.clone(),
                                switch_value.clone(),
                                checkbox_value.clone(),
                                selected_for_items.clone(),
                            ),
                            selected_for_tabs.clone(),
                        )
                        .overflow_mode(TabsOverflowMode::More)
                        .reorderable(true)
                        .width(dp(430.0))
                        .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                            app.tabs_selected.set(key);
                        }))
                        .on_reorder(ValueCommand::new(
                            |app: &mut App, event: TabsReorderEvent| {
                                let mut order = app.tabs_order.get();
                                if event.from_index < order.len() && event.to_index < order.len() {
                                    let item = order.remove(event.from_index);
                                    order.insert(event.to_index, item);
                                    app.tabs_order.set(order);
                                    app.tabs_selected.set(event.key.clone());
                                    app.tabs_reorder_status
                                        .set(format!("重排 {} -> {}", event.key, event.target_key));
                                }
                            },
                        ))
                        .into();
                        tabs
                    }))
                    .child(Text::new(app.tabs_reorder_status.signal()).style_full(styles::status_style)),
                CODE_TABS_BASIC,
            ),
            UsageDemo::new(
                "tabs/placement",
                "左侧 TabView",
                "TabView 别名可使用同样 API 设置侧向 placement。",
                TabView::new(
                    vec![
                        TabItem::new(
                            "overview",
                            "概览",
                            Text::new("概览内容").style_full(styles::status_style),
                        ),
                        TabItem::new(
                            "logs",
                            "日志",
                            Text::new("日志内容").style_full(styles::status_style),
                        ),
                        TabItem::new(
                            "settings",
                            "设置",
                            Text::new("设置内容").style_full(styles::status_style),
                        ),
                    ],
                    selected_for_left,
                )
                .placement(TabPlacement::Left)
                .width(dp(360.0))
                .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                    app.tabs_selected.set(key);
                })),
                CODE_TABVIEW_PLACEMENT,
            ),
        ],
    )
}

fn demo_tab_items(
    order: Vec<String>,
    slider_value: Signal<f32>,
    switch_value: Signal<bool>,
    checkbox_value: Signal<bool>,
    selected: Signal<String>,
) -> Vec<TabItem<App>> {
    order
        .into_iter()
        .map(|key| match key.as_str() {
            "overview" => TabItem::new(
                "overview",
                "概览",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new("Tabs 会根据选中 key 只渲染当前 panel。").style_full(styles::status_style),
                    ProgressBar::new(slider_value.clone().map(|value| value / 100.0))
                        .width(dp(240.0))
                        .show_label(true)
                        .label(
                            slider_value
                                .clone()
                                .map(|value| format!("当前音量 {:.0}%", value))
                        ),
                ]),
            ),
            "settings" => TabItem::new(
                "settings",
                "设置",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Switch::new(switch_value.clone()).on_change(ValueCommand::new(
                        |app: &mut App, enabled| app.switch.set(enabled),
                    )),
                    Checkbox::new(checkbox_value.clone())
                        .label("同步到偏好设置")
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.checkbox.set(checked)
                        })),
                ]),
            ),
            "logs" => TabItem::new(
                "logs",
                "日志",
                Text::new(selected.clone().map(|key| format!("active tab: {key}")))
                    .style_full(styles::status_style),
            ),
            "metrics" => TabItem::new(
                "metrics",
                "指标",
                Text::new("More 模式下会被收进更多菜单。").style_full(styles::status_style),
            ),
            _ => TabItem::new(
                "advanced",
                "高级",
                Text::new("支持拖拽重排的 tab。").style_full(styles::status_style),
            ),
        })
        .collect()
}

fn list_component(app: &App) -> Element<App> {
    demo_section::component_doc_stacked(
        app,
        "List",
        "List 构建在虚拟滚动基础上，支持分组、选择、loading/empty 和上下文菜单。",
        vec![
            UsageDemo::new(
                "list/selection",
                "分组和多选",
                "点击行、Shift-click 范围和键盘 Enter 都会派发事件。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new(list_selection_summary(app)).style_full(styles::status_style),
                    List::sections(contact_sections(), contact_row)
                        .width(pct(100.0))
                        .height(dp(300.0))
                        .item_layout(ItemLayout::Measured {
                            estimate: dp(64.0),
                            spacing: dp(4.0),
                            overscan: 3,
                        })
                        .selection_mode(ListSelectionMode::Multiple)
                        .selected_keys(app.list_selected_keys.signal())
                        .context_menu(vec![
                            MenuItem::new("Mark as reviewed")
                                .on_select(Command::new(App::list_context_action)),
                            MenuItem::new("Open profile")
                                .on_select(Command::new(App::list_context_action)),
                        ])
                        .on_selection_change(ValueCommand::new(App::set_list_selection))
                        .on_item_action(ValueCommand::new(App::open_list_item)),
                    Text::new(app.list_status.signal()).style_full(styles::status_style),
                ]),
                CODE_LIST_SELECTION,
            ),
            UsageDemo::new(
                "list/states",
                "Loading 和 Empty",
                "按钮切换列表的 loading slot 和 empty slot。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                        Button::new("Toggle loading")
                            .on_click(Command::new(App::toggle_list_loading)),
                        Button::new("Toggle empty").on_click(Command::new(App::toggle_list_empty)),
                        Button::new("Clear")
                            .ghost()
                            .on_click(Command::new(App::clear_list_selection)),
                    ]),
                    List::sections(
                        if app.list_show_empty.get() {
                            Vec::new()
                        } else {
                            contact_sections()
                        },
                        contact_row,
                    )
                    .width(pct(100.0))
                    .height(dp(220.0))
                    .item_layout(ItemLayout::Measured {
                        estimate: dp(64.0),
                        spacing: dp(4.0),
                        overscan: 3,
                    })
                    .loading(app.list_loading.signal())
                    .loading_view(state_view("Loading contact rows..."))
                    .empty(state_view("No contacts")),
                ]),
                CODE_LIST_STATES,
            ),
        ],
    )
}

fn list_selection_summary(app: &App) -> String {
    let keys = app.list_selected_keys.get();
    if keys.is_empty() {
        "No rows selected".to_string()
    } else {
        format!("{} selected: {:?}", keys.len(), keys)
    }
}

fn contact_sections() -> Vec<ListSection<DemoContact, App>> {
    vec![
        ListSection::new(
            section_header("Product"),
            vec![
                ListItem::keyed(
                    "ana",
                    DemoContact::new("Ana Torres", "Product lead", "Planning Q3 roadmap"),
                ),
                ListItem::keyed(
                    "mika",
                    DemoContact::new("Mika Chen", "Designer", "Reviewing interaction states"),
                ),
                ListItem::keyed(
                    "nora",
                    DemoContact::new("Nora Patel", "Research", "Disabled sample row"),
                )
                .disable(true),
            ],
        ),
        ListSection::new(
            section_header("Engineering"),
            vec![
                ListItem::keyed(
                    "owen",
                    DemoContact::new("Owen Blake", "Runtime", "Keyboard navigation"),
                ),
                ListItem::keyed(
                    "li",
                    DemoContact::new("Li Wei", "Rendering", "Virtualized list rows"),
                ),
                ListItem::keyed(
                    "sam",
                    DemoContact::new("Sam Rivera", "Platform", "Context menus"),
                ),
            ],
        ),
    ]
}

fn section_header(text: &'static str) -> Element<App> {
    Stack::new()
        .width(pct(100.0))
        .height(dp(30.0))
        .padding(Insets::symmetric(dp(12.0), dp(6.0)))
        .child(
            Text::new(text)
                .width(pct(100.0))
                .style_full(styles::status_style),
        )
        .into()
}

fn contact_row(ctx: ListItemContext<DemoContact>) -> Element<App> {
    let title = if ctx.selected {
        format!("{} (selected)", ctx.item.name)
    } else {
        ctx.item.name.to_string()
    };
    Flex::vertical()
        .width(pct(100.0))
        .align(Align::Stretch)
        .gap(dp(2.0))
        .child(
            Text::new(title)
                .width(pct(100.0))
                .style_full(styles::usage_title_style),
        )
        .child(
            Text::new(format!("{} - {}", ctx.item.role, ctx.item.status))
                .width(pct(100.0))
                .style_full(styles::status_style),
        )
        .into()
}

fn state_view(text: &'static str) -> Element<App> {
    Stack::new()
        .height(dp(150.0))
        .center()
        .child(Text::new(text).style_full(styles::status_style))
        .into()
}

fn virtual_list_component(app: &App) -> Element<App> {
    demo_section::component_doc_stacked(
        app,
        "VirtualList",
        "VirtualList 只构建可见行，适合大数据量滚动列表。",
        vec![
            UsageDemo::new(
                "virtual/fixed",
                "固定行高",
                "10,000 行数据使用固定 item_extent 和 overscan。",
                VirtualList::new_with_context(app.virtual_rows.clone(), virtual_row)
                    .item_layout(ItemLayout::Fixed {
                        item_extent: dp(32.0),
                        spacing: dp(2.0),
                        overscan: 4,
                    })
                    .width(pct(100.0))
                    .height(dp(300.0)),
                CODE_VIRTUAL_FIXED,
            ),
            UsageDemo::new(
                "virtual/measured",
                "测量行高",
                "List 可用 Measured layout 处理高度更灵活的行。",
                List::sections(contact_sections(), contact_row)
                    .width(pct(100.0))
                    .height(dp(260.0))
                    .item_layout(ItemLayout::Measured {
                        estimate: dp(64.0),
                        spacing: dp(4.0),
                        overscan: 3,
                    }),
                CODE_VIRTUAL_MEASURED,
            ),
        ],
    )
}

fn virtual_row(ctx: ListItemContext<String>) -> Element<App> {
    Stack::new()
        .width(pct(100.0))
        .padding(Insets::symmetric(dp(12.0), dp(6.0)))
        .child(
            Text::new(ctx.item)
                .width(pct(100.0))
                .style_full(styles::status_style),
        )
        .into()
}

fn tree_component(app: &App) -> Element<App> {
    demo_section::component_doc_stacked(
        app,
        "Tree",
        "Tree 展示层级数据，支持展开、选择、三态复选、右键菜单和拖拽 drop 事件。",
        vec![UsageDemo::new(
            "tree/basic",
            "层级节点",
            "受控 keys 保持展开、选择和复选状态，拖拽只派发 drop 事件。",
            Flex::vertical().gap(dp(8.0)).child(el![
                Text::new(tree_summary(app)).style_full(styles::status_style),
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("Toggle loading").on_click(Command::new(App::toggle_tree_loading)),
                    Button::new("Toggle empty").on_click(Command::new(App::toggle_tree_empty)),
                    Button::new("Clear")
                        .ghost()
                        .on_click(Command::new(App::clear_tree_selection)),
                ]),
                Tree::<&'static str, App>::new(
                    if app.tree_show_empty.get() {
                        Vec::new()
                    } else {
                        demo_tree_nodes()
                    },
                    tree_row,
                )
                .width(pct(100.0))
                .height(dp(320.0))
                .expanded_keys(app.tree_expanded_keys.signal())
                .selected_keys(app.tree_selected_keys.signal())
                .selection_mode(TreeSelectionMode::Multiple)
                .checkable(true)
                .checked_keys(app.tree_checked_keys.signal())
                .loading(app.tree_loading.signal())
                .loading_view(state_view("Loading tree nodes..."))
                .empty(state_view("No tree nodes"))
                .draggable(true)
                .context_menu(vec![
                    MenuItem::new("Mark node").on_select(Command::new(App::tree_context_action)),
                    MenuItem::new("Clear selection")
                        .on_select(Command::new(App::clear_tree_selection)),
                ])
                .on_expand_change(ValueCommand::new(App::set_tree_expanded))
                .on_selection_change(ValueCommand::new(App::set_tree_selection))
                .on_check_change(ValueCommand::new(App::set_tree_checked))
                .on_node_action(ValueCommand::new(App::open_tree_node))
                .on_drop(ValueCommand::new(App::drop_tree_node)),
                Text::new(app.tree_status.signal()).style_full(styles::status_style),
            ]),
            CODE_TREE_BASIC,
        )],
    )
}

fn tree_summary(app: &App) -> String {
    format!(
        "{} selected, {} checked, {} expanded",
        app.tree_selected_keys.get().len(),
        app.tree_checked_keys.get().len(),
        app.tree_expanded_keys.get().len()
    )
}

fn demo_tree_nodes() -> Vec<TreeNode<&'static str>> {
    vec![TreeNode::keyed("workspace", "workspace").children([
        TreeNode::keyed("src", "src").children([
            TreeNode::keyed("widgets", "ui/widget").children([
                TreeNode::keyed("tree", "tree/mod.rs"),
                TreeNode::keyed("list", "list/mod.rs"),
                TreeNode::keyed("table", "table/mod.rs"),
            ]),
            TreeNode::keyed("runtime", "runtime/input")
                .children([TreeNode::keyed("tree-runtime", "tree.rs")]),
        ]),
        TreeNode::keyed("docs", "docs").children([
            TreeNode::keyed("readme", "README.md"),
            TreeNode::keyed("roadmap", "COMPONENTS_ROADMAP.md").disable(true),
        ]),
    ])]
}

fn tree_row(ctx: TreeNodeContext<&'static str>) -> Element<App> {
    let label = if ctx.selected {
        format!("{} (selected)", ctx.item)
    } else if ctx.disabled {
        format!("{} (disabled)", ctx.item)
    } else {
        ctx.item.to_string()
    };
    Text::new(label)
        .width(pct(100.0))
        .style_full(styles::status_style)
        .into()
}

fn data_grid_component(app: &App) -> Element<App> {
    demo_section::component_doc_stacked(
        app,
        "DataGrid / Table",
        "DataGrid 展示表格型数据；Table 是同一组件的公开别名。",
        vec![
            UsageDemo::new(
                "datagrid/basic",
                "选择、排序和编辑",
                "表头排序、行选择、单元格编辑和右键菜单都在同一表格中展示。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new(data_summary(app)).style_full(styles::status_style),
                    DataGrid::new(sorted_rows(app), columns(app))
                        .width(pct(100.0))
                        .height(dp(360.0))
                        .density(DataGridDensity::Regular)
                        .selection_mode(DataGridSelectionMode::Multiple)
                        .selected_keys(app.data_selected.signal())
                        .sort(app.data_sort.signal())
                        .row_height(dp(42.0))
                        .overscan(4)
                        .context_menu(vec![
                            MenuItem::new("Mark reviewed").on_select(Command::new(App::mark_data_reviewed)),
                            MenuItem::new("Clear selection").on_select(Command::new(App::clear_data_selection)),
                        ])
                        .on_selection_change(ValueCommand::new(App::select_data_rows))
                        .on_sort_change(ValueCommand::new(App::sort_data_rows))
                        .on_column_width_change(ValueCommand::new(App::resize_data_column))
                        .on_column_reorder(ValueCommand::new(App::reorder_data_column))
                        .on_cell_action(ValueCommand::new(App::open_data_cell))
                        .on_cell_edit_commit(ValueCommand::new(App::commit_data_cell_edit)),
                    Text::new(app.data_status.signal()).style_full(styles::status_style),
                ]),
                CODE_DATAGRID_BASIC,
            ),
            UsageDemo::new(
                "datagrid/columns",
                "列能力",
                "列可排序、调整宽度、重排、固定在开始或结束位置。",
                Text::new("上方表格的 ID 列固定在开始位置，Status 列固定在结束位置，Name/Role/Status 支持编辑。")
                    .style_full(styles::status_style),
                CODE_DATAGRID_COLUMNS,
            ),
            UsageDemo::new(
                "datagrid/table-alias",
                "Table 别名",
                "Table<T, VM> 是 DataGrid<T, VM> 的公开别名，可使用同一套 API。",
                Table::new(sorted_rows(app), columns(app))
                    .width(pct(100.0))
                    .height(dp(260.0))
                    .density(DataGridDensity::Compact)
                    .row_height(dp(34.0))
                    .selection_mode(DataGridSelectionMode::Single),
                CODE_TABLE_ALIAS,
            ),
        ],
    )
}

fn data_summary(app: &App) -> String {
    let selected = app.data_selected.get();
    let sort = app.data_sort.get();
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

fn sorted_rows(app: &App) -> Vec<DataGridRow<Employee>> {
    let mut rows = app.data_rows.get();
    for sort in app.data_sort.get().iter().rev() {
        rows.sort_by(|left, right| compare_employee(left, right, sort));
    }
    rows.into_iter()
        .map(|employee| DataGridRow::keyed(employee.id, employee))
        .collect()
}

fn columns(app: &App) -> Vec<DataGridColumn<Employee, App>> {
    app.data_column_order
        .get()
        .into_iter()
        .filter_map(|key| column_for(app, &key))
        .collect()
}

fn column_for(app: &App, key: &str) -> Option<DataGridColumn<Employee, App>> {
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
            .width(app.data_name_width.signal())
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
            .width(app.data_role_width.signal())
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
            .width(app.data_team_width.signal())
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
            .width(app.data_status_width.signal())
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
