# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`tgui` 是一个基于 `wgpu` 的 Rust GUI crate（MVVM + `taffy` 布局 + 声明式组件树 + 可选音视频）。`AGENTS.md` 已经维护了详尽的中文上下文，本文只补充 Claude Code 高频需要的信息和容易踩的坑。

## 常用命令

```bash
cargo check -p tgui
cargo test -p tgui-runtime --lib -- --test-threads=1
cargo fmt
cargo test -p tgui-runtime <test_name>   # 单测过滤
cargo test -p tgui-runtime <module>::    # 按模块跑
```

按 feature 检查（这些组合都不在默认特性里，改动相关代码后必须显式跑）：

```bash
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
```

细粒度响应式渲染管线默认内置（见下方「细粒度响应式渲染管线」与根目录 `FINE_GRAINED_ROADMAP.md`）。改失效/场景拼接/顶点上传/滚动快路径时，至少跑默认、无默认 feature、音视频组合：

```bash
cargo check -p tgui
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
```

Benchmarks 在 workspace package `tgui-benchmarks`（目录 `benches/`），需要 `bench-support` feature：

```bash
cargo bench -p tgui-benchmarks --features bench-support --bench state_signal
cargo bench -p tgui-benchmarks --features bench-support --bench widget_core_layout
```

Examples 是 workspace member，也可以继续用 `--manifest-path` 运行：

