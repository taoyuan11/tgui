# 交互、手势与 Portal

除了基础点击、焦点和鼠标事件，`tgui` 还提供手势识别、文件拖放、浮层定位、应用内模态层和跨层 Portal。它们共享运行时命中测试、focus scope、关闭行为和 overlay z-order，因此可以和普通组件一样放在 ViewModel 的声明式视图里。

## 基础交互事件

多数可见组件或容器支持以下事件：

| API | 命令类型 | 说明 |
| --- | --- | --- |
| `on_click(...)` | `Command<VM>` | 鼠标点击或默认激活动作。 |
| `on_double_click(...)` | `Command<VM>` | 双击。 |
| `on_focus(...)` / `on_blur(...)` | `Command<VM>` | 焦点进入和离开。 |
| `on_mouse_enter(...)` / `on_mouse_leave(...)` | `Command<VM>` | hover 进入和离开。 |
| `on_mouse_move(...)` | `ValueCommand<VM, Point>` | 鼠标位置变化。 |
| `on_file_drop(...)` | `ValueCommand<VM, FileDropEvent>` | 文件拖放到区域。 |

```rust
Stack::new()
    .size(dp(360.0), dp(180.0))
    .center()
    .on_file_drop(ValueCommand::new(|vm: &mut AppVm, event: FileDropEvent| {
        vm.dropped_files.set(event.paths);
    }))
    .child(Text::new("拖放文件到这里"))
```

`Upload` 已经内置 drop zone 和文件对话框。如果只是收集文件队列，优先使用 `Upload`；如果需要把拖放映射到画布、节点图或自定义区域，再使用 `on_file_drop`。

## Tooltip

`Tooltip` 是挂在任意元素上的修饰符，适合短提示。默认 hover 延迟为 500ms，位置为 `Placement::top()`，空间不足时按 `FlipPolicy::FlipSide` 翻转。

```rust
Button::new("同步")
    .primary()
    .tooltip(
        Tooltip::new("从远端拉取最新配置")
            .placement(Placement::bottom().align(Alignment::Center))
            .delay(std::time::Duration::from_millis(250)),
    )
```

需要复杂内容时使用 `Tooltip::content(...)`：

```rust
Icon::builtin(BuiltinIcon::Info)
    .tooltip(Tooltip::content(
        Flex::vertical()
            .gap(dp(4.0))
            .child(Text::new("状态说明"))
            .child(Text::new("黄色表示任务仍在等待外部资源")),
    ))
```

## Popover

`Popover` 把任意 trigger 和任意内容组合成可控弹层。它适合轻量设置面板、筛选器、日期/颜色 picker 和小型详情卡。

```rust
Popover::new(Button::new("筛选").secondary())
    .open(self.filter_open.signal())
    .on_open_change(ValueCommand::new(|vm: &mut AppVm, open| {
        vm.filter_open.set(open);
    }))
    .placement(Placement::bottom().align(Alignment::Start))
    .flip_policy(FlipPolicy::FlipAndShift)
    .match_anchor_width(true)
    .content(
        Flex::vertical()
            .width(dp(280.0))
            .padding(Insets::all(dp(12.0)))
            .gap(dp(8.0))
            .child(Text::new("筛选条件"))
            .child(Checkbox::new(self.only_open.signal()).label("只看未完成")),
    )
```

常用 API：

| API | 说明 |
| --- | --- |
| `open(value)` | 外部受控开闭状态。 |
| `on_open_change(command)` | trigger 点击、Esc、外部点击导致开闭变化时回调。 |
| `content(element)` | 弹层内容，必须设置。 |
| `placement(Placement)` | 相对 trigger 的方向和对齐。 |
| `flip_policy(FlipPolicy)` | 空间不足时如何翻转或平移。 |
| `close_on_escape(bool)` | 是否允许 Esc 关闭。 |
| `close_on_outside_click(bool)` | 是否允许点击外部关闭。 |
| `match_anchor_width(bool)` | 弹层宽度是否匹配 trigger 宽度。 |

## Menu 和 ContextMenu

`Menu` 是 trigger 修饰符，适合按钮下拉菜单。`MenuItem` 支持普通动作、分隔线、勾选项、子菜单、图标和快捷键提示。

```rust
Menu::new(Button::new("文件"))
    .items(vec![
        MenuItem::new("新建")
            .shortcut_hint("Ctrl+N")
            .on_select(Command::new(AppVm::new_file)),
        MenuItem::new("打开")
            .shortcut(KeyChord::new(KeyCode::KeyO).ctrl())
            .on_select(Command::new(AppVm::open_file)),
        MenuItem::separator(),
        MenuItem::checkable("自动保存")
            .checked(self.auto_save.signal())
            .on_select(Command::new(|vm: &mut AppVm| {
                vm.auto_save.update(|value| *value = !*value);
            })),
    ])
    .placement(Placement::bottom().align(Alignment::Start))
```

右键菜单使用 `ContextMenu`，菜单项类型和 `Menu` 相同。适合表格行、树节点、Canvas item 等上下文操作。

## Modal

`Modal` 是应用内阻塞式对话框：打开后显示 backdrop，建立 focus trap，并支持 Esc 或点击 backdrop 关闭。它不会调用平台原生消息框；原生消息框见[对话框与通知](/features/dialogs-notifications)。

```rust
Modal::new(self.confirm_open.signal())
    .title("删除项目")
    .content(Text::new("此操作无法撤销，确认继续吗？"))
    .on_open_change(ValueCommand::new(|vm: &mut AppVm, open| {
        vm.confirm_open.set(open);
    }))
    .action(ModalAction::new("取消").on_click(Command::new(|vm: &mut AppVm| {
        vm.confirm_open.set(false);
    })))
    .action(ModalAction::primary("删除").on_click(Command::new(|vm: &mut AppVm| {
        vm.delete_selected();
        vm.confirm_open.set(false);
    })))
```

