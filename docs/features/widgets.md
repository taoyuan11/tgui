# 组件

组件通过 builder 组合成 `Element<VM>`。每个组件只描述“当前应该是什么样”，状态写入、事件处理和异步任务都回到 ViewModel。多数属性接受静态值或 `Signal<T>`，因此同一个 API 可以用于一次性静态界面，也可以用于受控组件。

下面的示例默认位于 `ViewModel::view` 或页面函数中，并已通过 `tgui::prelude::*` 导入常用类型。

## 组件模型

一个典型组件通常包含四类信息：

| 类别 | 常用 API | 说明 |
| --- | --- | --- |
| 数据 | `Text::new(...)`、`Input::new(controller)`、`Select::new(options, selected)` | 组件的核心内容或受控值。 |
| 布局 | `width`、`height`、`size`、`margin`、`grow`、`shrink`、`align_self` | 参与父容器布局。容器组件还支持 `padding`、`gap`、`align`、`justify`。 |
| 视觉 | `style(...)`、`style_full(...)`、`background`、`border_radius`、`opacity` | 局部覆盖主题或设置容器表面。 |
| 事件 | `on_click`、`on_change`、`on_open_change`、`on_mount`、`on_update` | 通过 `Command` / `ValueCommand` 回到 ViewModel。 |

有 payload 的事件使用 `ValueCommand<VM, T>`，例如 `Slider` 的新值、`Select` 的 `(key, value)`、`DataGrid` 的排序事件。没有 payload 的事件使用 `Command<VM>`。

```rust
Button::new("保存")
    .primary()
    .disable(self.saving.signal())
    .on_click(Command::new(AppVm::save))
```

## 基础展示

| 组件 | 构造方式 | 常用 API | 用途 |
| --- | --- | --- | --- |
| `Text` | `Text::new(content)` | `style`、`style_full`、`user_select`、鼠标事件 | 普通文本、状态文本、可选择日志。 |
| `RichText` | `RichText::markdown(markdown)` | `on_link_click`、`style` | Markdown 文档、帮助文本、轻量说明。 |
| `Button` | `Button::new(label)` | `primary`、`secondary`、`ghost`、`danger`、`disable`、`tooltip`、`on_click` | 命令入口。 |
| `Icon` | `Icon::builtin(...)` / `Icon::glyph(...)` / `Icon::svg(...)` | `style`、布局方法 | 小图标、按钮内容、状态提示。 |
| `Image` | `Image::new(source)` / `from_path` / `from_url` / `from_bytes` | `style`、`on_loading`、`on_success`、`on_error` | 本地、网络、内存图片和 SVG。 |

### Text 与 RichText

```rust
Flex::vertical().gap(dp(8.0)).child(el![
    Text::new(self.title.signal())
        .style(|style, _ctx| {
            style.typography.size = sp(20.0);
        }),
    Text::new("日志路径可以选择复制")
        .user_select(true),
    RichText::markdown("### 变更\n- **新增** 组件文档\n- 支持 `code`")
        .on_link_click(ValueCommand::new(|vm: &mut AppVm, link: RichTextLinkClick| {
            vm.status.set(format!("打开链接: {}", link.href));
        })),
])
```

`Text::new(...)` 接受字符串、`Signal<String>` 和 `Value<String>`。需要富文本、列表、代码块或链接回调时使用 `RichText::markdown(...)`。

### Button

```rust
Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
    Button::new("主按钮").primary(),
    Button::new("次按钮").secondary(),
    Button::new("幽灵按钮").ghost(),
    Button::new("危险操作").danger(),
    Button::new("禁用").disable(true),
    Button::new("带提示").tooltip(Tooltip::new("保存当前配置")),
])
```

按钮点击一般只做事件入口，业务状态由 ViewModel 处理：

```rust
Button::new("提交")
    .primary()
    .on_click(Command::new(|vm: &mut AppVm| {
        vm.submit();
    }))
```

## 表面与标识

