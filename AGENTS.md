# AGENTS.md

本文记录对当前 `tgui` 项目的快速理解，供后续代理或维护者进入仓库时参考。内容基于当前工作区源码、`Cargo.toml`、README 与示例目录整理。

## 项目定位

`tgui` 是一个 Rust GUI 框架 crate，目标是提供基于 `wgpu` 的 GPU 加速渲染、MVVM 状态模型、`taffy` 布局、声明式组件树、主题系统、动画、媒体加载、对话框、自定义窗口 chrome / 原生窗口控制以及可选视频播放能力。

crate 信息：

- 包名：`tgui`
- 当前版本：`0.1.6`
- Rust edition：`2021`
- License：MIT
- 主要依赖：`wgpu`、`winit-core` 及平台后端、`taffy`、`cosmic-text`、`image`、`resvg`、`reqwest`、`lyon`
- 可选视频依赖：`ffmpeg-next`

## 重要目录和文件

- `Cargo.toml`：crate 元数据、features、平台依赖和发布排除规则。
- `src/lib.rs`：公共 API 总出口，按 `application`、`mvvm`、`layout`、`widgets`、`canvas`、`theme`、`core`、`media`、`dialog`、`logging`、`platform`、`video` 分组导出。
- `src/application/mod.rs`：`Application`、`ApplicationBuilder`、`WindowSpec`、多窗口声明、窗口装饰开关与运行入口。
- `src/runtime.rs`：运行时核心，管理事件循环、窗口生命周期、输入、焦点、滚动、文本编辑、命令派发、窗口控制请求、异步对话框回调、主题绑定、动画刷新、媒体状态和渲染调度。
- `src/foundation/`：基础能力，包括 `Observable`、`Binding`、`ViewModelContext`、`Command`、`ValueCommand`、`WindowControl`、`InputTrigger`、`TguiError`、`Color`。
- `src/foundation/window_control.rs`：命令上下文中的运行时窗口控制，封装拖拽、拖拽调整大小、最小化、最大化、还原、关闭和最大化状态查询。
- `src/ui/layout.rs`：布局基础类型，封装 `Length`、`Track`、`Insets`、`Align`、`Justify`、`Axis`、`Overflow` 等。
- `src/ui/widget/`：组件和场景构建。`core.rs` 很大，负责元素树解析、Taffy 布局、渲染 primitive 收集、命中区域、输入/选择文本等大量逻辑；其他文件提供具体 widget builder。
- `src/ui/theme/`：主题 token、组件主题、状态解析、light/dark/system 模式。
- `src/rendering/renderer.rs`：`wgpu` 渲染器，包含矩形、渐变/brush、mesh、文字、纹理、透明窗口 surface、backdrop blur 等 pipeline。
- `src/rendering/shader/`：WGSL shader。
- `src/media/mod.rs`：图片、SVG、网络/本地/内存媒体加载，纹理缓存，SVG 栅格化，canvas shadow 缓存。
- `src/dialog.rs`：同步和异步原生对话框封装；桌面用 `rfd`，Android/OHOS 返回 unsupported。
- `src/platform.rs`：平台抽象和不同 winit 后端的选择。
- `src/video/`：启用 `video` feature 后的 `VideoController`、`VideoSurface`、FFmpeg 后端。
- `examples/`：独立 Cargo 示例工程。
- `docs/images/tgui_logo.png`：README 使用的 logo。

## Features 和平台

`Cargo.toml` 中的 features：

- `default = []`
- `android`：启用 Android 入口和 `winit-android`。
- `ohos`：启用 HarmonyOS / OpenHarmony 入口和 `tgui-winit-ohos`。
- `video`：启用 `ffmpeg-next` 视频能力。
- `video-static`：在 `video` 基础上启用 `ffmpeg-next/static`。

平台依赖按 target 区分：

- Windows、macOS、Linux：桌面窗口、剪贴板、对话框、音频相关依赖。
- Android：`jni`、`winit-android`。
- OHOS：`hilog-sys`、`tgui-winit-ohos`。

Windows 下启用 `video` feature 时，`build.rs` 会额外链接 `strmiids` 和 `mfuuid`。

## 公共 API 组织

优先从 `src/lib.rs` 理解对外 API：

- `prelude`：示例和小应用常用的一站式导入。
- `application`：`Application`、`WindowSpec`、`WindowRole`、`WindowClosePolicy`。
- `mvvm`：`ViewModel`、`ViewModelContext`、`Observable`、`Binding`、`Command`、`ValueCommand`、`CommandContext`、`WindowControl`、`WindowResizeDirection`。
- `layout`：`Flex`、`Grid`、`Stack` 以及布局尺寸和对齐类型。
- `widgets`：`Button`、`Text`、`Input`、`Image`、`Checkbox`、`Radio`、`Select`、`Switch`、`Element`、`WidgetTree` 等。
- `canvas`：`Canvas`、`PathBuilder`、路径、渐变、阴影、布尔运算、画布事件。
- `theme`：`Theme`、`ThemeMode`、`ThemeSet`、组件主题和设计 token。
- `media`：`MediaSource`、`MediaBytes`、`ContentFit`。
- `dialog`：文件选择和消息框。
- `video`：仅在 `video` feature 下导出。