常用 API：`title(...)`、`content(...)`、`action(...)`、`actions(...)`、`close_on_escape(...)`、`close_on_backdrop_click(...)`、`auto_focus_first(...)`、`return_focus_to(...)`。

## Drawer

`Drawer` 从窗口边缘滑出，常用于导航、设置面板或移动窄屏布局。默认是 overlay 模式；`DrawerMode::Push` 需要配合 `DrawerHost`。

```rust
Drawer::new(self.nav_open.signal())
    .placement(DrawerPlacement::Left)
    .on_open_change(ValueCommand::new(|vm: &mut AppVm, open| {
        vm.nav_open.set(open);
    }))
    .content(
        Flex::vertical()
            .padding(Insets::all(dp(16.0)))
            .gap(dp(8.0))
            .child(Text::new("导航"))
            .child(Button::new("总览"))
            .child(Button::new("设置")),
    )
```

## Overlay 定位类型

`Tooltip`、`Popover`、菜单、picker、combobox 和 `Portal` 共享运行时 overlay 定位引擎。常用类型：

| 类型 | 说明 |
| --- | --- |
| `Placement` | 主轴方向 + 交叉轴对齐，例如 `Placement::bottom().align(Alignment::Start)`。 |
| `Side` | `Top`、`Bottom`、`Left`、`Right`。 |
| `Alignment` | `Start`、`Center`、`End`。 |
| `FlipPolicy` | `None`、`ShiftOnly`、`FlipSide`、`FlipAndShift`、`Hide`。 |
| `OverlayLayer` | `Tooltip`、`Popover`、`Menu`、`Modal`、`Toast`，用于 z-order。 |
| `AnchorKey` | 稳定锚点标识，可被 Portal 定位引用。 |

定位引擎会结合 viewport padding、offset、cross offset、flip policy 和 anchor 尺寸计算最终矩形。普通组件通常只需要设置 `placement(...)`；只有跨层投递或手动锚点时才需要直接使用 `Portal`。

## Portal

`Portal` 把一个 `Element` 子树渲染到顶层 overlay layer。它可用于逃逸父级裁剪、实现全局浮层，或把内容投递到另一个运行时管理窗口。

```rust
Portal::new(
    Stack::new()
        .padding(Insets::all(dp(12.0)))
        .child(Text::new("全局浮层")),
)
.open(self.portal_open.signal())
.layer(OverlayLayer::Popover)
.anchor(PortalAnchor::Viewport)
.placement(Placement::top())
.offset(dp(16.0))
.close_on_escape(true)
.close_on_outside_click(true)
.on_open_change(ValueCommand::new(|vm: &mut AppVm, open| {
    vm.portal_open.set(open);
}))
```

跨窗口投递：

```rust
Portal::new(Text::new("投递到预览窗口"))
    .target_window("preview")
    .layer(OverlayLayer::Toast)
    .anchor(PortalAnchor::Viewport)
```

Portal 目标和锚点：

| API / 类型 | 说明 |
| --- | --- |
| `target(PortalTarget::CurrentWindow)` | 渲染到当前窗口。 |
| `target_window(key)` / `PortalTarget::window(key)` | 渲染到指定窗口 key。 |
| `stack(LayerStack::current(layer))` | 同时指定当前窗口和 layer。 |
| `stack(LayerStack::window(key, layer))` | 同时指定目标窗口和 layer。 |
| `anchor(PortalAnchor::SelfWidget)` | 使用 Portal 声明位置的自身 frame。 |
| `anchor(PortalAnchor::Viewport)` | 使用目标窗口 viewport。 |
| `anchor(Rect)` / `anchor(Point)` | 使用显式矩形或点。 |
| `anchor(AnchorKey)` | 使用运行时记录的稳定锚点。 |

`SelfWidget` 只适合当前窗口；跨窗口 Portal 通常使用 viewport、rect、point 或 anchor key。

## 手势识别

`GestureRecognizer` 可挂到任意 `Element` 上，识别长按、双击/双触、滑动、边缘滑动和双指缩放。

```rust
let content: Element<AppVm> = Element::from(
    Stack::new()
        .size(dp(320.0), dp(240.0))
        .center()
        .child(Text::new("gesture area")),
)
.gesture(
    GestureRecognizer::new()
        .on_long_press(ValueCommand::new(AppVm::handle_long_press))
        .on_swipe(SwipeAxis::Horizontal, ValueCommand::new(AppVm::handle_swipe))
        .on_edge_swipe(
            GestureEdgeSet::horizontal(),
            ValueCommand::new(AppVm::handle_edge_swipe),
        )
        .on_pinch(ValueCommand::new(AppVm::handle_pinch)),
);
```

常用事件类型包括 `LongPressEvent`、`DoubleTapEvent`、`SwipeGestureEvent`、`EdgeSwipeEvent` 和 `PinchGestureEvent`。事件中会带上 widget id、输入来源、阶段、位置和触控 finger id；`GesturePhase` 用于区分 `Start`、`Update`、`End`、`Cancel` 和 `Recognized`。

## 焦点与关闭

浮层类组件建议明确关闭行为：

- 可轻松重开的面板：保留 `close_on_escape(true)` 和 `close_on_outside_click(true)`。
- 需要用户确认的流程：使用 `Modal`，在动作按钮中关闭。
- 会修改大量状态的长表单：使用 `Drawer` 或普通页面，不要塞进很小的 `Popover`。
- 关闭后需要回到触发按钮：记录 trigger 的 `WidgetId` 并调用 `return_focus_to(...)`。
