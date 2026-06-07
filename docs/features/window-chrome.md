# 自定义窗口 Chrome

`tgui` 支持关闭系统标题栏，用组件树绘制自己的标题栏、窗口按钮和拖拽区域。

## 基础配置

```rust
Application::new()
    .decorations(false)
    .clear_color(Color::TRANSPARENT)
```

无边框透明窗口通常需要同时关注：

- 系统 decorations 是否关闭。
- clear color alpha 是否透明。
- surface composite alpha mode。
- 平台后端对透明窗口的支持。

## 运行时窗口控制

在命令中通过 `ctx.window()` 请求窗口操作：

- 拖拽窗口。
- 拖拽调整大小。
- 最小化。
- 最大化。
- 还原。
- 查询最大化状态。
- 关闭窗口。

这些请求会进入运行时队列，由平台窗口 API 在合适时机执行。

## 示例

查看仓库中的 `examples/frameless_window`，它演示了关闭系统标题栏后使用 tgui 组件实现自定义标题栏和窗口控制按钮。
