
<p align="center">
  <img src="./docs/public/images/tgui_logo.png" width="150px" alt="logo">
</p>

`tgui` 是一个基于 `wgpu` 的 Rust GUI 框架，强调这几件事：

- GPU 加速渲染
- 轻量 MVVM 状态模型
- 基于 `taffy` 的布局系统
- 声明式组件树 + 可绑定窗口属性
- 内置动画、图片/文本、系统通知、对话框、画布、自定义窗口 chrome，以及可选音频/视频能力

适合做桌面 GUI、工具型应用、可视化面板，以及需要较强自定义绘制能力的界面。

## 项目状态

`tgui` 目前已经能够基本使用：应用启动、窗口管理、MVVM 状态绑定、常用布局、基础控件、主题、动画、图片、Canvas、自定义窗口 chrome、系统通知、对话框以及可选音视频播放等核心链路已经打通，并配有多个可运行示例。

当前版本仍处于 `0.x` 阶段，公共 API 还可能根据真实应用反馈继续调整。它已经适合用于原型、内部工具、小型桌面应用、可视化面板和自定义绘制界面的探索；如果用于长期维护的生产项目，建议固定 crate 版本，并在升级前阅读 README、示例和变更记录。

### 版本承诺与升级节奏

- **0.x**：处于公开 API 调整窗口期，破坏性变更可能出现在任意 minor（`0.x.0`）版本；patch（`0.x.y`）只做兼容性修复。每次破坏性变更会在 [`CHANGELOG.md`](./CHANGELOG.md) 列出。
- **1.0 之前**：会做一次系统的公开 API review（重点是 `src/lib.rs` 的 re-export、`Application` 链式 API、widget builder），冻结后用 `cargo public-api` 守门。
- **1.0 之后**：严格遵循 SemVer，破坏性变更只在 major 版本出现。
- **wgpu 升级策略**：当前依赖 `wgpu 29`，跟随主版本时只在 minor 升级，不在 patch 升级；每次升级在 CHANGELOG 中列出迁移点。
- **winit 升级策略**：当前 `winit-* 0.31.0-beta.2`，会在 winit 0.31 stable 化时作为一次显式 minor 升级；详见 [`CHANGELOG.md`](./CHANGELOG.md)。

### MSRV

最低支持 Rust 版本（MSRV）为 **`1.85`**。MSRV 提升被视为 minor-level 变更并在 CHANGELOG 中显式标注。

## 文档站开发

项目文档使用 VitePress，文档工程位于 [`docs`](./docs/)：

```sh
pnpm --dir docs install
pnpm --dir docs dev
pnpm --dir docs build
pnpm --dir docs preview
```

## 当前能力概览

### 应用与窗口

- `Application`：应用入口，配置标题、窗口大小、主题、字体、图标
- `app_id(...)`：为通知等平台服务提供稳定应用标识
- `WindowSpec`：声明式多窗口描述
- `bind_title` / `bind_clear_color` / `bind_theme_mode`：将窗口属性绑定到状态
- `decorations(false)`：关闭系统标题栏，用 tgui 自绘窗口 chrome
- `on_input`：注册窗口级快捷键/输入触发

### 状态与 MVVM

- `ViewModelContext`：创建响应式状态与动画句柄
- `State<T>`：可变状态，更新后自动触发重绘
- `Signal<T>`：从状态派生 UI 值，支持 `map` 和 `animated`
- `TextController`：`Input` / `Textarea` 的保留式文本状态，支持程序化读写与批量变更通知
- `Form` / `FormField<T>` / `TextFormField`：纯 ViewModel 层表单值聚合、校验、错误传播与提交/重置抽象
- `Command<T>` / `ValueCommand<T, V>`：把按钮、输入、画布事件接回 ViewModel
- 生命周期事件：`on_mount`、`on_unmount`、`on_update`
- `CommandContext::window()`：在命令中请求窗口拖拽、拉伸、最小化、最大化/还原、关闭
- `CommandContext::notifications()`：在命令中发送通知、请求权限、处理通知 action 回调

### 布局与组件

