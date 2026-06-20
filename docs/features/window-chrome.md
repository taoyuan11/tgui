# 自定义窗口 Chrome

`tgui` 支持关闭系统标题栏，用组件树绘制自己的标题栏、窗口按钮、拖拽区域和 resize 热区。这个能力适合品牌化桌面应用、透明窗口、工具面板和需要自定义标题栏内容的生产力工具。

## 基础配置

关闭系统 decorations：

```rust
Application::new()
    .decorations(false)
    .clear_color(Color::TRANSPARENT)
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

无边框透明窗口通常需要同时关注：

- `decorations(false)` 关闭系统标题栏。
- `clear_color(Color::TRANSPARENT)` 或绑定透明 clear color。
- 根视图自己绘制窗口背景、边框和圆角。
- 目标平台 compositor 是否支持透明窗口。

如果只想自定义标题栏内容，不需要透明或圆角窗口，可以保留不透明 clear color，这样更稳定。

## 运行时窗口控制

在命令中通过 `ctx.window()` 请求窗口操作：

| API | 说明 |
| --- | --- |
| `drag_window()` | 开始平台原生窗口拖拽。 |
| `drag_resize_window(direction)` | 从指定边或角开始 resize 拖拽。 |
| `minimize()` | 最小化当前窗口。 |
| `maximize()` | 最大化当前窗口。 |
| `restore()` | 从最大化恢复。 |
| `toggle_maximize()` | 最大化和恢复之间切换。 |
| `close()` | 关闭当前窗口。 |
| `is_maximized()` | 查询当前窗口是否最大化。 |

```rust
Button::new("关闭")
    .on_click(Command::new_with_context(|_vm: &mut AppVm, ctx| {
        ctx.window().close();
    }))
```

`drag_window()` 和 `drag_resize_window(...)` 应该由鼠标按下/点击区域触发，运行时会把请求排队并在平台窗口 API 合适的时机执行。

## 自绘标题栏

标题栏通常是一个横向 `Flex`：左侧图标和标题，中间可拖拽区域，右侧窗口按钮。

```rust
fn title_bar(&self) -> Element<Self> {
    Flex::horizontal()
        .height(dp(44.0))
        .align(Align::Center)
        .padding(Insets::horizontal(dp(12.0)))
        .style(|style, ctx| {
            style.surface.background = Some(ctx.theme.colors.surface.into());
            style.surface.border_color = Some(ctx.theme.colors.outline_muted.into());
        })
        .child(Text::new("tgui App").grow(1.0))
        .child(
            Button::new("—")
                .ghost()
                .on_click(Command::new_with_context(|_, ctx| {
                    ctx.window().minimize();
                })),
        )
        .child(
            Button::new("□")
                .ghost()
                .on_click(Command::new_with_context(|_, ctx| {
                    ctx.window().toggle_maximize();
                })),
        )
        .child(
            Button::new("×")
                .danger()
                .on_click(Command::new_with_context(|_, ctx| {
                    ctx.window().close();
                })),
        )
        .on_click(Command::new_with_context(|_vm: &mut AppVm, ctx| {
            ctx.window().drag_window();
        }))
        .into()
}
```

真实应用中建议把按钮替换为图标按钮，并给按钮设置 tooltip。标题栏里的输入框、菜单、按钮不应该触发窗口拖拽；可以把可拖拽区域单独做成中间的 `Stack`，只在那块区域调用 `drag_window()`。

## Resize 热区

无边框窗口失去系统边框后，需要自己放置 resize 热区。方向类型为 `WindowResizeDirection`：

```rust
fn resize_edge(direction: WindowResizeDirection) -> Element<AppVm> {
    Stack::new()
        .position_absolute()
        .on_click(Command::new_with_context(move |_vm: &mut AppVm, ctx| {
            ctx.window().drag_resize_window(direction);
        }))
        .into()
}
```

常用方向：

| 方向 | 位置 |
| --- | --- |
| `North` / `South` | 上边 / 下边 |
| `West` / `East` | 左边 / 右边 |
| `NorthWest` / `NorthEast` | 左上角 / 右上角 |
| `SouthWest` / `SouthEast` | 左下角 / 右下角 |

建议热区宽度保持在 6-10 dp，角落热区略大一些。不要让 resize 热区盖住按钮、输入框或滚动条。

## 最大化状态

最大化状态可在命令中即时查询：

```rust
Button::new(self.maximize_label.signal())
    .on_click(Command::new_with_context(|vm: &mut AppVm, ctx| {
        if ctx.window().is_maximized() {
            ctx.window().restore();
            vm.maximize_label.set("最大化".to_string());
        } else {
            ctx.window().maximize();
            vm.maximize_label.set("还原".to_string());
        }
    }))
```

如果要让按钮图标实时反映窗口状态，建议在平台状态事件完善后绑定对应状态；当前最稳妥的是在触发操作时同步更新 ViewModel，并在需要时用 `is_maximized()` 校正。

## 多窗口

`WindowSpec` 也支持关闭 decorations。辅助窗口可以有自己的自绘 chrome：

```rust
WindowSpec::child("inspector")
    .title("Inspector")
    .window_size(dp(420.0), dp(640.0))
    .decorations(false)
    .root_view(AppVm::inspector_view)
```

命令里的 `ctx.window()` 总是指向触发该命令的当前窗口，因此同一个标题栏组件可以复用于多个窗口。

## 样式建议

- 根视图负责绘制窗口背景，避免透明窗口裸露未绘制区域。
- 标题栏高度保持稳定，按钮使用固定尺寸，避免窗口控件随标题文字变化抖动。
- 深浅色主题都要检查 hover、pressed、danger 和 disabled 状态对比度。
- 透明窗口使用阴影和圆角时，关注平台合成效果；不同系统的窗口边缘抗锯齿可能不同。
- 自绘关闭按钮仍然是破坏性操作，建议使用 `danger` 风格或明确的 hover 反馈。

完整示例见仓库中的 `examples/frameless_window`。