## 应用启动模型

项目当前是 MVVM-only 启动路径。典型流程：

```rust
Application::new()
    .title("demo")
    .window_size(dp(960.0), dp(640.0))
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

注意点：

- ViewModel 类型需要实现 `ViewModel: Send + 'static`。
- `ViewModelContext` 用来创建 `Observable<T>`、`AnimatedValue<T>` 和 timeline controller。
- `Observable::set/update` 会标记 invalidation 并唤醒事件循环。
- `Binding<T>` 是惰性读取值，可 `map` 派生，也可 `.animated(...)` 给支持的属性添加声明式过渡。
- `Command<T>` 处理无 payload 事件；`ValueCommand<T, V>` 处理带 payload 事件。
- `Command::new_with_context` / `ValueCommand::new_with_context` 可访问运行时服务，例如 `ctx.dialogs()` 和 `ctx.log()`。
- `CommandContext::window()` 返回当前命令所属窗口的 `WindowControl`，可请求原生窗口拖拽、拖拽调整大小、最小化、最大化 / 还原、关闭以及查询最大化状态。
- `Application::decorations(false)` 和 `WindowSpec::decorations(false)` 可关闭系统标题栏，用于自绘窗口 chrome；无边框透明窗口通常还需要 `clear_color(Color::TRANSPARENT)`。

## 渲染和布局流程

整体流程可以理解为：

1. ViewModel 构建 `Element<VM>` 组件树。
2. `WidgetTree` 解析组件树并用 `taffy` 计算布局。
3. 组件树生成 `ScenePrimitives`、命中区域、滚动区域、IME/caret 信息等。
4. `runtime.rs` 处理窗口事件、输入状态、hover/focus/pressed 状态、命令派发与缓存失效。
5. `Renderer` 把 scene primitives 提交到 `wgpu` pipeline。

重要渲染能力：

- 普通矩形和圆角矩形。
- 线性/径向渐变和 brush。
- mesh 绘制。
- 文字渲染，基于 `cosmic-text`。
- 图片/纹理绘制。
- 透明窗口 surface，根据 clear color alpha 选择 composite alpha mode；Windows 透明窗口优先走 DX12 / DXGI visual swapchain。
- backdrop blur 需要离屏 target 和 composite pipeline。
- Canvas 绘制通过路径/mesh/缓存纹理等 primitive 落到渲染器。

## 组件和样式约定

组件 builder 大多支持统一的链式 API：

- 尺寸：`size`、`width`、`height`、`min_*`、`max_*`、`aspect_ratio`
- 布局：`margin`、`padding`、`grow`、`shrink`、`basis`、`align_self`、`justify_self`、grid row/column
- 绝对定位：`position_absolute`、`left`、`top`、`right`、`bottom`、`inset`
- 视觉：`background`、`background_brush`、`background_image`、`background_blur`、`border`、`border_radius`、`opacity`、`offset`
- 交互：`on_click`、`on_double_click`、`on_focus`、`on_blur`、`on_mouse_enter`、`on_mouse_leave`、`on_mouse_move`
- 状态：很多属性接受 `Value<T>`，因此可以传静态值或 `Binding<T>`。

如果新增 widget，优先复用现有 `Element`、`WidgetKind`、`InteractionHandlers`、`MediaEventHandlers`、`VisualStyle`、`LayoutStyle` 模式，而不是另起一套事件或布局系统。

## 动画系统

动画分两类：

- 声明式属性过渡：`Binding::animated(Transition)`，由 `AnimationEngine` 按 `AnimationKey` 和属性类型解析。
- 时间线动画：`ViewModelContext::animated_value` + `ctx.timeline()` + `AnimationSpec` / `Keyframes` / `Playback`，返回 `AnimationControllerHandle`。

当前可插值类型包括 `Color`、`f32`、`Dp`、`Sp`、`Point`、`Insets` 等。主题变化有默认过渡。

## 媒体系统

`MediaSource` 支持：

- 本地路径
- URL
- 内存 bytes

图片支持 raster 格式和 SVG：

- SVG 可按目标尺寸栅格化，并有小型缓存。
- raster 图片会按物理像素请求异步栅格化，保留旧纹理作为加载中的 fallback。
- 媒体加载失败会产生 placeholder 颜色和标签。
- 网络加载使用 `reqwest` blocking client 和 rustls ring provider。

## 视频系统

视频能力位于 `src/video/`，需要 `video` feature：