| 组件 | 构造方式 | 常用 API | 用途 |
| --- | --- | --- | --- |
| `Card` | `Card::new()` | `header`、`body`、`footer`、`child`、`style` | 结构化信息块。 |
| `Badge` | `Badge::dot()` / `text(...)` / `count(...)` | `tone`、`max`、`placement`、`offset`、`attach` | 数量、状态、标签。 |
| `Avatar` | `Avatar::image(...)` / `initials(...)` / `name(...)` | `shape`、`badge`、`on_click` | 用户或对象身份。 |
| `AvatarGroup` | `AvatarGroup::new(vec![...])` | `max_visible`、`style` | 一组参与者。 |
| `Divider` | `Divider::new()` | `vertical`、`label`、`dashed`、`thickness`、`color` | 分隔内容区。 |
| `Collapse` | `Collapse::new(title, content)` | `expanded`、`on_change` | 单个折叠区块。 |
| `Accordion` | `Accordion::new(items, selected_key)` | `on_change` | 多个折叠项，同一时间展开一个 key。 |

```rust
Card::new()
    .width(dp(360.0))
    .header(Text::new("Release candidate"))
    .body(RichText::markdown("**Ready** for desktop QA."))
    .footer(
        Flex::horizontal().gap(dp(8.0)).child(el![
            Badge::text("UI").tone(BadgeTone::Primary),
            Badge::count(128u32).max(99),
        ]),
    )
```

给图标、头像或任意元素挂角标：

```rust
Badge::count(self.unread.signal())
    .max(99)
    .tone(BadgeTone::Error)
    .attach(Icon::builtin(BuiltinIcon::Info).size(dp(28.0), dp(28.0)))
```

## 输入与选择

基础表单组件采用受控值模式：组件读取 `State` / `TextController`，事件回调把新值写回 ViewModel。

| 组件 | 构造方式 | 事件 payload | 常用 API |
| --- | --- | --- | --- |
| `Input` | `Input::new(TextController)` | `on_change_set(TextChangeSet)` 或无 payload `on_change` | `placeholder`、`validation`、`disable`。 |
| `Textarea` | `Textarea::new(TextController)` | 同 `Input` | `show_scrollbar`、`auto_wrap`、`placeholder`。 |
| `Checkbox` | `Checkbox::new(checked)` | `bool` | `label`、`validation`、`disable`。 |
| `Switch` | `Switch::new(checked)` | `bool` | `validation`、`disable`。 |
| `Radio` | `Radio::new(checked)` | `bool` | `label`、`validation`、`disable`。 |
| `RadioGroup` | `RadioGroup::new(options, selected_key)` | `(K, V)` | `horizontal`、`vertical`、`direction`。 |
| `Select` | `Select::new(options, selected_key)` | `(K, V)` | `placeholder`、`open`、`on_open_change`、`validation`。 |
| `Combobox` / `AutoComplete` | `Combobox::new(controller, options)` | `ComboboxChange` | `selected_key`、`open`、`allow_custom`、`filter`。 |
| `Slider` | `Slider::new(value, min, max)` | `f32` | `step`、`vertical`、`show_ticks`、`show_value_label`、`format_value`。 |
| `Rating` | `Rating::new(value)` | `RatingChange` | `max`、`half`、`step`、`read_only`。 |

### 文本输入

```rust
struct AppVm {
    name: TextController,
    status: State<String>,
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            name: ctx.text_controller(""),
            status: ctx.state(String::new()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical().gap(dp(8.0)).child(el![
            Input::new(self.name.clone())
                .width(dp(320.0))
                .placeholder("请输入名称")
                .on_change(Command::new(|vm: &mut AppVm| {
                    vm.status.set("名称已更新".to_string());
                })),
            Text::new(self.status.signal()),
        ]).into()
    }
}
```

`Input::from_value(...)` / `Textarea::from_value(...)` 适合只读或简单展示，不会替代 `TextController` 的选择区间、IME composition 和编辑历史能力。真实表单输入优先使用 `TextController`。

### 选择控件

```rust
RadioGroup::new(
    vec![
        RadioOption::new("system".to_string(), "跟随系统".to_string()),
        RadioOption::new("light".to_string(), "明亮".to_string()),
        RadioOption::new("dark".to_string(), "暗色".to_string()),
    ],
    self.theme_key.signal(),
)
.horizontal()
.on_change(ValueCommand::new(|vm: &mut AppVm, (key, _label)| {
    vm.theme_key.set(key);
}))
```