```bash
cargo run -p basic_window
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

发布走 `publish.bat`（`cargo check` → `cargo test` → `cargo package --allow-dirty` → `cargo publish --allow-dirty`）。

## 架构总览

数据流：`ViewModel` → `Element<VM>` 树 → `WidgetTree` + `taffy` 布局 → `ScenePrimitives` / 命中区域 / IME caret → `runtime` 处理事件、缓存失效、命令派发 → `Renderer` 通过 `wgpu` 提交。

关键模块及职责：

- `Cargo.toml`：公开 facade package + workspace 配置；`default-members = ["."]`，所以根 `cargo check` 只检查 `tgui`。
- `src/lib.rs`：公开 `tgui` facade，按 `application` / `mvvm` / `layout` / `widgets` / `canvas` / `theme` / `core` / `media` / `dialog` / `notification` / `audio` / `video` 等子模块转发。改公开 API 必须同步检查这里的 re-export 和相关边界 crate。
- `crates/tgui-runtime/src/lib.rs`：主要实现 crate 的导出总览。
- `crates/tgui-runtime/src/application/mod.rs`：`Application`、`WindowSpec`、多窗口、`bind_title` / `bind_clear_color` / `bind_theme_mode`、`decorations(false)`、`on_input`。
- `crates/tgui-runtime/src/foundation/binding/`：`State<T>` / `Signal<T>` / `TextController` / `ViewModelContext`、依赖跟踪和 invalidation。
- `crates/tgui-runtime/src/foundation/view_model/`：`ViewModel`、`Command`、`ValueCommand`、`CommandContext`（命令里通过 `ctx.dialogs()` / `ctx.notifications()` / `ctx.window()` / `ctx.log()` 访问运行时服务）。
- `crates/tgui-runtime/src/foundation/window_control.rs`：拖拽、拖拽改大小、最小化/最大化/还原/关闭，由 runtime 排队 drain。
- `crates/tgui-runtime/src/runtime/`：事件循环、输入状态、hover/focus/pressed、scene patch（`scene_patch_*.rs` 拆 root/dependency/invalidation/cleanup）、命令派发。**高风险区**，集中了输入/布局/缓存/渲染/平台事件之间的耦合，不要当普通模块改。
- `crates/tgui-runtime/src/ui/widget/core/`：组件树解析、`taffy` 布局、scene primitive 收集、命中、选择、文本输入基础设施。**另一个高风险区**。修改 layout / render / resolved / scene_layout 时要补单测。
- `crates/tgui-runtime/src/ui/widget/`：公开 widget builder（`Button` / `Text` / `Input` / `Textarea` / `Image` / `Slider` / `Canvas` …）。新增 widget 优先复用现有 `Element`、`WidgetKind`、`InteractionHandlers`、`MediaEventHandlers`、`VisualStyle`、`LayoutStyle`，不要另起事件/布局系统。
- `crates/tgui-runtime/src/ui/theme/`：主题 token、`Stateful<T>`、light/dark/system 解析。
- `crates/tgui-runtime/src/rendering/renderer.rs` + `crates/tgui-runtime/src/rendering/shader/*.wgsl`：`wgpu` pipeline（圆角矩形、渐变、mesh、文字、纹理、透明 surface、backdrop blur）。
- `crates/tgui-runtime/src/media/`：图片、SVG 栅格化、URL/本地/内存加载、纹理缓存、shadow 缓存。网络层用 `reqwest` blocking + rustls ring。
- `crates/tgui-runtime/src/notification/`、`dialog/`、`audio/`、`video/`、`platform/`：对应章节的运行时服务和平台后端。
- `crates/tgui-core`、`tgui-platform`、`tgui-log`、`tgui-mvvm`、`tgui-media`、`tgui-ui`、`tgui-rendering`：内部边界 crate，当前主要从 runtime re-export 对应 API 面。

## 细粒度响应式渲染管线（高风险区）

失效/渲染管线在「细粒度依赖跟踪 + 保留式分块场景图」之上加了一组**分级降级**的内置快路径（路线图：根目录 `FINE_GRAINED_ROADMAP.md`，不进 crate）。三条红线：**保留 pull 模型缩短半径**、**任一快路径前置不满足必须干净回退**到子树 patch → 整帧重收集（绝不渲染错误）、**正确性优先**（每条快路径都有「与全量重收集逐项等价」+「回退路径」两类单测）。

- 场景命令原地拼接：叶子 scene-only 改动时把新 chunk 原地 splice 进各祖先 chunk 稳定区间，跳过祖先链 recompose。必须同时覆盖主渲染流（含并行数组）+ `hit_regions` + `scroll_regions`（每个 Container 无条件 push 一条 ScrollRegion）。z-order 是正确性红线。锚点：`scene_primitives.rs`、`hit_scene_state.rs`、`scene_layout.rs`、`runtime/scene_patch.rs`。
- 属性级依赖归因（`PropertySlot`）：**兜底**：失效消费侧只读 `widget_id + phase`，未识别属性退化为整 widget 失效 → 绝不漏更新。锚点：`foundation/binding/dependency.rs`、`resolved/collect/chrome/visual_state.rs`、`render/text.rs`。
- 顶点脏区间增量上传：顶点池 flush 按字节 diff 只上传脏区间。triple-buffer 保证部分覆盖安全。锚点：`rendering/renderer/vertex_pool.rs`。
- 纯滚动快路径：优先用 GPU per-draw 平移；adapter 不支持 IMMEDIATES 或场景前置不满足时回退到 CPU 子树重收集，再失败则整帧重收集。锚点：`runtime/mod.rs`、`input/interaction.rs`、`scene_runtime.rs`、`renderer/prepare.rs`。

> 改这条管线务必跑 `cargo check -p tgui` / `--no-default-features` / `audio` / `video` / `video-static` 检查，并补两类单测。

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

- **Windows + `video` feature**：`crates/tgui-runtime/build.rs` 会额外链接 `strmiids` 和 `mfuuid`；本机要装好 FFmpeg 链接环境，否则 `cargo check -p tgui --features video` 也过不了。
- **Windows 通知**：必须 `Application::app_id(...)`，否则通知身份初始化失败。
- **透明 / 无边框窗口**：`decorations(false)` 通常要配 `clear_color(Color::TRANSPARENT)`；renderer 根据 clear color alpha 选 surface alpha mode（Windows 优先 DX12 / DXGI visual swapchain）。
- **公共 API 变更**：同步改根 `src/lib.rs` facade、相关边界 crate re-export、`README.md`、相关示例。根 `Cargo.toml` 的 `exclude` 把 `examples/*`、`docs/*`、`crates/*`、`benches/*`、`*.png`、`.github/*`、`AGENTS.md`、`CLAUDE.md`、`FINE_GRAINED_ROADMAP.md`、`skills/*` 排除出公开 facade crate，发布前用 `cargo package -p <crate> --allow-dirty --list` 确认资源/文档不会进错包。
- **平台代码**：桌面平台路径要 `cfg` 严格隔离；新增平台能力优先走 `crates/tgui-runtime/src/platform/` 的后端抽象。
- **文本输入**：改 `Input` / `Textarea` / IME / 选择 / 滚动 时，要把 `TextController`、`crates/tgui-runtime/src/ui/widget/common.rs`、`crates/tgui-runtime/src/ui/widget/core/`、`crates/tgui-runtime/src/runtime/input/` 当成同一套基础设施一起改，不要只动一处。注意 UTF-8 边界、IME composition、caret 可见性、横向滚动。
- **未跟踪文件 `Video.md`**：根目录这个文件是工作中的笔记，不要在没有明确请求时删除/重命名/覆盖。

## 测试分布

测试分散在源码内 `mod tests` 和模块级 `tests/` 目录里，没有顶层 `tests/`。重点位置：

- `crates/tgui-runtime/src/runtime/tests/`、`crates/tgui-runtime/src/runtime/tests.rs`：事件、焦点、文本输入、滚动、命令、canvas/video 命中、缓存生命周期。
- `crates/tgui-runtime/src/ui/widget/core/tests/`、`crates/tgui-runtime/src/ui/widget/core/tests.rs`：布局、render primitive、命中、选择、状态。
- `crates/tgui-runtime/src/animation/`、`media/`、`notification/`、`audio/`、`video/`、`text/font/`、`ui/theme/`、`ui/widget/canvas/`、`ui/widget/common.rs` 各自有局部测试。

修改运行时、widget core、渲染 primitive、文本输入、媒体加载、通知、窗口控制时优先补单测；不要只跑示例当验证。

## 进一步参考

- `AGENTS.md`：完整中文上下文，包含模块逐项说明、组件 API 链式约定、动画两套体系（声明式过渡 vs 时间线）、媒体/通知/音视频/平台细节、维护注意事项和推荐阅读顺序。
- `README.md`：面向使用者的公开 API 介绍、示例和 quick start。
- `docs/features/canvas.md`：Canvas / retained scene 详解。
- `docs/advanced/performance.md`：性能观察点、ResourceBudget、细粒度增量渲染管线说明。