- 布局与滚动容器：`Stack`、`Grid`、`Flex`、`ScrollView`、`VirtualViewport`、`VirtualList`、`List`、`DataGrid` / `Table`、`Tree`
- 基础组件：`Text`、`Button`、`Input`、`Textarea`、`Radio`、`Checkbox`、`Select`、`Slider`、`Switch`、`Tabs` / `TabView`、`ProgressBar`、`Spinner`、`Image`、`Badge`、`Avatar` / `AvatarGroup`、`Skeleton`、`Card`、`Icon`
- 体验组件：`Collapse` / `Accordion`、`Splitter` / `ResizablePanels`、`Breadcrumb`、`Pagination`、`Rating`、`RichText`、`Carousel`、`AutoComplete` / `Combobox`
- 浮层基础设施：统一的 runtime overlay anchoring 引擎，当前已为 `Tooltip`、`Popover`、`Select` 与 `Menu` / `ContextMenu` / `MenuBar` 提供锚点定位、自动翻转、脱离父级裁剪、关闭与回焦能力
- `Popover`：支持 click 固定打开、hover 预览、外部点击 / `Esc` 关闭的锚定轻量浮层，可承载任意 widget 子树
- `ToastHost` + `ToastQueue`：应用内 toast / snackbar 队列，支持 success / error / warning / info、自动消失、持久提示、action 按钮以及桌面端 hover 暂停
- 菜单组件：`Menu`（按钮触发的下拉操作菜单）、`ContextMenu`（长按 / 鼠标右键触发的浮层菜单）、`MenuBar`（顶部主菜单条）；统一 `MenuItem` 模型支持图标占位、勾选项、子菜单标识、快捷键提示文本、分隔线、禁用项
- 画布：`Canvas`、`CanvasRecorder`、渐变/阴影/混合/裁剪/文字与图片绘制
- 音频：`Audio`、`AudioController`、`AudioSource`（需启用 `audio` feature）
- 视频：`Video`、`VideoSurface`、`VideoController`、`VideoSource`（需启用 `video` feature）

### 样式与基础类型

- 主题：`Theme`、`ThemeMode`、`ThemeSet`
- 主题状态与存储：`ResolvedThemeMode`、`ThemeStore`
- 颜色：`Color`
- 单位：`dp()`、`sp()`、`Dp`、`Sp`
- 排版：`FontWeight`
- 布局类型：`Align`、`Justify`、`Axis`、`Wrap`、`Overflow`、`Insets`、`Length`、`Track`

### 动画与媒体

- 声明式过渡：`Transition`
- 时间线动画：`AnimatedValue`、`AnimationSpec`、`Keyframes`
- 图片来源：`MediaSource`、`MediaBytes`
- 适配模式：`ContentFit`

### 运行时服务

- 对话框：文件选择、消息框，同步/异步两种调用方式
- 通知：权限查询 / 请求、普通通知、带 action 的交互式通知
- 窗口控制：`WindowControl`、`WindowResizeDirection`
- 日志：`Log`、`tgui_log`、`init_logging_from_cargo_toml!`
- 平台导出：`platform::*`

## 安装

```toml
[dependencies]
tgui = "0.1.8"
```

如果需要音频能力：

```toml
[dependencies]
tgui = { version = "0.1.8", features = ["audio"] }
```

如果需要视频能力：

```toml
[dependencies]
tgui = { version = "0.1.8", features = ["video"] }
```

可选 feature：

- `audio`：启用 FFmpeg + CPAL 音频播放能力
- `video`：启用 FFmpeg 视频播放能力
- `video-static`：在 `video` 基础上启用静态链接 FFmpeg 的音视频能力

性能优化已默认内置，无需配置 feature：

- 细粒度场景命令原地拼接：改深层叶子属性时跳过祖先链重合成。
- 属性级依赖归因：把视觉属性读取归因到具体属性槽，未识别属性安全退化为整 widget 失效。
- GPU 顶点脏区间增量上传：逐帧只上传变化字节区间，完全相同则跳过上传。
- 纯滚动快路径：优先用 GPU per-draw 平移；adapter 或场景前置不满足时回退到 CPU 子树重收集，再失败则整帧重收集。

细粒度增量渲染管线的完整说明见 [性能文档](./docs/advanced/performance.md)。

移动端支持说明：当前版本暂时放弃 Android、HarmonyOS / OpenHarmony 等移动端支持，相关入口、feature、示例和平台依赖已经移除。`tgui` 目前聚焦 Windows、macOS 与 Linux 桌面端。

## 公开 API 结构

`tgui` 的公开类型按职责分类导出：

- `application`：应用、窗口和运行入口
- `mvvm`：`ViewModel`、`ViewModelContext`、`State`、`Signal`、`TextController`、`Form`、`FormField`、`TextFormField`、`Command`、`ValueCommand`、`CommandContext`、`WindowControl`
- `layout`：布局容器、尺寸、间距和滚动相关类型
- `widgets` / `canvas`：基础控件、控件树和 Canvas 绘制 API
- `theme`：主题、色板、排版、状态和设计 token
- `core`：颜色、错误、输入触发器、基础单位和几何类型
- `notification`：系统通知、权限与交互式 action
- `media` / `dialog` / `logging` / `platform` / `audio` / `video`：媒体、对话框、日志、平台和音视频能力

示例代码可使用 `tgui::prelude::*` 引入常用 API；库代码建议优先从具体分类模块导入。

## 快速开始