```rust
Select::new(
    vec![
        SelectOption::new("archive".to_string(), "归档".to_string()),
        SelectOption::new("delete".to_string(), "删除".to_string()).disable(true),
        SelectOption::new("share".to_string(), "分享".to_string()),
    ],
    self.action.signal(),
)
.placeholder("请选择操作")
.on_change(ValueCommand::new(|vm: &mut AppVm, (key, _label)| {
    vm.action.set(Some(key));
}))
```

### Slider 与 Rating

```rust
Slider::new(self.volume.signal(), 0.0, 100.0)
    .step(5.0)
    .show_ticks(true)
    .show_value_label(true)
    .format_value(|value| format!("{value:.0}%"))
    .on_change(ValueCommand::new(|vm: &mut AppVm, value| {
        vm.volume.set(value);
    }))
```

```rust
Rating::new(self.score.signal())
    .half()
    .on_change(ValueCommand::new(|vm: &mut AppVm, change: RatingChange| {
        vm.score.set(change.value);
    }))
```

日期、时间、数字、颜色和上传控件见[表单增强控件](/features/input-controls)。

## 反馈与状态

| 组件 | 构造方式 | 常用 API | 用途 |
| --- | --- | --- | --- |
| `ProgressBar` | `ProgressBar::new(value)` / `indeterminate(open)` | `show_label`、`label` | 进度、后台任务状态。 |
| `Spinner` | `Spinner::new()` | `thickness`、`track`、`style` | 不确定等待。 |
| `Skeleton` | `Skeleton::rect()` / `circle()` / `line()` / `lines(n)` | `style` | 加载占位。 |
| `ToastHost` | `ToastHost::new(queue)` | `placement`、`max_visible` | 全局 toast 展示队列。 |

```rust
Flex::vertical().gap(dp(10.0)).child(el![
    ProgressBar::new(self.progress.signal())
        .show_label(true)
        .label(self.progress.signal().map(|v| format!("{:.0}%", v * 100.0))),
    ProgressBar::indeterminate(self.loading.signal()),
    Spinner::new().thickness(dp(3.0)),
])
```

Toast 通常由 ViewModel 持有 `ToastQueue`，页面根部放一个 `ToastHost`：

```rust
Stack::new()
    .child(main_content)
    .child(
        ToastHost::new(self.toasts.clone())
            .placement(ToastPlacement::BottomRight)
            .max_visible(4),
    )
```

## 数据组件

数据组件把“数据源、行渲染函数、受控状态和事件”分开。行数据应尽量有稳定 key，方便选择、展开、拖拽、局部刷新和虚拟滚动。

| 组件 | 构造方式 | 关键受控状态 | 主要事件 |
| --- | --- | --- | --- |
| `Tabs` / `TabView` | `Tabs::new(items, selected)` | selected key | `on_change`、`on_reorder` |
| `List` | `List::new(items, row)` / `List::sections(...)` | selected keys | `on_selection_change`、`on_item_action` |
| `VirtualList` | `VirtualList::new(...)` / `new_with_context(...)` | 数据源 | 普通鼠标/生命周期事件 |
| `Tree` | `Tree::new(nodes, row)` | expanded / selected / checked keys | `on_expand_change`、`on_selection_change`、`on_check_change`、`on_drop` |
| `DataGrid` / `Table` | `DataGrid::new(rows, columns)` | selected keys、sort、列宽 | selection、sort、column resize、cell action/edit |
| `Breadcrumb` | `Breadcrumb::new(items)` | 无 | item `on_click` |
| `Pagination` | `Pagination::new(page, page_count)` | page、page_size | `on_change(PaginationChange)` |

### Tabs

```rust
Tabs::new(
    vec![
        TabItem::new("overview", "概览", Text::new("概览内容")),
        TabItem::new("settings", "设置", Text::new("设置内容")),
    ],
    self.selected_tab.signal(),
)
.overflow_mode(TabsOverflowMode::More)
.reorderable(true)
.on_change(ValueCommand::new(|vm: &mut AppVm, (key, _label)| {
    vm.selected_tab.set(key);
}))
```

`TabView` 是同一组件的别名，可用 `placement(TabPlacement::Left)` 做侧向 tabs。

### List 与 VirtualList

