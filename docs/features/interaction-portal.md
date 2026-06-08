# 交互、手势与 Portal

除了基础点击、焦点和鼠标事件，`tgui` 还公开了手势识别、文件拖放、overlay 定位和 Portal 能力。它们适合移动触控适配、跨层浮层、跨窗口 overlay 和高级工具界面。

## 基础交互事件

组件 builder 通常支持：

- `on_click`
- `on_double_click`
- `on_focus`
- `on_blur`
- `on_mouse_enter`
- `on_mouse_leave`
- `on_mouse_move`
- `on_file_drop`

文件拖放事件使用 `FileDropEvent`，其中包含拖入的路径列表。`Upload` 已经内置了 drop zone 处理；自定义文件投放区域可以直接在任意容器上使用 `on_file_drop(...)`。

## GestureRecognizer

`GestureRecognizer` 可挂到任意 `Element` 上，识别长按、双击/双触、滑动、边缘滑动和双指缩放。

```rust
let content: Element<App> = Element::from(
    Stack::new()
        .size(dp(320.0), dp(240.0))
        .child(Text::new("gesture area")),
)
.gesture(
    GestureRecognizer::new()
        .on_long_press(ValueCommand::new(App::handle_long_press))
        .on_swipe(SwipeAxis::Horizontal, ValueCommand::new(App::handle_swipe))
        .on_edge_swipe(
            GestureEdgeSet::horizontal(),
            ValueCommand::new(App::handle_edge_swipe),
        )
        .on_pinch(ValueCommand::new(App::handle_pinch)),
);
```

常用事件类型：

- `LongPressEvent`
- `DoubleTapEvent`
- `SwipeGestureEvent`
- `EdgeSwipeEvent`
- `PinchGestureEvent`

事件里会带上 `widget_id`、输入来源、阶段、位置和触控 finger id。`GesturePhase` 用于区分 `Start`、`Update`、`End`、`Cancel` 和 `Recognized`。

## Overlay 定位类型

`Tooltip`、`Popover`、`Select`、菜单、picker、combobox 和 `Portal` 共享 runtime overlay 定位引擎。公开类型包括：

- `OverlayPlacement`
- `OverlayAlignment`
- `OverlaySide`
- `OverlayFlipPolicy`
- `OverlayLayer`
- `OverlayAnchorKey`

常见写法：

```rust
Popover::new(Button::new("Open"))
    .content(Text::new("Floating content"))
    .placement(OverlayPlacement::bottom().align(OverlayAlignment::Start))
```

定位引擎会根据 viewport padding、flip policy、offset 和 cross offset 计算最终位置。

## Portal

`Portal` 把一个 `Element` 子树渲染到顶层 overlay layer。它可用于从当前组件树逃逸父级裁剪，也可把内容投递到运行时管理的另一个窗口。

```rust
Portal::new(Text::new("floating"))
    .layer(OverlayLayer::Popover)
    .anchor(PortalAnchor::Viewport)
    .placement(OverlayPlacement::top())
    .close_on_escape(true)
    .close_on_outside_click(true)
    .on_open_change(ValueCommand::new(|app: &mut App, open| {
        app.portal_open.set(open);
    }))
```

跨窗口投递：

```rust
Portal::new(Text::new("remote portal"))
    .target_window("secondary")
    .layer(OverlayLayer::Toast)
```

## Portal 目标和锚点

目标：

- `PortalTarget::CurrentWindow`
- `PortalTarget::WindowKey(String)`
- `LayerStack::current(layer)`
- `LayerStack::window(key, layer)`

锚点：

- `PortalAnchor::SelfWidget`
- `PortalAnchor::Viewport`
- `PortalAnchor::Rect(...)`
- `PortalAnchor::Point(...)`
- `PortalAnchor::Key(...)`

`SelfWidget` 只适合当前窗口；跨窗口 Portal 通常使用 viewport、rect、point 或 anchor key。

## 焦点与关闭

`Portal` 支持：

- `close_on_escape(true)`
- `close_on_outside_click(true)`
- `return_focus_to(widget_id)`
- `focus_scope(FocusScopeOptions)`

这些能力和内建浮层共用运行时关闭与回焦路径。对菜单、弹窗、模态层这类交互区域，优先显式设置关闭行为和焦点范围。