`tgui` 只支持 MVVM 启动路径。即使是静态界面，也需要定义一个命名 ViewModel 并显式实现 `ViewModel`。

```rust
use tgui::prelude::*;

struct CounterVm {
    count: State<u32>,
}

impl CounterVm {
    fn increment(&mut self) {
        self.count.update(|value| *value += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .child(Text::new(
                self.count.signal().map(|count| format!("Count: {count}")),
            ))
            .child(
                Button::new("Increment")
                    .on_click(Command::new(Self::increment)),
            )
            .into()
    }
}

impl ViewModel for CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.state(0),
        }
    }

    fn view(&self) -> Element<Self> {
        CounterVm::view(self)
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .with_view_model(CounterVm::new)
        .root_view(CounterVm::view)
        .run()
}
```

## 典型 API 入口

常见应用启动链路大致如下：

```rust
Application::new()
    .title("demo")
    .window_size(dp(960.0), dp(640.0))
    .theme(Theme::dark())
    .decorations(true)
    .with_view_model(AppVm::new)
    .bind_title(AppVm::title)
    .bind_clear_color(AppVm::clear_color)
    .bind_theme_mode(AppVm::theme_mode)
    .on_input(InputTrigger::KeyPressed(/* ... */), Command::new(AppVm::handle_input))
    .root_view(AppVm::view)
    .windows(AppVm::windows)
    .run()
```

其中最常用的公开类型包括：

```rust
Application
WindowSpec
ViewModel
ViewModelContext
State<T>
Signal<T>
TextController
Command<T>
ValueCommand<T, V>
CommandContext<T>
WindowControl
WindowResizeDirection
NotificationOptions
NotificationAction
Notifications

Stack / Grid / Flex / ScrollView / VirtualViewport / VirtualList / List / DataGrid / Table / Tree
Text / Button / Image / Canvas
ItemSource<T> / ItemLayout / VirtualArrangement / VirtualDirection
DataGridColumn<T, VM> / DataGridRow<T> / DataGridSort
Tree<T, VM> / TreeNode<T> / TreeNodeContext<T> / TreeCheckState

Theme / ThemeMode / ThemeSet / ThemeStore / ResolvedThemeMode / Color / FocusRingStyle
dp / sp / Dp / Sp

Transition
AnimatedValue<T>
AnimationSpec<T>
Keyframes<T>
Form
FormField<T>
TextFormField
```

## 列表与虚拟列表

`List` 是面向产品界面的受控列表组件，基于 `VirtualList` 渲染。它支持单选 / 多选、分组、空态、加载态、键盘导航、双击 / `Enter` 主动作以及行右键菜单；选择态由 ViewModel 持有，组件通过 `on_selection_change` 发出下一状态。

```rust
use tgui::prelude::*;

struct MailVm {
    selected: State<Vec<WidgetKey>>,
}

impl ViewModel for MailVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            selected: ctx.state(vec![WidgetKey::from("inbox")]),
        }
    }

    fn view(&self) -> Element<Self> {
        let folders = vec![
            ListItem::keyed("inbox", "Inbox"),
            ListItem::keyed("sent", "Sent"),
            ListItem::keyed("archive", "Archive"),
        ];

        List::new(folders, |ctx| Text::new(ctx.item).into())
            .selected_keys(self.selected.signal())
            .on_selection_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.selected.set(change.selected_keys);
            }))
            .on_item_action(ValueCommand::new(|_vm: &mut Self, action| {
                println!("open row {}", action.index);
            }))
            .empty(Text::new("No folders").into())
            .loading(false)
            .loading_view(Text::new("Loading...").into())
            .height(dp(240.0))
            .into()
    }
}
```

`VirtualList` 是 `VirtualViewport` 的语义薄封装，默认垂直列表、固定 40dp 行高、overscan 为 2。适合长列表或下拉候选项，只会把可见范围附近的行实例化进 widget tree。

```rust
use tgui::prelude::*;

fn rows_view<VM: 'static>() -> Element<VM> {
    let rows = (0..100_000).collect::<Vec<_>>();

    VirtualList::new(rows, |_visible_index, row| {
        Text::new(format!("Row {row}"))
            .height(dp(40.0))
            .into()
    })
    .height(dp(360.0))
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(40.0),
        spacing: dp(4.0),
        overscan: 4,
    })
    .into()
}
```

## Table / DataGrid

`DataGrid`（别名 `Table`）是受控多列表格组件。组件负责表头/单元格渲染、虚拟化、选择、排序交互、列宽拖拽、列重排、固定列、右键菜单和文本编辑提交事件；真实 rows、排序结果、列宽、列顺序和提交后的数据由 ViewModel 更新。

