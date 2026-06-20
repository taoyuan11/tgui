# 示例索引

仓库示例位于 `examples/`，每个示例都是 workspace member，也可以继续通过独立 manifest 运行。

## 运行方式

```sh
cargo run -p basic_window
cargo run --manifest-path examples/basic_window/Cargo.toml
```

把 `basic_window` 替换为目标示例目录名即可。

## 当前示例

| 示例 | 关注点 |
| --- | --- |
| `basic_window` | 最基础窗口启动 |
| `mvvm_counter` | MVVM 状态和命令 |
| `animation_showcase` | 声明式动画能力 |
| `timeline_controller` | 时间线动画控制 |
| `multi_window` | 多窗口声明和生命周期 |
| `dialogs` | 原生对话框 |
| `canvas` | Canvas 绘制与交互 |
| `background_effects` | 背景、模糊和视觉效果 |
| `frameless_window` | 自定义窗口 chrome |
| `demo` | 综合组件展示 |
| `text_area` | 多行文本输入 |
| `multiple_vm_examples` | 多 ViewModel 页面组织 |
| `drawer_demo` | Drawer 浮层 |
| `modal_demo` | Modal 浮层 |
| `list_virtual_list` | 列表与虚拟列表 |
| `table_datagrid` | 表格和 DataGrid |
| `toast_snackbar` | toast / snackbar |
| `tree` | Tree 组件 |

`examples/demo` 按类型组织组件：`Basics` 覆盖 `Badge`、`Avatar`、`Card`、`Icon`、`RichText`、`Collapse`、`Accordion` 和 `ResizablePanels`；`Forms` 覆盖 `DatePicker`、`TimePicker`、`NumberInput`、`ColorPicker`、`Upload`、`Combobox` / `AutoComplete` 和 `Rating`；`Feedback` 覆盖 `Skeleton`；`Data` 覆盖 `Breadcrumb` 和 `Pagination`；`Media & Canvas` 覆盖 `Carousel`。

## 推荐阅读顺序

1. `basic_window`
2. `mvvm_counter`
3. `canvas`
4. `frameless_window`
5. `demo`

如果正在调试某个组件，优先查看同名或相近示例，再阅读对应模块源码。
