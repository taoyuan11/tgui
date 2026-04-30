
<p align="center">
  <img src="./docs/images/tgui_logo.png" width="150px" alt="logo">
</p>

`tgui` 是一个基于 `wgpu` 的 Rust GUI 框架，强调这几件事：

- GPU 加速渲染
- 轻量 MVVM 状态模型
- 基于 `taffy` 的布局系统
- 声明式组件树 + 可绑定窗口属性
- 内置动画、图片/文本、对话框、画布、自定义窗口 chrome 和可选视频能力

适合做桌面 GUI、工具型应用、可视化面板，以及需要较强自定义绘制能力的界面。

## 当前能力概览

### 应用与窗口

- `Application`：应用入口，配置标题、窗口大小、主题、字体、图标
- `WindowSpec`：声明式多窗口描述
- `bind_title` / `bind_clear_color` / `bind_theme_mode`：将窗口属性绑定到状态
- `decorations(false)`：关闭系统标题栏，用 tgui 自绘窗口 chrome
- `on_input`：注册窗口级快捷键/输入触发

### 状态与 MVVM

- `ViewModelContext`：创建响应式状态与动画句柄
- `Observable<T>`：可变状态，更新后自动触发重绘
- `Binding<T>`：从状态派生 UI 值，支持 `map` 和 `animated`
- `Command<T>` / `ValueCommand<T, V>`：把按钮、输入、画布事件接回 ViewModel
- `CommandContext::window()`：在命令中请求窗口拖拽、拉伸、最小化、最大化/还原、关闭

### 布局与组件

- 布局：`Stack`、`Grid`、`Flex`
- 基础组件：`Text`、`Button`、`Input`、`Radio`、`Checkbox`、`Select`、`Image`
- 画布：`Canvas`、`CanvasPath`、`PathBuilder`、渐变/阴影/布尔运算
- 视频：`VideoSurface`、`VideoController`、`VideoSource`（需启用 `video` feature）

### 样式与基础类型

- 主题：`Theme`、`ThemeMode`、`ThemeSet`
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
- 窗口控制：`WindowControl`、`WindowResizeDirection`
- 日志：`Log`、`tgui_log`
- 平台导出：`platform::*`

## 安装

```toml
[dependencies]
tgui = "0.1.6"
```

如果需要视频能力：

```toml
[dependencies]
tgui = { version = "0.1.6", features = ["video"] }
```

可选 feature：

- `video`：启用 FFmpeg 视频播放能力
- `video-static`：启用静态链接 FFmpeg 的视频能力
- `android`：启用 Android 入口
- `ohos`：启用 HarmonyOS / OpenHarmony 入口

## 公开 API 结构

`tgui` 的公开类型按职责分类导出：

- `application`：应用、窗口和运行入口
- `mvvm`：`ViewModel`、`Observable`、`Binding`、`Command`、`CommandContext`、`WindowControl`
- `layout`：布局容器、尺寸、间距和滚动相关类型
- `widgets` / `canvas`：基础控件、控件树和 Canvas 绘制 API
- `theme`：主题、色板、排版、状态和设计 token
- `core`：颜色、错误、输入触发器、基础单位和几何类型
- `media` / `dialog` / `logging` / `platform` / `video`：媒体、对话框、日志、平台和视频能力

示例代码可使用 `tgui::prelude::*` 引入常用 API；库代码建议优先从具体分类模块导入。

## 快速开始

`tgui` 只支持 MVVM 启动路径。即使是静态界面，也需要定义一个命名 ViewModel 并显式实现 `ViewModel`。

```rust
use tgui::prelude::*;

struct CounterVm {
    count: Observable<u32>,
}

impl CounterVm {
    fn increment(&mut self) {
        self.count.update(|value| *value += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .child(Text::new(
                self.count.binding().map(|count| format!("Count: {count}")),
            ))
            .child(
                Button::new(Text::new("Increment"))
                    .on_click(Command::new(Self::increment)),
            )
            .into()
    }
}

impl ViewModel for CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.observable(0),
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
Observable<T>
Binding<T>
Command<T>
ValueCommand<T, V>
CommandContext<T>
WindowControl
WindowResizeDirection

Stack / Grid / Flex
Text / Button / Input / Image / Canvas

Theme / ThemeMode / ThemeSet / Color
dp / sp / Dp / Sp

Transition
AnimatedValue<T>
AnimationSpec<T>
Keyframes<T>
```