```rust
use tgui::prelude::*;

#[derive(Clone)]
struct Person {
    id: &'static str,
    name: String,
    role: String,
}

struct PeopleVm {
    rows: State<Vec<Person>>,
    selected: State<Vec<WidgetKey>>,
    sort: State<Vec<DataGridSort>>,
}

impl PeopleVm {
    fn view(&self) -> Element<Self> {
        let rows = self
            .rows
            .get()
            .into_iter()
            .map(|row| DataGridRow::keyed(row.id, row))
            .collect::<Vec<_>>();
        let columns = vec![
            DataGridColumn::new("name", "Name".to_string(), |ctx: DataGridCellContext<Person>| {
                Text::new(ctx.row.name).into()
            })
            .width(dp(180.0))
            .sortable(true)
            .text_value(|row| row.name.clone())
            .editable(true),
            DataGridColumn::new("role", "Role".to_string(), |ctx: DataGridCellContext<Person>| {
                Text::new(ctx.row.role).into()
            })
            .width(dp(180.0))
            .sortable(true),
        ];

        DataGrid::new(rows, columns)
            .selected_keys(self.selected.signal())
            .selection_mode(DataGridSelectionMode::Multiple)
            .sort(self.sort.signal())
            .on_selection_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.selected.set(change.selected_keys);
            }))
            .on_sort_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.sort.set(change.sort);
            }))
            .on_cell_edit_commit(ValueCommand::new(|vm: &mut Self, commit| {
                vm.rows.update(|rows| {
                    if let Some(row) = rows.iter_mut().find(|row| WidgetKey::from(row.id) == commit.row_key) {
                        row.name = commit.value;
                    }
                });
            }))
            .height(dp(360.0))
            .into()
    }
}
```

## Tree

`Tree` 是受控层级数据组件，基于 `VirtualList` 渲染可见节点。组件负责展开箭头、缩进、三态 checkbox、选择高亮、键盘导航、右键菜单和拖放命中；真实树数据、展开集合、选择集合、勾选集合和拖放后的改父级由 ViewModel 更新。

```rust
use tgui::prelude::*;

struct FilesVm {
    expanded: State<Vec<WidgetKey>>,
    selected: State<Vec<WidgetKey>>,
    checked: State<Vec<WidgetKey>>,
}

impl FilesVm {
    fn view(&self) -> Element<Self> {
        let nodes = vec![
            TreeNode::keyed("src", "src").children([
                TreeNode::keyed("widgets", "ui/widget")
                    .child(TreeNode::keyed("tree", "tree/mod.rs")),
                TreeNode::keyed("runtime", "runtime/input"),
            ]),
        ];

        Tree::new(nodes, |ctx| Text::new(ctx.item).into())
            .expanded_keys(self.expanded.signal())
            .selected_keys(self.selected.signal())
            .selection_mode(TreeSelectionMode::Multiple)
            .checkable(true)
            .checked_keys(self.checked.signal())
            .on_expand_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.expanded.set(change.expanded_keys);
            }))
            .on_selection_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.selected.set(change.selected_keys);
            }))
            .on_check_change(ValueCommand::new(|vm: &mut Self, change| {
                vm.checked.set(change.checked_keys);
            }))
            .height(dp(360.0))
            .into()
    }
}
```

## 表单校验

`tgui` 的表单抽象位于 `tgui::mvvm`，是纯 ViewModel 层能力，不引入新的 widget。文本类输入使用 `TextFormField`，其他录入控件通常使用 `FormField<T>` 配合现有 `on_change(...)`。

```rust
use tgui::prelude::*;

fn build_form_ui(ctx: &ViewModelContext) -> Element<()> {
    let form = Form::new(ctx);
    let email = form
        .text_field("email", "")
        .validator(|value| {
            if value.trim().is_empty() {
                ValidationErrors::single("请输入邮箱")
            } else {
                ValidationErrors::none()
            }
        });
    let agree = form
        .field("agree", false)
        .validator(|value| {
            if *value {
                ValidationErrors::none()
            } else {
                ValidationErrors::single("请先同意协议")
            }
        });

    Flex::new(Axis::Vertical)
        .gap(dp(8.0))
        .child(Input::<()>::new(email.controller()).placeholder("邮箱"))
        .child(
            Text::new(
                email
                    .first_error()
                    .map(|value| value.unwrap_or_default()),
            )
            .color(Color::hexa(0xDC2626FF)),
        )
        .child(
            Checkbox::new(agree.signal())
                .label("同意协议")
                .on_change(agree.bind_change()),
        )
        .into()
}
```

如果你希望在失焦或值变化时提前显示错误，可以继续沿用现有 `on_blur` / `on_change` 事件，在回调里显式调用字段或整个表单的 `validate()`。

## 组件生命周期事件

所有公开组件都支持：

