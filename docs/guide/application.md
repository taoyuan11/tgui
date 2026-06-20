# 应用与窗口

`Application` 负责配置应用级信息、主窗口、主题、字体、资源预算、ViewModel、窗口属性绑定和运行入口。一般应用只需要一个 `Application::new()` 链式调用；复杂工具可以使用 `WindowSpec` 声明多窗口。

## 常用启动链路

```rust
use tgui::prelude::*;

fn main() -> Result<(), TguiError> {
    Application::new()
        .app_id("com.example.demo")
        .title("Demo")
        .window_size(dp(960.0), dp(640.0))
        .theme_mode(ThemeMode::System)
        .theme_set(ThemeSet::default())
        .decorations(true)
        .resource_budget(ResourceBudget::default())
        .with_view_model(AppVm::new)
        .bind_title(AppVm::title)
        .bind_clear_color(AppVm::clear_color)
        .bind_theme_mode(AppVm::theme_mode)
        .root_view(AppVm::view)
        .run()
}
```

常见配置项：

| API | 说明 |
| --- | --- |
| `app_id(...)` | 设置稳定应用 ID。系统通知、平台集成和日志身份通常需要它。 |
| `title(...)` | 设置初始窗口标题。可被 `bind_title(...)` 覆盖。 |
| `window_size(width, height)` | 设置主窗口初始大小，使用 `Dp`。 |
| `decorations(false)` | 关闭系统标题栏，用 tgui 自绘窗口 chrome。 |
| `msaa(MsaaMode::...)` | 配置多重采样。 |
| `window_icon(bytes)` | 设置窗口图标。 |
| `font_bytes(...)` / `font_file(...)` | 注册应用字体。 |
| `theme_mode(...)` | 设置初始 light / dark / system。 |
| `theme_set(...)` | 设置 light/dark 主题集合。 |
| `resource_budget(...)` | 调整图片、SVG、阴影等缓存容量。 |
| `with_view_model(...)` | 指定 ViewModel 构造函数。 |
| `root_view(...)` | 指定根组件树函数。 |
| `run()` | 启动事件循环。 |

## 窗口属性绑定

窗口标题、清屏颜色和主题模式可以由 ViewModel 状态驱动：

```rust
struct AppVm {
    title: State<String>,
    theme: State<ThemeMode>,
}

impl AppVm {
    fn title(&self) -> Signal<String> {
        self.title.signal()
    }

    fn clear_color(&self) -> Signal<Color> {
        self.theme.signal().map(|mode| match mode {
            ThemeMode::Dark => Color::hexa(0x0F172AFF),
            _ => Color::hexa(0xF8FAFCFF),
        })
    }

    fn theme_mode(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }
}
```

```rust
Application::new()
    .with_view_model(AppVm::new)
    .bind_title(AppVm::title)
    .bind_clear_color(AppVm::clear_color)
    .bind_theme_mode(AppVm::theme_mode)
    .root_view(AppVm::view)
    .run()
```

`bind_clear_color` 对透明或自绘窗口尤其重要。需要无边框透明窗口时，通常同时设置 `decorations(false)` 和透明 clear color。

## 窗口级输入

`Application::on_input(...)` 注册窗口级快捷键。它适合全局命令，例如保存、重置、打开命令面板。

```rust
use tgui::platform::keyboard::KeyCode;

Application::new()
    .with_view_model(CounterVm::new)
    .on_input(
        InputTrigger::KeyPressed(KeyCode::Space),
        Command::new(CounterVm::increment),
    )
    .on_input(
        InputTrigger::KeyPressed(KeyCode::KeyR),
        Command::new(CounterVm::reset),
    )
    .root_view(CounterVm::view)
    .run()
```

组件内部的点击、输入和选择仍然优先用组件自身事件；窗口级输入只处理跨页面或全局意义的快捷键。

## 多窗口

多窗口通过 `WindowSpec` 描述。ViewModel 提供窗口声明，运行时根据窗口 key、角色和关闭策略管理生命周期。

```rust
Application::new()
    .app_id("com.example.tools")
    .with_view_model(AppVm::new)
    .root_view(AppVm::main_view)
    .windows(AppVm::windows)
    .run()
```

```rust
impl AppVm {
    fn windows(&self) -> Vec<WindowSpec<Self>> {
        vec![
            WindowSpec::child("settings")
                .title("Settings")
                .window_size(dp(520.0), dp(420.0))
                .close_policy(WindowClosePolicy::Close)
                .root_view(AppVm::settings_view),
        ]
    }
}
```

适合使用多窗口的场景：

- 主窗口 + 设置窗口。
- 工具面板、检查器、预览窗口。
- 需要独立尺寸、标题或关闭策略的辅助界面。

`WindowClosePolicy` 用于控制窗口关闭请求。当前公开策略为 `WindowClosePolicy::Close`，即关闭当前原生窗口，并让其余窗口继续运行。

## 窗口控制

命令处理函数可以通过 `CommandContext::window()` 请求运行时窗口操作，包括拖拽、拉伸、最小化、最大化/还原和关闭。

```rust
Button::new("关闭")
    .on_click(Command::new_with_context(|_vm: &mut AppVm, ctx| {
        ctx.window().close();
    }))
```

自绘标题栏通常在标题区域绑定拖拽：

```rust
Flex::horizontal()
    .height(dp(44.0))
    .align(Align::Center)
    .child(Text::new("My App").grow(1.0))
    .child(Button::new("—").on_click(Command::new_with_context(|_, ctx| {
        ctx.window().minimize();
    })))
    .child(Button::new("×").on_click(Command::new_with_context(|_, ctx| {
        ctx.window().close();
    })))
    .on_click(Command::new_with_context(|_vm: &mut AppVm, ctx| {
        ctx.window().drag_window();
    }))
```

> 实际标题栏交互可参考 `examples/frameless_window`，其中同时处理拖拽、拉伸、最小化、最大化和关闭。

## 自定义窗口 Chrome

关闭系统标题栏后，窗口装饰、按钮、拖拽区域和透明背景都由应用负责：

```rust
Application::new()
    .decorations(false)
    .with_view_model(AppVm::new)
    .bind_clear_color(AppVm::clear_color)
    .root_view(AppVm::view)
    .run()
```

自绘窗口通常需要同时考虑：

- 标题栏拖拽区域。
- 窗口边缘 resize 区域。
- 最小化、最大化/还原、关闭按钮。
- 深浅色主题下的文字和按钮对比度。
- 透明窗口的 `clear_color` alpha。

## 资源预算

图片、SVG、Canvas shadow 和 widget shadow 都有缓存。内存敏感应用可以使用紧凑预算：

```rust
Application::new()
    .resource_budget(ResourceBudget::compact())
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

如果界面大量使用网络图片、SVG 图标或阴影，可以在性能测试后按需调大对应字段。更多建议见[性能与资源](/advanced/performance)。

## 平台注意事项

- Windows 通知建议设置稳定 `app_id(...)`，否则通知身份初始化可能失败。
- 透明 / 无边框窗口与平台 compositor 能力有关，建议在目标系统上实测。
- 文件对话框、通知和窗口控制通过运行时服务调度，命令回调里不要在后台线程直接持有或修改 ViewModel。
