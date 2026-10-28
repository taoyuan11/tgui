# 示例索引

仓库示例位于 `examples/`，每个示例都是 workspace member，也可以继续通过独立 manifest 运行。阅读示例时建议先看 `Cargo.toml`，再看 `src/main.rs`；综合 demo 会按页面拆分到 `examples/demo/src/pages/`。

## 运行方式

```sh
cargo run -p basic_window
cargo run --manifest-path examples/basic_window/Cargo.toml
```

把 `basic_window` 替换为目标示例目录名即可。部分示例会打开桌面窗口；如果在无图形环境中运行，需要先准备对应平台的显示服务。

## 推荐阅读路线

1. `basic_window`：理解最小 Application 启动。
2. `mvvm_counter`：理解 `State`、`Signal` 和 `Command`。
3. `demo`：按组件类型浏览完整控件用法。
4. `canvas`：学习 Canvas recorder、scene 和事件。
5. `frameless_window`：学习无边框窗口和运行时窗口控制。
6. `table_datagrid`、`list_virtual_list`、`tree`：学习数据密集型组件。

## 当前示例

| 示例 | 关注点 | 可搭配阅读 |
| --- | --- | --- |
| `basic_window` | 最基础窗口启动、标题、窗口大小和根视图。 | [快速开始](/guide/quick-start)、[应用与窗口](/guide/application) |
| `mvvm_counter` | MVVM 状态和命令，最小可交互应用。 | [MVVM 状态模型](/guide/mvvm) |
| `animation_showcase` | 声明式属性动画、主题变化过渡。 | [主题与样式](/features/theme) |
| `timeline_controller` | 时间线动画控制、keyframes 和播放控制。 | [MVVM 状态模型](/guide/mvvm) |
| `multi_window` | 多窗口声明、窗口 key、辅助窗口生命周期。 | [应用与窗口](/guide/application) |
| `dialogs` | 原生文件对话框和消息对话框。 | [对话框与通知](/features/dialogs-notifications) |
| `canvas` | Canvas 绘制、命中和交互。 | [Canvas](/features/canvas) |
| `background_effects` | 背景、模糊、透明和视觉效果。 | [主题与样式](/features/theme)、[媒体](/features/media) |
| `frameless_window` | 自定义窗口 chrome、拖拽、resize、窗口按钮。 | [自定义窗口 Chrome](/features/window-chrome) |
| `demo` | 综合组件展示和页面组织。 | [组件](/features/widgets)、[表单增强控件](/features/input-controls) |
| `text_area` | 多行文本输入、选择、滚动和文本控制器。 | [组件](/features/widgets)、[MVVM 状态模型](/guide/mvvm) |
| `multiple_vm_examples` | 多 ViewModel 页面组织和 scoped command。 | [MVVM 状态模型](/guide/mvvm) |
| `drawer_demo` | Drawer 浮层、侧边栏导航。 | [交互与 Portal](/features/interaction-portal) |
| `modal_demo` | 应用内 Modal、动作按钮、关闭行为。 | [交互与 Portal](/features/interaction-portal) |
| `list_virtual_list` | List、VirtualList、虚拟滚动和行布局。 | [组件](/features/widgets)、[布局系统](/features/layout) |
| `table_datagrid` | DataGrid/Table、列定义、选择、排序、编辑。 | [组件](/features/widgets) |
| `toast_snackbar` | ToastHost、toast 队列和临时反馈。 | [组件](/features/widgets) |
| `tree` | Tree 节点、展开、选择、勾选和拖拽。 | [组件](/features/widgets) |
| `task_console` | 类控制台/任务面板界面，适合参考工具型应用组织。 | [布局系统](/features/layout)、[组件](/features/widgets) |

## demo 页面地图

`examples/demo` 是最完整的组件索引，按页面组织：

| 页面 | 覆盖内容 |
| --- | --- |
| `Basics` | `Badge`、`Avatar`、`Card`、`Icon`、`RichText`、`Collapse`、`Accordion`、`ResizablePanels`。 |
| `Forms` | `DatePicker`、`TimePicker`、`NumberInput`、`ColorPicker`、`Upload`、`Combobox` / `AutoComplete`、`Rating`。 |
| `Feedback` | `ProgressBar`、`Spinner`、`Skeleton`、toast/snackbar 相关状态。 |
| `Data` | `Tabs`、`List`、`Breadcrumb`、`Pagination`、表格式数据展示。 |
| `Overlays` | `Tooltip`、`Popover`、`Menu`、`Modal`、`Drawer`、Portal 类浮层。 |
| `Media & Canvas` | 图片、Carousel、Canvas 和媒体展示。 |

## 调试示例的建议

- 想确认 API 名称时，先在示例里 `rg "ComponentName::"`，再去对应源码模块。
- 修改共享组件后，优先运行覆盖该组件的示例做人工检查。
- 示例是 workspace member，根目录 `cargo check --workspace --all-targets` 会覆盖示例编译。
- README 中提到的示例名称应以当前 `examples/` 目录为准。