- `on_mount(Command<VM>)`
- `on_unmount(Command<VM>)`
- `on_update(Command<VM>)`

语义说明：

- `on_mount`：组件首次进入当前窗口的组件树时触发一次
- `on_unmount`：组件在应用仍运行时从当前组件树移除时触发一次
- `on_update`：同一组件身份在一次 view 重建后仍然存在时触发一次

注意事项：

- 动态列表建议始终显式设置 `.key(...)`，这样生命周期身份才会稳定
- `on_update` 表示“同身份组件参与了一次重建”，不是“属性 diff 后确实有变化”
- 关闭窗口或应用退出时，不额外保证补发 `on_unmount`

## 仓库示例

仓库内示例基本覆盖了当前主要能力：

- `basic_window`：命名空 ViewModel 驱动的最小完整窗口
- `mvvm_counter`：响应式状态、标题绑定、清屏色绑定、快捷键输入
- `animation_showcase`：`Signal::animated` 声明式过渡
- `timeline_controller`：时间线动画控制器
- `multi_window`：共享 ViewModel 的多窗口
- `dialogs`：同步/异步文件选择与消息框
- `canvas`：scene-style 画布，支持 path/text/image/group/clip、渐变、阴影、布尔运算和 item 事件
- `background_effects`：通用渐变背景和 backdrop blur
- `frameless_window`：关闭系统装饰后的自绘标题栏、拖拽、拉伸和窗口按钮
- `demo`：综合展示常用布局、组件、P3 组件、`Tooltip` / `Popover` / `Tabs` / `Toast`、通知和画布
- `toast_snackbar`：`ToastHost` / `ToastQueue` 专项示例，覆盖语义提示、action、持久提示、短时提示和不同位置
- `list_virtual_list`：`List` / `VirtualList` 专项示例，覆盖受控多选、分组、loading / empty slot、行主动作、右键菜单和大数据虚拟化
- `table_datagrid`：`DataGrid` / `Table` 专项示例，覆盖受控选择、排序、列宽、列重排、固定列、右键菜单和文本提交
- `tree`：`Tree` 专项示例，覆盖受控展开、选择、三态复选、empty / loading slot、右键菜单和拖拽改父级
- `text_area`：受控 `Textarea` 编辑示例，读取自身源码但不保存
- `multiple_vm_examples`：多页面 / 多 ViewModel 示例

这些示例是独立小工程，运行方式如下：

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
cargo run --manifest-path examples/list_virtual_list/Cargo.toml
cargo run --manifest-path examples/table_datagrid/Cargo.toml
cargo run --manifest-path examples/tree/Cargo.toml
```

README 中的示例名称以当前 `examples/` 目录为准；如果新增或删除示例，应同步更新本节和 `examples/README.md`。

## Notification

通知服务通过 `CommandContext::notifications()` 提供，支持：

- `send(...)`：发送普通通知
- `send_with_actions(...)`：发送最多两个 action 的交互式通知
- `request_permission(...)` / `permission_status()`：权限查询与请求

使用通知前，建议在应用入口设置稳定的应用标识：

```rust
Application::new()
    .app_id("com.example.tgui-demo")
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

```rust
Button::new("发送通知").on_click(Command::new_with_context(|_, ctx| {
    let _ = ctx.notifications().send(
        NotificationOptions::new("TGUI Demo")
            .body("任务已经完成")
            .app_name("TGUI Demo"),
    );
}))
```

平台说明：

- Windows：建议始终设置 `Application::app_id(...)`，这是通知身份的前置条件。
- Linux：当前通过 `notify-rust` 发送通知，并支持 action 回调。
- macOS：支持普通通知；系统通知 action 当前标记为不支持。

## 图片、画布与视频

### 图片

`Image` 支持：

- 本地路径
- URL
- 内存字节
- SVG 资源加载与栅格化

相关类型：

- `Image`
- `MediaSource`
- `MediaBytes`
- `ContentFit`

### 画布

`Canvas` 适合做自定义图形与交互式绘制，公开入口现在统一为记录式 API。常用能力包括：

- `CanvasRecorder`
- `CanvasScene`
- `CanvasItem` / `CanvasPath` / `CanvasText` / `CanvasImage` / `CanvasGroup`
- `CanvasStroke`
- `CanvasTransform2D`
- `CanvasLinearGradient`
- `CanvasRadialGradient`
- `CanvasShadow`
- `CanvasBlendMode`
- `CanvasTextStyle` / `CanvasParagraphStyle`
- `CanvasMouseEvent` / `CanvasWheelEvent` / `CanvasDragEvent`

除了 recorder，`CanvasScene` 现在也可以作为 retained scene 使用，支持：

