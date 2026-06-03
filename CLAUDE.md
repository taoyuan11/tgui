# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`tgui` 是一个基于 `wgpu` 的 Rust GUI crate（MVVM + `taffy` 布局 + 声明式组件树 + 可选音视频）。`AGENTS.md` 已经维护了详尽的中文上下文，本文只补充 Claude Code 高频需要的信息和容易踩的坑。

## 常用命令

```bash
cargo check
cargo test
cargo fmt
cargo test <test_name>           # 单测过滤
cargo test -p tgui <module>::    # 按模块跑
```

按 feature 检查（这些组合都不在默认特性里，改动相关代码后必须显式跑）：

```bash
cargo check --features audio
cargo check --features video
cargo check --features video-static
```

Benchmarks 在 `benches/`，需要 `bench-support` feature：

```bash
cargo bench --features bench-support --bench state_signal
cargo bench --features bench-support --bench widget_core_layout
```

Examples 是独立 Cargo 工程（不在 workspace 里），用 `--manifest-path` 运行：

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

发布走 `publish.bat`（`cargo check` → `cargo test` → `cargo package --allow-dirty` → `cargo publish --allow-dirty`）。

## 架构总览

数据流：`ViewModel` → `Element<VM>` 树 → `WidgetTree` + `taffy` 布局 → `ScenePrimitives` / 命中区域 / IME caret → `runtime` 处理事件、缓存失效、命令派发 → `Renderer` 通过 `wgpu` 提交。

关键模块及职责：

- `src/lib.rs`：唯一公共出口，按 `application` / `mvvm` / `layout` / `widgets` / `canvas` / `theme` / `core` / `media` / `dialog` / `notification` / `audio` / `video` 等子模块分组导出。改公开 API 必须同步检查这里的 re-export。
- `src/application/mod.rs`：`Application`、`WindowSpec`、多窗口、`bind_title` / `bind_clear_color` / `bind_theme_mode`、`decorations(false)`、`on_input`。
- `src/foundation/binding/`：`State<T>` / `Signal<T>` / `TextController` / `ViewModelContext`、依赖跟踪和 invalidation。
- `src/foundation/view_model/`：`ViewModel`、`Command`、`ValueCommand`、`CommandContext`（命令里通过 `ctx.dialogs()` / `ctx.notifications()` / `ctx.window()` / `ctx.log()` 访问运行时服务）。
- `src/foundation/window_control.rs`：拖拽、拖拽改大小、最小化/最大化/还原/关闭，由 runtime 排队 drain。
- `src/runtime/`：事件循环、输入状态、hover/focus/pressed、scene patch（`scene_patch_*.rs` 拆 root/dependency/invalidation/cleanup）、命令派发。**高风险区**，集中了输入/布局/缓存/渲染/平台事件之间的耦合，不要当普通模块改。
- `src/ui/widget/core/`：组件树解析、`taffy` 布局、scene primitive 收集、命中、选择、文本输入基础设施。**另一个高风险区**。修改 layout / render / resolved / scene_layout 时要补单测。
- `src/ui/widget/`：公开 widget builder（`Button` / `Text` / `Input` / `Textarea` / `Image` / `Slider` / `Canvas` …）。新增 widget 优先复用现有 `Element`、`WidgetKind`、`InteractionHandlers`、`MediaEventHandlers`、`VisualStyle`、`LayoutStyle`，不要另起事件/布局系统。
- `src/ui/theme/`：主题 token、`Stateful<T>`、light/dark/system 解析。
- `src/rendering/renderer.rs` + `src/rendering/shader/*.wgsl`：`wgpu` pipeline（圆角矩形、渐变、mesh、文字、纹理、透明 surface、backdrop blur）。
- `src/media/`：图片、SVG 栅格化、URL/本地/内存加载、纹理缓存、shadow 缓存。网络层用 `reqwest` blocking + rustls ring。
- `src/notification/`、`src/dialog/`、`src/audio/`、`src/video/`、`src/platform/`：对应章节的运行时服务和平台后端。

## 启动模型

仅支持 MVVM 启动路径，即使是静态 UI 也要显式定义 ViewModel：

```rust
Application::new()
    .app_id("com.example.demo")
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

- `ViewModel: Send + 'static`
- `State::set/update` 标记 invalidation 并唤醒事件循环
- `Signal<T>` 惰性派生，可 `map`，可 `.animated(transition)` 走 `AnimationEngine`
- 公开属性大多接受 `impl Into<Value<T>>`，新增绑定属性要保持这个签名以便静态值和 `Signal` 都能传

## 容易踩的坑

- **Windows + `video` feature**：`build.rs` 会额外链接 `strmiids` 和 `mfuuid`；本机要装好 FFmpeg 链接环境，否则 `cargo check --features video` 也过不了。
- **Windows 通知**：必须 `Application::app_id(...)`，否则通知身份初始化失败。
- **透明 / 无边框窗口**：`decorations(false)` 通常要配 `clear_color(Color::TRANSPARENT)`；renderer 根据 clear color alpha 选 surface alpha mode（Windows 优先 DX12 / DXGI visual swapchain）。
- **公共 API 变更**：同步改 `src/lib.rs` 的 re-export、`README.md`、相关示例。`Cargo.toml` 的 `exclude` 把 `examples/*`、`*.png`、`.github/*`、`AGENTS.md`、`skills/*` 排除出 crate，发布前要确认资源不会进 crate。
- **平台代码**：桌面平台路径要 `cfg` 严格隔离；新增平台能力优先走 `src/platform/` 的后端抽象。
- **文本输入**：改 `Input` / `Textarea` / IME / 选择 / 滚动 时，要把 `TextController`、`src/ui/widget/common.rs`、`src/ui/widget/core/`、`src/runtime/input/` 当成同一套基础设施一起改，不要只动一处。注意 UTF-8 边界、IME composition、caret 可见性、横向滚动。
- **未跟踪文件 `Video.md`**：根目录这个文件是工作中的笔记，不要在没有明确请求时删除/重命名/覆盖。

## 测试分布

测试分散在源码内 `mod tests` 和模块级 `tests/` 目录里，没有顶层 `tests/`。重点位置：

- `src/runtime/tests/`、`src/runtime/tests.rs`：事件、焦点、文本输入、滚动、命令、canvas/video 命中、缓存生命周期。
- `src/ui/widget/core/tests/`、`src/ui/widget/core/tests.rs`：布局、render primitive、命中、选择、状态。
- `src/animation/`、`src/media/`、`src/notification/`、`src/audio/`、`src/video/`、`src/text/font/`、`src/ui/theme/`、`src/ui/widget/canvas/`、`src/ui/widget/common.rs` 各自有局部测试。

修改运行时、widget core、渲染 primitive、文本输入、媒体加载、通知、窗口控制时优先补单测；不要只跑示例当验证。

## 进一步参考

- `AGENTS.md`：完整中文上下文，包含模块逐项说明、组件 API 链式约定、动画两套体系（声明式过渡 vs 时间线）、媒体/通知/音视频/平台细节、维护注意事项和推荐阅读顺序。
- `README.md`：面向使用者的公开 API 介绍、示例和 quick start。
- `docs/canvas.md`：Canvas / retained scene 详解。