```rust
fn contact_row(ctx: ListItemContext<Contact>) -> Element<AppVm> {
    Flex::vertical()
        .gap(dp(2.0))
        .child(Text::new(ctx.item.name))
        .child(Text::new(ctx.item.role))
        .into()
}

List::new(self.contacts.get(), contact_row)
    .height(dp(320.0))
    .selection_mode(ListSelectionMode::Multiple)
    .selected_keys(self.selected_contacts.signal())
    .item_layout(ItemLayout::Measured {
        estimate: dp(56.0),
        spacing: dp(4.0),
        overscan: 3,
    })
    .on_selection_change(ValueCommand::new(AppVm::set_contact_selection))
```

大量同质行优先使用 `VirtualList::new_with_context(...)`，并选择固定高度布局：

```rust
VirtualList::new_with_context(self.rows.clone(), |ctx: ListItemContext<String>| {
    Text::new(ctx.item).into()
})
.height(dp(360.0))
.item_layout(ItemLayout::Fixed {
    item_extent: dp(32.0),
    spacing: dp(2.0),
    overscan: 4,
})
```

### Tree

```rust
Tree::new(self.nodes.get(), tree_row)
    .height(dp(360.0))
    .expanded_keys(self.expanded.signal())
    .selected_keys(self.selected.signal())
    .selection_mode(TreeSelectionMode::Multiple)
    .checkable(true)
    .checked_keys(self.checked.signal())
    .draggable(true)
    .on_expand_change(ValueCommand::new(AppVm::set_expanded))
    .on_selection_change(ValueCommand::new(AppVm::set_selected))
    .on_check_change(ValueCommand::new(AppVm::set_checked))
```

### DataGrid / Table

```rust
fn columns(vm: &AppVm) -> Vec<DataGridColumn<Employee, AppVm>> {
    vec![
        DataGridColumn::new("name", "Name".to_string(), |ctx: DataGridCellContext<Employee>| {
            Text::new(ctx.row.name).into()
        })
        .width(vm.name_width.signal())
        .min_width(dp(140.0))
        .sortable(true)
        .editable(true),
    ]
}

DataGrid::new(self.rows.get(), columns(self))
    .height(dp(420.0))
    .selection_mode(DataGridSelectionMode::Multiple)
    .selected_keys(self.selected_rows.signal())
    .sort(self.sort.signal())
    .on_selection_change(ValueCommand::new(AppVm::select_rows))
    .on_sort_change(ValueCommand::new(AppVm::sort_rows))
    .on_cell_edit_commit(ValueCommand::new(AppVm::commit_cell_edit))
```

`Table<T, VM>` 是 `DataGrid<T, VM>` 的公开别名，适合在业务代码中使用更简短的命名。

## 浮层和导航

| 组件 | 构造方式 | 说明 |
| --- | --- | --- |
| `Tooltip` | `Tooltip::new(text)` / `Tooltip::content(element)` | 可挂在 `Button::tooltip(...)` 等触发器上。 |
| `Popover` | `Popover::new(trigger)` | 触发器 + 任意内容，支持受控 open。 |
| `Menu` | `Menu::new(trigger)` | 按钮菜单、快捷键提示、checkable、submenu。 |
| `ContextMenu` | `ContextMenu::new(child)` | 右键或长按触发。 |
| `Modal` | `Modal::new(open)` | 对话框、焦点陷阱、Esc/backdrop 关闭。 |
| `Drawer` | `Drawer::new(open)` | 侧边、顶部、底部抽屉；`DrawerHost` 支持 push 模式。 |
| `Portal` | `Portal::new(...)` | 需要手动控制 anchor/layer 的高级浮层。 |

详细用法见[交互与 Portal](/features/interaction-portal)。

## 选择建议

| 需求 | 推荐组件 |
| --- | --- |
| 简单垂直或水平组合 | `Flex` |
| 需要重叠、居中、背景层 | `Stack` |
| 仪表盘、表单两列、多区域布局 | `Grid` |
| 普通按钮命令 | `Button` |
| 短文本输入 | `Input` |
| 多行编辑 | `Textarea` |
| 少量选项 | `RadioGroup` 或 `Select` |
| 可搜索选项 | `Combobox` / `AutoComplete` |
| 大量列表 | `VirtualList` 或 `List` |
| 层级文件树 | `Tree` |
| 表格型业务数据 | `DataGrid` / `Table` |
| 自定义图表、白板、编辑器 | `Canvas` |

更多完整页面可直接运行 `cargo run -p demo`，它按 Basics、Forms、Feedback、Data、Overlays、Media & Canvas 等页面展示这些组件。