- `items()` / `items_mut()`
- `find(id)` / `find_named(name)`
- `visit(...)`
- `remove(id)`
- `query_point(...)` / `query_point_all(...)`
- `query_point_with(...)` / `query_point_all_with(...)`
- `debug_info()`
- `export_json()`
- `export_debug_text()` / `export_debug_json()`

这让 Canvas 不只适合声明式绘制，也开始适合作为编辑器、白板、流程图和设计器的基础场景层。

示例：

```rust
Canvas::new(CanvasRecorder::build(|canvas| {
    canvas
        .set_fill(Color::hexa(0x0EA5E9FF))
        .fill_round_rect(24.0, 24.0, 180.0, 96.0, dp(24.0));
    canvas
        .set_text_style(CanvasTextStyle {
            color: Color::WHITE,
            font_size: sp(18.0),
            ..Default::default()
        })
        .draw_text(Rect::new(36.0, 44.0, 140.0, 32.0), "Recorded items");
}))
```

也可以直接持有并查询 `CanvasScene`：

```rust
use tgui::canvas::*;

let scene = CanvasRecorder::build(|canvas| {
    canvas
        .next_item_name("hero")
        .fill_round_rect(0.0, 0.0, 120.0, 60.0, dp(16.0));
});

let item = scene.find_named("hero").expect("named item should exist");
println!("item id={}", item.id().get());
println!("{}", scene.export_debug_text());
println!("{}", scene.export_json());
if let Some(hit) = scene.query_point(Point::new(12.0, 12.0)) {
    println!("hit item={}", hit.item_id.get());
}
```

更完整的能力说明、限制和 retained scene 建议见 [Canvas 文档](./docs/features/canvas.md)。

### 通用背景

除 `Canvas` 外，常规控件背景现在也支持更丰富的视觉能力：

- `BackgroundBrush`
- `BackgroundLinearGradient`
- `BackgroundRadialGradient`
- `BackgroundGradientStop`
- `Shadow`
- `background_brush(...)`
- `background_blur(...)`

`background_blur(...)` 是应用窗口内容上的 backdrop blur，可用于玻璃卡片、磨砂面板和层叠浮层。

常规控件也可以通过 style resolver 上的 `style.surface.shadow` 配置单层外阴影：

```rust
Stack::new().style(|style, _ctx| {
    style.surface.shadow = Some(tgui::theme::Shadow {
        offset_x: dp(0.0),
        offset_y: dp(8.0),
        blur: dp(24.0),
        spread: dp(-4.0),
        color: Color::hexa(0x00000033),
    }.into());
})
```

`SliderStyle` 还支持 `thumb_shadow`，用于单独给圆形 thumb 添加阴影，而不会影响整个 slider 外框。

### 音频

启用 `audio` feature 后可使用：

- `audio::Audio`
- `audio::AudioController`
- `audio::AudioSource`
- `audio::AudioPlaybackState`
- `audio::AudioMetrics`

`Audio` 是一个不渲染任何 UI 的隐形组件，只负责把音频播放生命周期挂进 widget tree；业务按钮、进度条、音量条由你自己用 `AudioController` 拼。

```rust
use std::time::Duration;
use tgui::prelude::*;

struct AudioVm {
    audio: AudioController,
}

impl ViewModel for AudioVm {
    fn new(ctx: &ViewModelContext) -> Self {
        let audio = AudioController::new(ctx);
        audio
            .load(AudioSource::url("https://example.com/demo.mp3"))
            .expect("failed to load audio source");
        Self { audio }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .child(Audio::new(self.audio.clone()).autoplay(true).looping(false))
            .child(Button::new("Play").on_click(Command::new(|vm: &mut Self| vm.audio.play())))
            .child(Button::new("Pause").on_click(Command::new(|vm: &mut Self| vm.audio.pause())))
            .child(
                Button::new("Replay 10s")
                    .on_click(Command::new(|vm: &mut Self| {
                        let target = vm.audio.position().get().saturating_sub(Duration::from_secs(10));
                        vm.audio.seek(target);
                    })),
            )
            .into()
    }
}
```

网络音频如果需要自定义请求头，可以把 header 挂在 `AudioSource` 上，再通过 `AudioController::load(...)` 设置源：

```rust
let source = tgui::audio::AudioSource::url("https://example.com/demo.mp3")
    .with_header("Authorization", "Bearer <token>")
    .with_headers([
        ("Referer", "https://example.com/player"),
        ("Cookie", "session=abc123"),
    ]);

controller.load(source)?;
```

### 视频

启用 `video` feature 后可使用：

- `video::Video`
- `video::VideoController`
- `video::VideoSurface`
- `video::VideoSource`
- `video::VideoPlaybackState`
- `video::VideoMetrics`

`Video` 是浏览器式内置控制栏播放器，组合了画面、底部 SVG 图标控制栏、播放/暂停、seek、缓冲、时间、音量/静音和状态文本；`VideoSurface` 是更低层的画面 surface，适合自定义控制栏。