- `VideoController` 管理播放、暂停、seek、音量、静音、buffer memory limit 等。
- `VideoSurface` 作为 widget 参与布局、渲染和命中测试。
- FFmpeg 后端位于 `src/video/backend/ffmpeg/`，包含 decode、audio、present 等模块。

涉及视频变更时建议至少运行带 feature 的检查或测试，但本机需要具备对应 FFmpeg/link 环境。

## 示例工程

当前 `examples/` 中存在这些独立示例：

- `basic_window`
- `mvvm_counter`
- `animation_showcase`
- `timeline_controller`
- `multi_window`
- `dialogs`
- `canvas`
- `background_effects`
- `frameless_window`
- `demo`
- `multiple_vm_examples`
- `android_basic_window`
- `ohos_basic_window`

运行桌面示例：

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

README 中提到的一些示例名称未必都在当前工作区存在；维护文档时以实际 `examples/` 目录为准。

## 常用开发命令

```bash
cargo check
cargo test
cargo fmt
```

按 feature 检查：

```bash
cargo check --features video
cargo check --features video-static
cargo check --features android
cargo check --features ohos
```

运行某个测试：

```bash
cargo test <test_name>
```

运行某个示例：

```bash
cargo run --manifest-path examples/<example_name>/Cargo.toml
```

## 测试分布

仓库中单元测试主要集中在：

- `src/ui/widget/core.rs`：布局、渲染 primitive、输入、选择、滚动、组件状态。
- `src/runtime.rs`：事件、焦点、输入编辑、滚动条、命令派发、canvas/video 命中等运行时行为。
- `src/application/mod.rs`、`src/foundation/window_control.rs`：窗口配置、装饰开关、命令上下文窗口控制。
- `src/media/mod.rs`：图片/SVG 加载、栅格化、缓存、外部资源解析。
- `src/animation.rs`：属性动画和 timeline 行为。
- `src/video/backend/ffmpeg/*`：视频后端内部逻辑，需 feature/环境支持。
- `src/ui/widget/canvas.rs`、`src/ui/widget/common.rs`、`src/ui/theme/mod.rs`、`src/text/font.rs` 等也有局部测试。

修改共享行为时，不要只跑示例；至少跑相关模块测试。修改 `runtime.rs`、`ui/widget/core.rs` 或渲染 primitive 时，优先补充小型单元测试。

## 维护注意事项

- 不要把 `src/runtime.rs` 或 `src/ui/widget/core.rs` 当作普通小文件随意大改；它们是行为集中区，牵涉输入、布局、缓存、渲染、命令和平台事件。
- 公共 API 变更要同步检查 `src/lib.rs` 的 re-export、README、示例和文档。
- `Cargo.toml` 的 `exclude` 会排除 `examples/*`、`assets/*`、`*.png`、`*.ttf` 等，发布相关变更时要留意资源是否会进入 crate。
- 新增平台能力时优先走 `platform.rs` 的后端抽象，并用 `cfg` 控制依赖和代码路径。
- 新增窗口控制能力时同步检查 `ApplicationConfig` / `WindowSpec`、`CommandContext`、`WindowControl` 请求队列、runtime drain 逻辑、多窗口关闭策略和平台窗口 API。
- 透明 / 无边框窗口相关改动要同时关注 `Application::decorations`、clear color alpha、surface alpha mode、平台后端选择和示例表现。
- 新增视觉属性时通常需要同时考虑：widget builder、`VisualStyle` 或相关状态、scene primitive、动画 key、renderer/shader。
- 新增绑定属性时优先接受 `impl Into<Value<T>>`，这样静态值和 `Binding<T>` 都可用。
- 新增交互事件时要检查 hover/focus/pressed 状态、命中区域、命令 scope、运行时事件派发以及缓存失效。
- 文本相关修改要注意 UTF-8 边界、IME composition、选择区间、caret 可见性和横向滚动。
- 媒体和异步加载修改要确保完成后调用 invalidation，避免 UI 不刷新。
- 对话框异步回调通过 runtime dispatcher 回到 ViewModel；不要在线程里直接持有或修改 ViewModel。
- 当前工作区存在未跟踪的 `Video.md`，不要在无明确需求时删除、重命名或覆盖它。

## 推荐阅读顺序

第一次接手时建议按这个顺序读：

1. `README.md`
2. `src/lib.rs`
3. `examples/mvvm_counter/src/main.rs`
4. `src/application/mod.rs`
5. `src/foundation/binding.rs`
6. `src/foundation/view_model.rs`
7. 涉及窗口控制时读 `src/foundation/window_control.rs` 和 `examples/frameless_window/src/main.rs`
8. `src/ui/widget/core.rs` 中的 `Element`、`WidgetTree`、布局和渲染输出相关部分
9. `src/runtime.rs` 的 `BoundRuntime`、`BoundRuntimeHandler` 和事件处理部分
10. 需要改渲染时再读 `src/rendering/renderer.rs` 与 `src/rendering/shader/*`