## 仓库示例

仓库内示例基本覆盖了当前主要能力：

- `basic_window`：命名空 ViewModel 驱动的最小完整窗口
- `mvvm_counter`：响应式状态、标题绑定、清屏色绑定、快捷键输入
- `animation_showcase`：`Binding::animated` 声明式过渡
- `timeline_controller`：时间线动画控制器
- `multi_window`：共享 ViewModel 的多窗口
- `dialogs`：同步/异步文件选择与消息框
- `canvas`：路径绘制、渐变、阴影、布尔运算、命中事件
- `background_effects`：通用渐变背景和 backdrop blur
- `frameless_window`：关闭系统装饰后的自绘标题栏、拖拽、拉伸和窗口按钮
- `demo`：综合展示常用布局、组件和样式
- `multiple_vm_examples`：多页面 / 多 ViewModel 示例
- `android_basic_window`：Android 入口示例
- `ohos_basic_window`：OpenHarmony / HarmonyOS 入口示例

这些示例是独立小工程，运行方式如下：

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

README 中的示例名称以当前 `examples/` 目录为准；如果新增或删除示例，应同步更新本节和 `examples/README.md`。

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

`Canvas` 适合做自定义图形与交互式绘制，目前公开能力包括：

- `PathBuilder`
- `CanvasPath`
- `CanvasStroke`
- `CanvasLinearGradient`
- `CanvasRadialGradient`
- `CanvasShadow`
- `CanvasBooleanOp`
- `CanvasPointerEvent`

### 通用背景

除 `Canvas` 外，常规控件背景现在也支持更丰富的视觉能力：

- `BackgroundBrush`
- `BackgroundLinearGradient`
- `BackgroundRadialGradient`
- `BackgroundGradientStop`
- `background_brush(...)`
- `background_blur(...)`

`background_blur(...)` 是应用窗口内容上的 backdrop blur，可用于玻璃卡片、磨砂面板和层叠浮层。

### 视频

启用 `video` feature 后可使用：

- `video::VideoController`
- `video::VideoSurface`
- `video::VideoSource`
- `video::PlaybackState`
- `video::VideoMetrics`

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

桌面端当前包含 Windows、macOS、Linux 相关实现；同时提供：

- `run_android` / `android` feature
- `run_ohos` / `ohos` feature

多窗口通过 `WindowSpec` 描述，主窗口与子窗口共享同一个 ViewModel，适合做文档窗口、检查器窗口、浮动工具面板等场景。

`Application::decorations(false)` 或 `WindowSpec::decorations(false)` 可以关闭系统标题栏，用普通 tgui 组件自绘窗口 chrome。命令处理里可以通过 `ctx.window()` 操作当前窗口：

```rust
Button::new(Text::new("Close"))
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

通过 `Command::new_with_context` 或 `ValueCommand::new_with_context`，可以在命令处理中访问运行时服务：

- `ctx.dialogs()`：文件选择、消息框
- `ctx.window()`：当前窗口控制
- `ctx.log()`：运行时日志

相关类型：

- `Dialogs`
- `FileDialogOptions`
- `MessageDialogOptions`
- `MessageDialogButtons`
- `MessageDialogResult`

## 适合先看哪些文件

- `src/lib.rs`：crate 导出总览
- `src/application/mod.rs`：应用与窗口入口
- `src/foundation/binding.rs`：`Observable` / `Binding`
- `src/foundation/view_model.rs`：`Command` / `ValueCommand`
- `src/foundation/window_control.rs`：`WindowControl` / `WindowResizeDirection`
- `src/ui/widget/*`：组件与布局实现
- `examples/frameless_window/src/main.rs`：无边框窗口和窗口控制参考
- `examples/*`：最直接的上手参考

## License

MIT