网络视频如果需要自定义请求头，可以把 header 直接挂在 `VideoSource` 上：

```rust
let source = tgui::video::VideoSource::url("https://example.com/demo.mp4")
    .with_header("Authorization", "Bearer <token>")
    .with_headers([
        ("Referer", "https://example.com/player"),
        ("Cookie", "session=abc123"),
    ]);

controller.load(source)?;
```

## 多窗口与平台支持

当前仅支持 Windows、macOS、Linux 桌面端。项目暂时放弃移动端支持，因此不再提供 `run_android`、`run_ohos`、`android` feature、`ohos` feature 或对应示例。

多窗口通过 `WindowSpec` 描述，主窗口与子窗口共享同一个 ViewModel，适合做文档窗口、检查器窗口、浮动工具面板等场景。

`Application::decorations(false)` 或 `WindowSpec::decorations(false)` 可以关闭系统标题栏，用普通 tgui 组件自绘窗口 chrome。命令处理里可以通过 `ctx.window()` 操作当前窗口：

```rust
Button::new("Close")
    .on_click(Command::new_with_context(|_, ctx| {
        ctx.window().close();
    }));
```

可用窗口控制包括：

- `drag_window()`
- `drag_resize_window(WindowResizeDirection::SouthEast)`
- `minimize()`
- `maximize()` / `restore()` / `toggle_maximize()`
- `close()`
- `is_maximized()`

透明无边框窗口通常同时设置 `clear_color(Color::TRANSPARENT)`，渲染器会根据 clear color 的 alpha 选择合适的 surface alpha mode。

## 对话框与运行时服务

### 日志配置

所有 `tgui` 日志行都会带本地时间戳：

```text
[YYYY-MM-DD HH:mm:ss.SSS +08:00] [INFO] [tgui] message
```

默认不写本地文件，且最低等级为 `trace`，保持未配置应用的既有输出行为。应用可以在自己的 `Cargo.toml` 中配置日志，并在第一条需要配置生效的日志之前初始化：

```toml
[package.metadata.tgui.logging]
level = "debug"
log_dir = "logs"
file_name = "tgui.log"
max_file_size_bytes = 10485760
max_files = 5
```

```rust
fn main() -> Result<(), tgui::core::TguiError> {
    tgui::init_logging_from_cargo_toml!()?;
    // start Application here
    Ok(())
}
```

`log_dir` 相对路径基于应用 `Cargo.toml` 所在目录解析。设置后日志会继续输出到平台日志 sink，同时写入本地文件；轮转文件命名为 `tgui.log.1`、`tgui.log.2` 等。

通过 `Command::new_with_context` 或 `ValueCommand::new_with_context`，可以在命令处理中访问运行时服务：

- `ctx.dialogs()`：文件选择、消息框
- `ctx.notifications()`：系统通知、权限、action 回调
- `ctx.window()`：当前窗口控制
- `ctx.log()`：运行时日志

相关类型：

- `Dialogs`
- `NotificationOptions`
- `NotificationAction`
- `NotificationActionEvent`
- `NotificationError`
- `NotificationPermission`
- `Notifications`
- `FileDialogOptions`
- `MessageDialogOptions`
- `MessageDialogButtons`
- `MessageDialogResult`

## 线程模型与 Send / Sync

`tgui` 的事件循环、widget 树解析、布局、渲染都在主线程上同步执行；ViewModel 也只在主线程被读写。因此公共 API 中绝大多数类型并不要求 `Sync`：能跨线程的边界主要发生在 ViewModel 持有的回调（按钮 `Command`、命令上下文里的异步完成回调等）以及命令通道里。下面的表格只列出对调用方有约束的位置，方便排查 `not Send` / `not Sync` 编译错误。

