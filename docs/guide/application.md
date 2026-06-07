# 应用与窗口

`Application` 负责配置应用级信息、主窗口、ViewModel、窗口属性绑定和运行入口。

## 常用启动链路

```rust
Application::new()
    .app_id("com.example.demo")
    .title("demo")
    .window_size(dp(960.0), dp(640.0))
    .theme(Theme::dark())
    .decorations(true)
    .with_view_model(AppVm::new)
    .bind_title(AppVm::title)
    .bind_clear_color(AppVm::clear_color)
    .bind_theme_mode(AppVm::theme_mode)
    .root_view(AppVm::view)
    .run()
```

常见配置项：

- `app_id(...)`：为系统通知等平台服务提供稳定身份。
- `title(...)` / `window_size(...)`：设置主窗口标题和初始尺寸。
- `theme(...)` / `bind_theme_mode(...)`：设置或绑定主题。
- `bind_title(...)` / `bind_clear_color(...)`：把窗口属性连接到 ViewModel 状态。
- `decorations(false)`：关闭系统标题栏，用 tgui 自绘窗口 chrome。
- `on_input(...)`：注册窗口级快捷键或输入触发。

## 多窗口

多窗口通过 `WindowSpec` 描述。ViewModel 可以返回窗口声明，运行时会根据角色和窗口 key 管理生命周期。

适合使用多窗口的场景：

- 主窗口 + 设置窗口。
- 工具面板、检查器、预览窗口。
- 需要独立尺寸、标题或关闭策略的辅助界面。

## 窗口控制

命令处理函数可以通过 `CommandContext::window()` 请求运行时窗口操作，包括拖拽、拉伸、最小化、最大化/还原和关闭。

自绘窗口 chrome 通常会组合使用：

- `Application::decorations(false)`。
- 透明或自定义 `clear_color`。
- 标题栏区域上的拖拽命令。
- 最大化、最小化和关闭按钮。