| 位置 | 约束 | 说明 |
|------|------|------|
| `ViewModel` 实现 | `Send + 'static` | ViewModel 由 runtime 通过 `Arc<Mutex<VM>>` 持有，命令派发时短暂跨线程移动；不要求 `Sync`，但任何字段如果想在 `Command` 闭包之外按引用共享，需要自己包成 `Arc<Mutex<...>>` 或类似容器。 |
| `Command::new` / `ValueCommand::new` 闭包 | `Fn(&mut VM[, V]) + Send + Sync + 'static` | 命令对象可能被 runtime 缓存并从命令通道线程读取，因此闭包要 `Send + Sync`。闭包**只在主线程**被调用，但类型系统层面仍需要 `Sync`。 |
| `Command::new_with_context` / `ValueCommand::new_with_context` 闭包 | 同上，签名追加 `&CommandContext<VM>` | `CommandContext` 不可跨线程持有，只在闭包调用期间有效。 |
| `WindowSpec::root_view` / `Application::root_view` | `Fn(&VM) -> Element<VM> + Send + Sync + 'static` | runtime 在主线程调用，但与 `Command` 一样需要 `Send + Sync` 才能放进命令通道。 |
| `ctx.signal(reader)` | `Fn() -> T + Send + Sync + 'static`，`T: Clone + Send + Sync + 'static` | `Signal` 的求值会随渲染管线在主线程发生，但绑定层为了跨依赖图共享会要求 `Send + Sync`。 |
| `NotificationActionEvent` 回调 | `FnOnce(NotificationActionEvent) + Send + 'static` | 通知 action 由系统在 worker 线程触发，回调通过 `async_notification_channel` 队列回到主线程；闭包必须 `Send`。 |
| `Dialogs::*` 完成回调 | `FnOnce(...) + Send + 'static` | 文件 / 消息对话框在 worker 线程运行，结果走命令通道回主线程。 |
| `State<T>` / `Signal<T>` / `TextController` | 不要求 `Sync` | 仅在主线程访问；不要把它们克隆到工作线程使用。 |

`Application::run` 不消费 `'static` 之外的引用：所有传入的工厂、命令、绑定都需要满足上面的 `'static` 约束。如果你需要从 worker 线程把数据送回 ViewModel，常见的做法是：

1. 在 `with_view_model` / `view` 中保存一个 `State<T>` 或 `TextController`。
2. 在工作线程拿到 `Command<VM>` 的克隆，把消息打包送到自定义的 `mpsc::channel`。
3. 在 `bind_*` 或 `on_input` 触发的命令中 drain 通道并 `state.set(...)`，由 runtime 自动唤醒事件循环重绘。

## 适合先看哪些文件

- `src/lib.rs`：crate 导出总览
- `src/application/mod.rs`：应用与窗口入口
- `src/foundation/binding/mod.rs`：`State` / `Signal` / `TextController`
- `src/foundation/view_model/mod.rs`：`Command` / `ValueCommand`
- `src/notification/mod.rs`：通知、权限与 action 回调
- `src/foundation/window_control.rs`：`WindowControl` / `WindowResizeDirection`
- `src/audio/`：音频控制器、隐形 `Audio` 组件和 FFmpeg/CPAL 播放管线
- `src/ui/widget/`：组件与布局实现
- `examples/frameless_window/src/main.rs`：无边框窗口和窗口控制参考
- `examples/*`：最直接的上手参考

## 贡献指南

欢迎提交 issue、示例、文档和代码改进。`tgui` 的目标是保持 API 易用、行为可预测，同时保留足够强的自定义绘制能力；贡献时建议优先围绕真实使用场景描述问题和改动动机。

提交代码前建议至少运行：

```bash
cargo fmt
cargo check
cargo test
```

如果改动涉及特定 feature 或平台，请尽量补充对应检查：

```bash
cargo check --features video
```

贡献时请注意：

- 公共 API 变更需要同步更新 README、示例和 `src/lib.rs` 中的 re-export。
- 新增 widget 或样式能力时，优先复用现有 `Element`、布局、事件、主题和 `Value<T>` / `Signal<T>` 模式。
- 文本输入或通知能力变更时，建议同时检查 `src/runtime/`、`src/notification/` 与相关示例。
- 修改 `src/runtime/`、`src/ui/widget/core/`、渲染 primitive、文本输入、媒体加载或窗口控制时，建议补充针对性的单元测试。
- 新增示例时保持示例独立、可运行，并同步更新 README 中的示例列表。
- 音频相关改动需要考虑 `audio` feature、本机 FFmpeg 解码链路以及桌面端 `cpal` 播放行为。
- 视频相关改动需要考虑 `video` / `video-static` feature，以及本机 FFmpeg 链接环境差异。
- 桌面平台相关改动请使用 `cfg` 明确隔离，避免影响其他平台构建。
- 文档和示例同样重要；如果你发现某个 API 已经可用但缺少说明，欢迎直接补充。

较大的功能改动建议先开 issue 讨论设计方向，尤其是涉及公开 API、运行时事件、布局行为、渲染管线或平台抽象的改动。

## License

本项目源代码采用双协议授权，可在以下两种协议中任选其一：

- [MIT 协议](./LICENSE-MIT)
- [Apache License, Version 2.0](./LICENSE-APACHE)

项目内置图标由本仓库维护为 SVG 资源，不依赖第三方字体图标资产。`Cargo.toml` 的包级 license metadata 标记为 `MIT OR Apache-2.0`。

除非你明确声明，否则你提交到本项目的任何贡献都将按 Apache-2.0 的定义同时以上述两个协议授权，无附加条款。详见 [`NOTICE`](./NOTICE)。
