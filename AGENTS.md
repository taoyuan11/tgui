# AGENTS.md

本文记录对当前 `tgui` 项目的快速理解，供后续代理或维护者进入仓库时参考。内容基于当前工作区源码、`Cargo.toml`、README 与示例目录整理。

**UI 工作必读：后续新增或修改组件外观、交互状态、主题及示例界面时，必须先阅读并遵循本文「UI 设计规范」。该规范是项目默认设计基线，组件实现与评审均以此为依据。**

## 项目定位

`tgui` 是一个 Rust GUI 框架 workspace，公开 crate `tgui` 继续作为兼容门面，目标是提供基于 `wgpu` 的 GPU 加速渲染、MVVM 状态模型、`taffy` 布局、声明式组件树、主题系统、动画、媒体加载、系统通知、对话框、自定义窗口 chrome / 原生窗口控制，以及可选音频/视频播放能力。

workspace / crate 信息：

- 包名：`tgui`
- 当前版本：`0.2.0`
- Rust edition：`2021`
- Rust version（MSRV）：`1.89`；CI 固定使用 `1.89.0` 检查。
- License：MIT OR Apache-2.0
- 主要依赖：`wgpu`、`winit-core` 及平台后端、`taffy`、`cosmic-text`、`image`、`resvg`、`reqwest`、`lyon`
- 可选音视频依赖：`ffmpeg-next`

## 重要目录和文件

- `Cargo.toml`：公开 facade package、workspace 成员、共享依赖、root feature 转发和发布排除规则；`default-members = ["."]`，所以根 `cargo check` 默认只检查 `tgui`。
- `src/lib.rs`：公开 API 兼容门面，按 `application`、`mvvm`、`layout`、`widgets`、`canvas`、`theme`、`core`、`media`、`dialog`、`notification`、`logging`、`platform`、`video` 分组 re-export。
- `crates/tgui-runtime/`：主要实现 crate，承载 application、runtime、UI、rendering、media、platform、audio/video 等实现。
- `crates/tgui-runtime/src/lib.rs`：实现 crate 总出口和公开分组。
- `crates/tgui-runtime/src/application/mod.rs`：`Application`、`WindowSpec`、多窗口声明、窗口装饰开关、窗口绑定和运行入口。
- `crates/tgui-runtime/src/foundation/binding/mod.rs`：`ViewModelContext`、`State`、`Signal`、`TextController`、依赖跟踪与 invalidation。
- `crates/tgui-runtime/src/foundation/view_model/mod.rs`：`ViewModel`、`Command`、`ValueCommand`、`CommandContext`。
- `crates/tgui-runtime/src/foundation/window_control.rs`：命令上下文中的运行时窗口控制，封装拖拽、拖拽调整大小、最小化、最大化、还原、关闭和最大化状态查询。
- `crates/tgui-runtime/src/runtime/`：运行时核心模块；`mod.rs` 管主流程，`input.rs` 处理文本输入状态，`commands.rs` / `theme.rs` 拆分部分行为，`tests.rs` 聚合运行时测试。
- `crates/tgui-runtime/src/ui/layout/mod.rs`：布局基础类型，封装 `Length`、`Track`、`Insets`、`Align`、`Justify`、`Axis`、`Overflow`、`Value` 等。
- `crates/tgui-runtime/src/ui/widget/`：公开 widget builder 与基础样式；`input/` / `textarea/` 提供文本输入组件。
- `crates/tgui-runtime/src/ui/widget/core/`：组件树解析、Taffy 布局、渲染 primitive 收集、命中区域、选择与文本输入基础设施等高风险核心模块。
- `crates/tgui-runtime/src/ui/theme/`：主题 token、组件主题、状态解析、light/dark/system 模式。
- `crates/tgui-runtime/src/rendering/renderer.rs`：`wgpu` 渲染器，包含矩形、渐变/brush、mesh、文字、纹理、透明窗口 surface、backdrop blur 等 pipeline。
- `crates/tgui-runtime/src/rendering/shader/`：WGSL shader。
- `crates/tgui-runtime/src/media/mod.rs`：图片、SVG、网络/本地/内存媒体加载，纹理缓存，SVG 栅格化，canvas shadow 缓存。
- `crates/tgui-runtime/src/audio/`：启用 `audio` feature 后的 `AudioController`、`Audio` 组件、播放状态与 FFmpeg/CPAL 后端。
- `crates/tgui-runtime/src/dialog/mod.rs`：同步和异步原生对话框封装；桌面端通过 `rfd` 提供文件选择和消息框能力。
- `crates/tgui-runtime/src/notification/mod.rs`：系统通知抽象，包含通知选项、权限状态、动作回调分发与平台后端实现。
- `crates/tgui-runtime/src/platform/mod.rs`：平台抽象和不同 winit 后端的选择。
- `crates/tgui-runtime/src/video/`：启用 `video` feature 后的 `VideoController`、`VideoSurface`、FFmpeg 后端。
- `crates/tgui-core`、`crates/tgui-platform`、`crates/tgui-log`、`crates/tgui-mvvm`、`crates/tgui-media`、`crates/tgui-ui`、`crates/tgui-rendering`：内部边界 crate，当前主要从 `tgui-runtime` re-export 对应 API 面。
- `benches/`：workspace package `tgui-benchmarks`，显式声明 Criterion bench targets。
- `examples/`：workspace 示例工程，也可继续用 `--manifest-path` 单独运行。
- `docs/images/tgui_logo.png`：README 使用的 logo。

## Features 和平台

根 `Cargo.toml` 中的 features 只做 facade 转发：

- `default = []`
- `audio`：转发到 `tgui-runtime/audio`，启用 `ffmpeg-next` 音频解码能力。
- `video`：转发到 `tgui-runtime/video`，在 `audio` 基础上启用视频能力。
- `bench-support` / `collect-profile`：转发到 `tgui-runtime`，用于基准与相位画像探针，关闭时编译成零成本空操作。
- `mimalloc`：可选全局分配器替换，继续留在公开 `tgui` facade，由下游二进制 crate 决定是否启用。

音视频功能仅支持动态链接 FFmpeg；构建时需要 FFmpeg 开发库，运行及分发应用时需要提供对应动态库。

细粒度响应式渲染管线已默认内置（详见下文「细粒度响应式渲染管线」一节），不再通过 Cargo feature 开关启用。

平台依赖按 target 区分：

- Windows、macOS、Linux：桌面窗口、剪贴板、对话框、音频相关依赖。

平台依赖现在位于 `crates/tgui-runtime/Cargo.toml`。FFmpeg 动态库的查找与链接由 `ffmpeg-sys-next` 处理。

## 公共 API 组织

优先从 `src/lib.rs` 理解对外 API：

- `prelude`：示例和小应用常用的一站式导入。
- `application`：`Application`、`WindowSpec`、`WindowRole`、`WindowClosePolicy`。
- `mvvm`：`ViewModel`、`ViewModelContext`、`State`、`Signal`、`TextController`、`TextChange`、`TextChangeSet`、`TextSnapshot`、`Command`、`ValueCommand`、`CommandContext`、`WindowControl`、`WindowResizeDirection`。
- `layout`：`Flex`、`Grid`、`Stack` 以及布局尺寸和对齐类型。
- `widgets`：`Button`、`Text`、`Input`、`Textarea`、`Image`、`Checkbox`、`Radio`、`Select`、`Slider`、`Switch`、`Element`、`WidgetTree` 等。
- `canvas`：`Canvas`、`PathBuilder`、路径、渐变、阴影、布尔运算、画布事件。
- `theme`：`Theme`、`ThemeMode`、`ThemeSet`、组件主题和设计 token。
- `media`：`MediaSource`、`MediaBytes`、`ContentFit`。
- `dialog`：文件选择和消息框。
- `notification`：`NotificationOptions`、`NotificationAction`、`NotificationActionEvent`、`NotificationPermission`、`Notifications`。
- `audio`：仅在 `audio` feature 下导出 `Audio`、`AudioController`、`AudioSource`、`PlaybackState`、`AudioMetrics`。
- `video`：仅在 `video` feature 下导出。

## 应用启动模型

项目当前是 MVVM-only 启动路径。典型流程：

```rust
Application::new()
    .app_id("com.example.demo")
    .title("demo")
    .window_size(dp(960.0), dp(640.0))
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

注意点：

- ViewModel 类型需要实现 `ViewModel: Send + 'static`。
- `ViewModelContext` 用来创建 `State<T>`、`TextController`、`AnimatedValue<T>` 和 timeline controller。
- `State::set/update` 会标记 invalidation 并唤醒事件循环。
- `Signal<T>` 是惰性读取值，可 `map` 派生，也可 `.animated(...)` 给支持的属性添加声明式过渡。
- `Command<T>` 处理无 payload 事件；`ValueCommand<T, V>` 处理带 payload 事件。
- `Command::new_with_context` / `ValueCommand::new_with_context` 可访问运行时服务，例如 `ctx.dialogs()`、`ctx.notifications()`、`ctx.window()` 和 `ctx.log()`。
- `Application` 支持 `bind_title`、`bind_clear_color`、`bind_theme_mode` 这类窗口属性绑定，以及 `on_input` 注册窗口级快捷键。
- 需要系统通知时，优先在 `Application` 上设置稳定的 `app_id`；Windows 下这是通知身份初始化的前置条件。
- `Application::decorations(false)` 和 `WindowSpec::decorations(false)` 可关闭系统标题栏，用于自绘窗口 chrome；无边框透明窗口通常还需要 `clear_color(Color::TRANSPARENT)`。

## 渲染和布局流程

整体流程可以理解为：

1. ViewModel 构建 `Element<VM>` 组件树。
2. `WidgetTree` 解析组件树并用 `taffy` 计算布局。
3. 组件树生成 `ScenePrimitives`、命中区域、滚动区域、IME/caret 信息等。
4. `crates/tgui-runtime/src/runtime/` 处理窗口事件、输入状态、hover/focus/pressed 状态、命令派发与缓存失效。
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

## 细粒度响应式渲染管线

`tgui` 的失效/渲染管线在「细粒度依赖跟踪 + 保留式分块场景图」之上，逐步加入了一组**分级降级**的内置快路径（路线图见仓库根 `FINE_GRAINED_ROADMAP.md`，不进 crate）。核心设计原则：**保留 pull 模型缩短 pull 半径**；**任一快路径前置不满足必须能干净回退**到子树 patch → 整帧重收集，绝不渲染错误；**正确性优先**，每条快路径都有「与从零全量重收集逐项等价」的单测和对应回退路径的测试。

五条快路径（高风险区，改前先读路线图对应快路径的「实施记录」）：

1. **场景命令区间原地拼接。** 叶子/子树 scene-only 改动（如改色）时，若新旧 chunk 各流命令数量完全一致、且子树不含 overlay/portal/scroll-as-overlay/focus/anchor/carousel/virtual/ime，则把新 chunk 原地 splice 进 `cached.computed` 与各祖先 chunk 的稳定区间，**跳过祖先链 `recompose_scene_chunk` 向上重合成**。必须同时覆盖主渲染流（含按类型分组的并行数组）、`hit_regions`、`scroll_regions`——每个 `Container` 无条件 push 一条 `ScrollRegion`。锚点：`scene_primitives.rs`（`counts/SceneCounts/splice_in_place`）、`hit_scene_state.rs`（`is_simple_for_splice/scene_counts/splice_chunk_in_place`）、`scene_layout.rs`（`scene_splice_ancestor_offsets`）、`scene_patch.rs`（splice 快路径）。
2. **属性级依赖归因。** 场景阶段解析视觉属性（background / border color/width/radius / opacity / offset / scale / text color）时把 `PropertySlot` 压入依赖作用域，使 Signal 读取归因到「哪个属性」而非仅「哪个 widget 的哪个 phase」。**兜底红线**：失效消费侧（`scene_patch_invalidation`）当前只读 `widget_id + phase`，从不读 `property`，所以引入归因对失效决策零行为影响——未识别属性安全退化为整 widget 失效，结构上「绝不漏更新」。锚点：`dependency.rs`（`PropertySlot` / `track_property_scope`）、`resolved/collect/chrome/visual_state.rs`、`render/text.rs`。
3. **GPU 顶点脏区间增量上传。** 逐帧顶点池 flush 时与「该轮转缓冲上次写入内容」做字节级 diff，只 `write_buffer` 变化区间（按 `COPY_BUFFER_ALIGNMENT` 对齐），完全相同则跳过。triple-buffered 保证部分覆盖无读写竞态；覆盖后缓冲 `[0..write_len]` 逐字节一致 → 渲染等价。锚点：`vertex_pool.rs`（`last_uploaded` / `dirty_range` / `flush`）。
4. **纯滚动帧子树重收集。** 识别「本帧只有 `scroll_epoch` 变化、其余全不变」的纯滚动帧，只对发生偏移的滚动容器（嵌套时取最高根）调 `patch_cached_scene_for_roots` 重收集**滚动子树**（复用同一收集函数，逐项等价），绕开整树重收集。virtual 容器排除。锚点：`runtime/mod.rs` + `input/interaction.rs`（`scroll_dirty_widgets`）、`cache_support.rs`（`scene_cache_fields_match_ignoring_scroll`）、`scene_runtime.rs`（`try_pure_scroll_fast_path`）。
5. **纯滚动帧 GPU 平移变体。** 在纯滚动子树重收集基础上，对满足严格前置的单一滚动容器保留离屏命令与命中 metadata，纯滚动帧只更新 scroll offset / hit regions，并通过 wgpu `IMMEDIATES` 给 tagged draw 下发 per-draw 平移。adapter 不支持、virtual、嵌套 scroll、overlay/portal、IME、可见 scrollbar、unsupported clip / composite 等任一前置不满足即回退到 CPU 子树重收集。锚点：`scene_primitives.rs`（GPU scroll command metadata / clipped-out 保留）、`layout_media.rs`（滚动内容收集作用域）、`scene_runtime.rs`（`try_pure_scroll_gpu_fast_path` / `gpu_scroll_deferred`）、`renderer/prepare.rs` + shader。

> 改这一管线时务必同时跑：默认 / `--no-default-features` / `audio` / `video` 的 `cargo check`，并补「快路径结果 == 全量重收集」+「回退路径」两类单测。

## 组件和样式约定

组件 builder 大多支持统一的链式 API：

- 尺寸：`size`、`width`、`height`、`min_*`、`max_*`、`aspect_ratio`
- 布局：`margin`、`padding`、`grow`、`shrink`、`basis`、`align_self`、`justify_self`、grid row/column
- 绝对定位：`position_absolute`、`left`、`top`、`right`、`bottom`、`inset`
- 视觉：`background`、`background_brush`、`background_image`、`background_blur`、`border`、`border_radius`、`opacity`、`offset`
- 交互：`on_click`、`on_double_click`、`on_focus`、`on_blur`、`on_mouse_enter`、`on_mouse_leave`、`on_mouse_move`
- 生命周期：`on_mount`、`on_unmount`、`on_update`
- 状态：很多属性接受 `Value<T>`，因此可以传静态值或 `Signal<T>`。

如果新增 widget，优先复用现有 `Element`、`WidgetKind`、`InteractionHandlers`、`MediaEventHandlers`、`VisualStyle`、`LayoutStyle` 模式，而不是另起一套事件或布局系统。

当前仓库提供公开的 `Input` 和 `Textarea` 组件。处理文本输入相关逻辑时，优先沿着 `TextController`、`TextChangeSet`、`crates/tgui-runtime/src/ui/widget/common.rs`、`crates/tgui-runtime/src/ui/widget/core/` 和 `crates/tgui-runtime/src/runtime/input/` 的共享基础设施追踪，而不是把输入、选择、IME、滚动、渲染当成彼此独立的局部功能。

## UI 设计规范

### 适用范围与执行原则

本节以现有内置主题、组件默认样式及 `examples/demo` 为基础整理，适用于框架内置组件、复合组件、组件示例和文档中的默认 UI。**后续组件必须沿用这套视觉语言和交互约定，不得为单个组件随意引入另一套配色、尺寸、圆角或状态规则。**

- 下文标注「当前默认」的数值描述已有实现；「设计要求」约束后续工作，不代表现有组件已全部满足。发现差距时，在相关组件的修改范围内修正，并说明尚未覆盖的部分。
- 用户明确要求的业务风格、品牌主题及特效示例可以定制，但应通过主题或可复用样式实现，保留状态语义与可用性，并在变更说明中记录偏离默认规范的原因。演示渐变、模糊或动画能力的示例不作为普通组件的外观基线。
- 现有代码与本节的数值不一致时，先确认是文档过时还是实现偏差；不要只为匹配文档而批量改动无关组件。调整公共设计 token 时，同步更新本节、相关测试和示例。

设计依据与实现入口：

| 内容 | 来源 |
| --- | --- |
| 语义颜色、排版、间距、形状、动效 | `crates/tgui-runtime/src/ui/theme/` 下的 `color.rs`、`typography.rs`、`spacing.rs`、`shape.rs`、`motion.rs` |
| 密度、状态与主题上下文 | 同目录的 `context.rs`、`state.rs`、`theme.rs` |
| 组件默认外观及尺寸 | `crates/tgui-runtime/src/ui/widget/style/controls.rs` 和 `style/controls/` |
| 共享表面与样式规则 | `crates/tgui-runtime/src/ui/widget/style/shared.rs`、`sheet.rs` |
| 组件展示页的层次与组合 | `examples/demo/src/styles.rs`、`demo_section.rs`、`pages/` |
| 主题 API 用法 | `docs/features/theme.md` |

### 1. 视觉定位与信息层次

整体风格是**克制、清晰、紧凑的桌面工具界面**：中性灰构成页面和内容表面，蓝色强调主要操作与选中状态，小圆角和轻描边组织内容，阴影主要表达浮层关系。

- 内容和操作优先；通过字号、字重、留白和对齐建立层次，避免用大量彩色块、粗边框、大阴影或装饰动画争夺注意力。
- 页面按「页面标题与说明 → 功能分组 → 内容或表单 → 操作与反馈」组织。同一操作区域通常只保留一个主操作，次要操作使用 secondary / ghost，危险操作使用 danger。
- 普通内容以 `background`、`surface`、`surface_low` 分层；有明确分组需要时使用 Card，不把所有段落都包装成多层卡片。
- 浅色和深色是同等重要的设计目标。主题切换保持信息层次、语义和几何尺寸一致，不通过简单反色生成另一种模式。

### 2. 颜色与语义 token

组件样式必须从当前 `StyleContext` / `Theme` 读取语义颜色。以下是当前默认色值，供核对和设计沟通；**组件代码应引用 token，不复制十六进制常量**。六位色值为不透明 RGB，八位色值为 RGBA。

| Token | Light | Dark | 用途 |
| --- | --- | --- | --- |
| `background` | `#FAFAFA` | `#09090B` | 窗口、页面底色 |
| `on_background` | `#09090B` | `#FAFAFA` | 页面底色上的主文字 |
| `surface` | `#FFFFFF` | `#18181B` | 卡片、面板、默认输入表面 |
| `surface_low` | `#F4F4F5` | `#27272A` | 次级内容区、预览区、轻量 hover 背景 |
| `surface_high` | `#E4E4E7` | `#3F3F46` | 更明显的中性填充、交互反馈 |
| `surface_overlay` | `#FFFFFFFA` | `#18181BFA` | 菜单、Popover、Modal 等浮层 |
| `on_surface` | `#18181B` | `#F4F4F5` | 内容表面上的正文和主要图标 |
| `on_surface_muted` | `#71717A` | `#A1A1AA` | 辅助说明、占位符、次要图标 |
| `primary` / `on_primary` | `#2563EB` / `#FFFFFF` | `#60A5FA` / `#08111F` | 主操作、勾选和关键强调及其前景 |
| `primary_container` / `on_primary_container` | `#DBEAFE` / `#1E3A8A` | `#172554` / `#DBEAFE` | 柔和选中背景及其前景 |
| `success` / `on_success` | `#10B981` / `#09090B` | `#34D399` / `#09090B` | 成功、完成 |
| `warning` / `on_warning` | `#F59E0B` / `#09090B` | `#FBBF24` / `#09090B` | 警告、需要注意 |
| `error` / `on_error` | `#DC2626` / `#FAFAFA` | `#F87171` / `#09090B` | 失败、校验错误、危险操作 |
| `outline` | `#D4D4D8` | `#52525B` | 可辨认的轮廓和交互描边 |
| `outline_muted` | `#E4E4E780` | `#3F3F4680` | 卡片等弱分组描边 |
| `disabled` / `on_disabled` | `#F4F4F5` / `#A1A1AA` | `#27272A` / `#71717A` | 禁用背景与内容 |
| `scrim` | `#09090B66` | `#000000CC` | 模态遮罩 |

- 前景与背景成对使用：`primary` 配 `on_primary`，语义实色背景配对应的 `on_*`；柔和选中背景配 `on_primary_container`。不要假定所有强调背景都适合白字。
- 当前 `focus_ring` 为主色乘以 Light 0.75 / Dark 0.65 的 alpha，`selection` 为主色乘以 Light 0.18 / Dark 0.24 的 alpha；焦点提示和文本选区分别引用对应 token。
- 成功、警告和错误颜色必须有明确语义，并配合文字、图标或形状表达状态；不使用颜色作为唯一线索。浅色主题下，语义色直接作为小号文字前景时仍需单独验证对比度。
- 普通正文与实际背景的对比度目标至少为 4.5:1；必要的控件识别和焦点指示目标至少为 3:1。透明色需与实际底色合成后检查，不能假定 `on_surface_muted` 在所有表面上都达标。装饰性弱分隔线不承担唯一的控件识别职责。
- 自定义主题也必须生效。不要在通用组件中调用 `ColorScheme::light/dark()` 来绕过用户主题；demo 中按 resolved mode 同步切换自定义外壳颜色的做法仅用于其展示页。主题过渡过程中同样要检查前景与背景的可读性。

### 3. 排版、图标与文案

字号及行高使用 `sp`，布局和图标尺寸使用 `dp`。优先选择完整的 `theme.typography` 样式，不分别硬编码字号、字重与行高。

| Typography token | 当前字号 / 行高（sp） | 字重 | 使用场景 |
| --- | --- | --- | --- |
| `display` | 40 / 48 | Bold | 少量展示性标题，不用于常规工具栏 |
| `headline` | 28 / 34 | SemiBold | 页面标题 |
| `title` | 20 / 28 | SemiBold | 分组标题、对话框标题 |
| `body` | 16 / 22 | Regular | 正文、输入内容 |
| `body_small` | 14 / 20 | Regular | 辅助说明、紧凑内容 |
| `label` | 14 / 18 | Medium | 按钮、字段标签、导航 |
| `label_small` | 12 / 16 | Medium | 次级标签、简短状态 |
| `code` | 14 / 20 | Regular，monospace | 代码、需要等宽展示的内容 |

- 默认字体跟随框架字体解析与平台回退；确保中文、英文、数字和常用符号可读。不要为单个普通组件引入另一套字体。避免小于 `label_small` 的常规信息文字。
- 标签、输入内容和操作按钮应保持一致的视觉基线。文本缩放或内容变长时允许布局增长、换行或显式滚动，不通过缩小字体掩盖溢出，也不裁掉关键操作文案。
- 图标优先复用 `Icon` / `BuiltinIcon`；新 SVG 与现有图标保持一致的轮廓、线条和光学尺寸。当前 `IconStyle` 默认大小为 `spacing.md`（16dp）。禁止用 emoji 或随意的 Unicode 字符代替稳定的功能图标。
- 图标按钮必须能识别其用途，可通过可见标签或 Tooltip 补充说明；可访问名称能力需按实际支持情况接入，不能把 Tooltip 当作已完成无障碍语义的证明。
- 文案简短、具体、统一语言；操作按钮使用「保存」「添加文件」「重试」等动作词。错误提示说明问题和可执行的修正方法；placeholder 只提供示例或提示，不代替字段标签。

### 4. 间距、密度与布局

当前 `SpaceScale` 为 `xxs=2`、`xs=4`、`sm=8`、`md=16`、`lg=24`、`xl=32`、`xxl=40` dp。以 4dp 为主要节奏，2dp 用于细部调整；12dp、20dp 等优先由现有 token 组合得到，不另建重复尺寸体系。

- 设计建议：图标与文字间距 4–8dp，相关控件间距 8–12dp，表单组或内容组间距 16–24dp，页面内边距 24dp，狭窄窗口可收至 16dp。布局 API 无法直接取得上下文时，将必要常量集中在共享布局样式中，保持与 token 对齐。
- 当前默认密度为 `Density::Compact`。同一区域的按钮、输入、选择器应使用同一密度；组件显式尺寸变体按已有 `ControlSize` 路径实现，不能将密度和尺寸变体混为一谈。

| 当前密度基线（dp） | Compact | Comfortable | Spacious |
| --- | --- | --- | --- |
| 常规控件最小高度 | 32 | 40 | 48 |
| Checkbox / Radio 标记尺寸 | 16 | 18 | 20 |
| Switch 宽 × 高 | 36 × 20 | 40 × 24 | 48 × 28 |
| Card 内边距 / 内部 gap | 8 / 4 | 16 / 8 | 24 / 16 |

- 表中是默认几何基线，不是强制固定高度；文字缩放、边框和实际内容可以使控件增长。Textarea 当前最小高度为 96dp，其他视觉规则沿用 Input。
- 使用 Flex / Grid / Stack 与明确的伸缩、最小值和最大值表达布局；绝对定位用于确有叠放或锚定关系的内容。窗口变窄时合理换行、重排或滚动，不能靠隐藏溢出来掩盖内容损失。
- 长文本、空数据、大量数据和不同 DPI 都是正常输入条件。表格、代码等确需横向空间的内容可局部横向滚动，页面主内容优先适应可用宽度。
- 小图标、勾选标记和滑块的可视尺寸不等于完整命中范围；可通过标签及容器扩大可操作区域。密集列表也要避免相邻操作的命中区重叠。

### 5. 圆角、描边与层级

当前圆角 token：`none=0`、`sm=4`、`md=6`、`lg=8`、`xl=12`、`full=999` dp。描边 token：`none=0`、`thin=1`、`normal=1.5`、`thick=2` dp。`full` 表示胶囊或圆形意图。

| 元素 | 当前默认圆角 | 当前默认阴影 / 层级 |
| --- | --- | --- |
| Button、Input、Textarea、Select 触发器 | `lg` | 普通控件表面，不默认添加装饰阴影 |
| Card | `lg` | `elevation.none`；`surface` + `outline_muted` + `border.thin` |
| Checkbox | `sm` | 以填充和勾选标记表达状态 |
| Radio、Switch | `full` | 以填充、选中标记或位置表达状态 |
| Tooltip | `sm` | `elevation.none`；高对比、不透明表面 |
| Menu、Popover | `xl` | `elevation.md` |
| Toast | `lg` | `elevation.lg` |
| Modal | `xl` | `elevation.xl` + `scrim` |
| Drawer | `none` | 贴边面板，`elevation.xl`；遮罩随模式决定 |
| Badge、进度轨道 | `full` | 紧凑标记或连续轨道 |

- 同类组件复用同一圆角，不随意把普通表单改为大胶囊。嵌套表面的圆角与内边距应协调，避免边缘相切或内容越过圆角。
- 常规卡片依靠留白和轻描边分组；阴影复用 `theme.elevation` 的浅色／深色配置，不为每个组件手写黑色透明度。
- 渐变、毛玻璃、发光和多层阴影作为明确需求下的可选效果，不进入默认控件外观。不得为不透明普通表面添加没有可见收益的 backdrop blur。

### 6. 交互状态与可用性

每个交互组件都必须设计其适用状态及组合状态，不能只交付静止外观：

| 状态 | 设计要求 |
| --- | --- |
| normal | 清晰表达用途和可操作性，默认层次克制 |
| hovered | 通过表面或描边的轻微变化反馈，不移动布局 |
| pressed | 比 hover 更明确的反馈；尺寸与内容位置保持稳定 |
| focused / focus_visible | 键盘焦点可辨认，并与选中态区分 |
| selected / checked | 使用强调填充、选中标记或位置变化；hover 后仍能识别选中结果 |
| open | 触发器与浮层保持关联，适当更新箭头或展开提示 |
| disabled | 使用禁用 token，确实阻止操作，不能只改变颜色 |
| invalid | 错误描边或图标配合邻近错误文案，不依赖单一红色 |
| loading / empty / error / success | 按组件职责提供内容状态、反馈和恢复入口，避免布局剧烈跳动 |

- 复用 `WidgetState` / `StateValue`。当前 `StateValue` 解析优先级为 `disabled → invalid → pressed → hovered → focus_visible → focused → selected → checked → open → normal`，未设置的可选项继续向后解析。组合状态需要明确设计，不能假定多个状态样式会自动叠加；loading 等内容状态也不是现有 `WidgetState` 字段。
- **当前内置主题的 `focus_ring.enabled` 为 `false`**，其配置宽度和间距分别为 `border.thick`（2dp）与 `spacing.xxs`（2dp）。后续交互组件仍须提供有效的键盘焦点反馈，可使用明确的边框／指示器或按需启用主题焦点环；不要假定所有组件已经自动显示焦点环，也不要通过局部修复擅自改变全局默认。
- 按控件语义支持 Tab 导航及 Enter / Space、方向键、Esc 等必要键盘操作。设计要求不等于既有实现能力；涉及尚未支持的路径时，应实现并验证或明确记录限制。
- Modal 等模态交互需要约束焦点范围；菜单和其他可关闭浮层应按各自语义处理 Esc、外部点击和关闭后回焦。浮层靠近窗口边缘时应翻转或限高滚动，复用现有 overlay anchoring 机制。
- 输入组件同时检查 caret、selection、placeholder、只读、禁用、校验和 IME 状态；只读仍可读取或选择内容，不等同于禁用。保留 UTF-8 编辑、中文组合输入及滚动可见性。

### 7. 组件组合规则

- **按钮：** primary 用于关键提交或推进，secondary 用于普通操作，ghost 用于工具栏与低强调操作，danger 用于破坏性操作。保留各变体的状态色，不用仅修改 normal 背景的方式伪造新变体；危险操作文案要明确对象或后果。
- **表单：** 标签、控件、辅助说明／错误提示按稳定顺序排列；同组字段对齐，提交反馈靠近操作区域。Input、Textarea、Select、Combobox 及日期／时间／数字等复合输入复用已有输入外观与弹层规则。
- **选择与切换：** Checkbox 表达独立选择，Radio 表达互斥选择，Switch 表达开关状态；选中、未选中及禁用均可辨认。Slider 配合可读值或标签，进度反馈区分已知进度与等待状态。
- **导航：** Tabs、侧栏、Breadcrumb 与 Pagination 明确当前位置；持久选中与短暂 hover 使用不同线索。侧栏等柔和选中区域使用 `primary_container` 配 `on_primary_container`。
- **数据组件：** List、Tree、Table / DataGrid 优先保证扫描效率，统一行高、列对齐和操作位置；选中行、悬停行、聚焦单元格及展开层次可区分。提供适用的空态、加载态和失败重试；虚拟化不得改变视觉和交互语义。
- **浮层与反馈：** Tooltip 只承载简短解释，交互内容使用 Popover；Menu 承载操作，Modal 承载需要明确处理的任务，Drawer 承载侧向补充内容。Toast 用于短暂反馈，字段错误留在字段旁，需要用户处理的信息不能只靠短时 Toast 传达。
- **媒体与异步内容：** 图片、头像、视频和异步数据保留合理占位尺寸，提供加载与失败外观，避免完成加载时大面积跳动。Skeleton 尽量对应最终内容结构；状态徽标配合文字或图标解释语义。

### 8. 动效

当前 `MotionScale` 为 `fast_ms=120`、`normal_ms=180`、`slow_ms=280`。设计上分别用于轻量状态／小浮层、常规进入与展开、大范围位移；框架的 `StyleContext::motion_*_transition` 辅助方法按实时主题解析并使用 ease-out。

- 动效用于解释状态变化和空间关系，优先短距离位移、轻微缩放、透明度与颜色过渡。避免默认弹跳、夸张缩放、无限装饰动画及反复触发布局的效果。
- 必须响应实时 `reduced_motion`；减少动态效果或对应时长为零时直接到达目标状态，不残留动画 deadline。加载组件仍需保留可理解的静态状态提示。
- 动画不阻塞输入、不延迟实际状态提交；连续快速操作、反向切换、主题切换及关闭组件时，视觉与命中状态必须一致。

### 9. 实现与扩展方式

1. 开始实现前先查找最接近的现有组件，列明用途、变体、尺寸、状态及适用的键盘行为，沿用其公开 builder 和共享基础设施。
2. 全局风格通过 `ThemeBuilder` / `ThemeSet` 和语义 token 调整；内置组件默认外观在对应的 `Style::default_for_theme` / 密度解析路径中实现，主题级组件覆盖使用 `ComponentThemes`。应用内可复用规则使用 `StyleSheet` + class / style id，单个组件的小范围差异优先使用 `.style(...)`。
3. 保持现有组件样式解析顺序：组件默认样式 → 主题组件规则 → StyleSheet → 局部 style；共享表面样式填充视觉属性时还需保留显式 builder 值的优先级。`.style_full(...)` 会提供完整样式对象，仅在有意接管整套样式时使用，并负责保留主题、密度及状态行为。
4. 新的公共视觉能力需要同时检查组件 Style、主题默认值、状态解析、动画、布局与渲染路径，并核对 facade / 边界 crate 导出。可绑定属性优先沿用 `Value<T>` / `Signal<T>`，避免在构建时把动态主题或状态冻结成常量。
5. 新增设计 token 前确认现有 token 无法表达语义；新增时必须定义用途、light/dark 配对、适用状态及组件用法，更新本节。禁止仅为单个画面增加无人复用的全局 token。

### 10. UI 交付与评审清单

新增组件或实际 UI 改动按适用范围检查以下内容；不适用项说明原因。纯文档整理不要求启动示例或执行 Rust 编译测试。

- [ ] 颜色、排版、圆角、间距、阴影和密度符合上述基线；自定义主题及局部覆盖仍有效。
- [ ] 在 Light / Dark 下检查正常、hover、pressed、焦点、禁用及适用的选中／错误／展开状态；检查有代表性的组合状态与主题切换过程。
- [ ] 检查最小可用窗口、较长中英文内容、文本缩放和不同 DPI；无意外裁切、遮挡、溢出或命中错位。
- [ ] 验证适用的键盘路径、浮层关闭与回焦；输入改动检查中文 IME、UTF-8、选区、caret 与滚动。
- [ ] 动效遵循 token，reduced motion 下能立即稳定到目标状态；异步内容具有适用的 loading / empty / error 反馈。
- [ ] 新组件或明显外观变化在 `examples/demo` 的对应页面或专用示例中可复现；实际运行并检查受影响视图，条件受限时明确说明未验证的部分。
- [ ] 按变更范围运行相关现有测试与必要的 `cargo check`；共享状态、布局或渲染行为改变时补充有意义的回归测试，并遵循本文前述高风险管线检查要求。不为单纯数值或文案调整机械新增测试。
- [ ] 交付说明指出设计依据、偏离基线的原因（如有）、验证结果和已知限制；规范或公共 API 改动已同步相关文档。

## 动画系统

动画分两类：

- 声明式属性过渡：`Signal::animated(Transition)`，由 `AnimationEngine` 按 `AnimationKey` 和属性类型解析。
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

## 通知系统

通知能力位于 `crates/tgui-runtime/src/notification/`，通过 `CommandContext::notifications()` 暴露给命令处理：

- `Notifications::send`：发送普通系统通知。
- `Notifications::send_with_actions`：发送最多两个 action 的交互式通知，并把结果回调到 ViewModel。
- `Notifications::request_permission` / `permission_status`：查询或请求通知权限。
- `NotificationOptions` 支持 title、body、subtitle、app name、icon、声音开关和 action。

平台差异：

- Windows：要求 `Application::app_id(...)`，运行时会准备通知身份。
- Linux：当前通过 `notify-rust` 发送通知并监听 action。
- macOS：接口已公开，但当前仍依赖 UserNotifications bridge，调用时可能返回 backend error。

## 音频系统

音频能力位于 `crates/tgui-runtime/src/audio/`，需要 `audio` feature：

- `AudioController` 管理加载、播放、暂停、seek、音量、静音和播放状态查询。
- `Audio` 是挂接到 widget tree 的隐形播放组件，本身不渲染 UI。
- FFmpeg 解码与桌面播放后端位于 `crates/tgui-runtime/src/audio/backend/`，桌面输出依赖 `cpal`。

涉及音频变更时建议至少运行相关模块测试，并在桌面环境补一次实际播放链路检查。

## 视频系统

视频能力位于 `crates/tgui-runtime/src/video/`，需要 `video` feature：

- `VideoController` 管理播放、暂停、seek、音量、静音、buffer memory limit 等。
- `VideoSurface` 作为 widget 参与布局、渲染和命中测试。
- FFmpeg 后端位于 `crates/tgui-runtime/src/video/backend/ffmpeg/`，包含 decode、audio、present 等模块。

涉及视频变更时建议至少运行带 feature 的检查或测试，但本机需要具备对应 FFmpeg/link 环境。

## 示例工程

当前 `examples/` 中存在这些 workspace 示例：

- `animation_showcase`
- `background_effects`
- `basic_window`
- `canvas`
- `demo`
- `dialogs`
- `drawer_demo`
- `frameless_window`
- `list_virtual_list`
- `modal_demo`
- `multi_window`
- `multiple_vm_examples`
- `mvvm_counter`
- `table_datagrid`
- `text_area`
- `timeline_controller`
- `toast_snackbar`
- `tree`

运行桌面示例：

```bash
cargo run -p basic_window
cargo run --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/canvas/Cargo.toml
cargo run --manifest-path examples/frameless_window/Cargo.toml
```

README 中提到的一些示例名称未必都在当前工作区存在；维护文档时以实际 `examples/` 目录为准。

## 常用开发命令

```bash
cargo fmt
cargo metadata --no-deps --format-version 1
cargo check -p tgui
cargo test -p tgui-runtime --lib -- --test-threads=1
```

按 feature 检查：

```bash
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check --workspace --all-targets
```

运行某个测试：

```bash
cargo test -p tgui-runtime <test_name>
```

运行某个示例：

```bash
cargo run -p <example_package>
cargo run --manifest-path examples/<example_name>/Cargo.toml
```

编译 benchmark：

```bash
cargo bench -p tgui-benchmarks --no-run --features bench-support
```

## 测试分布

仓库中单元测试主要集中在：

- `crates/tgui-runtime/src/ui/widget/core/tests.rs`：布局、渲染 primitive、输入、选择、滚动、组件状态。
- `crates/tgui-runtime/src/runtime/tests.rs`：事件、焦点、文本输入编辑、滚动条、命令派发、canvas/video 命中等运行时行为。
- `crates/tgui-runtime/src/application/mod.rs`、`crates/tgui-runtime/src/foundation/window_control.rs`：窗口配置、装饰开关、命令上下文窗口控制。
- `crates/tgui-runtime/src/notification/tests.rs`：通知选项校验、平台分发、异步 action completion 回调。
- `crates/tgui-runtime/src/media/tests.rs`：图片/SVG 加载、栅格化、缓存、外部资源解析。
- `crates/tgui-runtime/src/animation/tests.rs`：属性动画和 timeline 行为。
- `crates/tgui-runtime/src/audio/controller/tests.rs`、`crates/tgui-runtime/src/audio/backend/shared/tests.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/tests.rs`：音频控制器、共享播放后端与 FFmpeg 会话逻辑。
- `crates/tgui-runtime/src/video/backend/ffmpeg/*`：视频后端内部逻辑，需 feature/环境支持。
- `crates/tgui-runtime/src/ui/widget/canvas/tests.rs`、`crates/tgui-runtime/src/ui/widget/common.rs`、`crates/tgui-runtime/src/ui/theme/mod.rs`、`crates/tgui-runtime/src/text/font/tests.rs` 等也有局部测试。

修改共享行为时，不要只跑示例；至少跑相关模块测试。修改 `crates/tgui-runtime/src/runtime/`、`crates/tgui-runtime/src/ui/widget/core/` 或渲染 primitive 时，优先补充小型单元测试。文本选择、光标、IME、输入同步或滚动相关变更要优先覆盖 UTF-8 边界、选区、滚动和失效路径；通知变更优先覆盖 options 校验、平台约束和 completion 回调。

## 维护注意事项

- 不要把 `crates/tgui-runtime/src/runtime/` 或 `crates/tgui-runtime/src/ui/widget/core/` 当作普通小模块随意大改；它们是行为集中区，牵涉输入、布局、缓存、渲染、命令和平台事件。
- 公共 API 变更要同步检查根 `src/lib.rs` facade、相关内部边界 crate 的 re-export、README、示例和文档。
- `Cargo.toml` 的 `exclude` 会排除 `examples/*`、`docs/*`（独立 vitepress 站点）、`crates/*`、`benches/*`、`*.png`、`.github/*`、`AGENTS.md`、`CLAUDE.md`、`FINE_GRAINED_ROADMAP.md`、`skills/*`、`Video.md` 等，发布相关变更时要留意资源和文档是否会进入正确 crate（`cargo package -p <crate> --allow-dirty --list` 核对）。
- 新增平台能力时优先走 `crates/tgui-runtime/src/platform/` 的后端抽象，并用 `cfg` 控制依赖和代码路径。
- 新增窗口控制能力时同步检查 `ApplicationConfig` / `WindowSpec`、`CommandContext`、`WindowControl` 请求队列、runtime drain 逻辑、多窗口关闭策略和平台窗口 API。
- 透明 / 无边框窗口相关改动要同时关注 `Application::decorations`、clear color alpha、surface alpha mode、平台后端选择和示例表现。
- 新增视觉属性时通常需要同时考虑：widget builder、`VisualStyle` 或相关状态、scene primitive、动画 key、renderer/shader。
- 新增绑定属性时优先接受 `impl Into<Value<T>>`，这样静态值和 `Signal<T>` 都可用。
- 新增交互事件时要检查 hover/focus/pressed 状态、命中区域、命令 scope、运行时事件派发以及缓存失效。
- 新增或修改文本输入基础设施时，要同时检查 widget builder、`Input` / `Textarea`、scroll region、caret/selection、键盘事件和主题/渲染输出，而不是只改渲染或只改输入。
- 文本相关修改要注意 UTF-8 边界、IME composition、选择区间、caret 可见性和横向滚动。
- 通知相关改动要同步检查 `Application::app_id`、`CommandContext::notifications()`、平台后端限制、action 数量约束以及异步回调回到 ViewModel 的路径。
- 媒体和异步加载修改要确保完成后调用 invalidation，避免 UI 不刷新。
- 音频相关改动要同步检查 `audio` feature、桌面 `cpal` 输出、FFmpeg 解码错误路径以及 `Audio` 隐形组件的生命周期。
- 对话框异步回调通过 runtime dispatcher 回到 ViewModel；不要在线程里直接持有或修改 ViewModel。
- 当前工作区存在未跟踪的 `Video.md`，不要在无明确需求时删除、重命名或覆盖它。

## 推荐阅读顺序

第一次接手时建议按这个顺序读：

1. `README.md`
2. `src/lib.rs`
3. `examples/mvvm_counter/src/main.rs`
4. `crates/tgui-runtime/src/lib.rs`
5. `crates/tgui-runtime/src/application/mod.rs`
6. `crates/tgui-runtime/src/foundation/binding/mod.rs`
7. `crates/tgui-runtime/src/foundation/view_model/mod.rs`
8. 涉及文本选择、光标、IME、`Input` 或 `Textarea` 时读 `crates/tgui-runtime/src/ui/widget/input/mod.rs`、`crates/tgui-runtime/src/ui/widget/textarea/mod.rs`、`crates/tgui-runtime/src/ui/widget/common.rs`、`crates/tgui-runtime/src/ui/widget/core/` 和 `crates/tgui-runtime/src/runtime/input/`
9. 涉及通知时读 `crates/tgui-runtime/src/notification/mod.rs`、`crates/tgui-runtime/src/foundation/view_model/mod.rs` 中的 `CommandContext`，以及 `examples/demo/src/main.rs`
10. 涉及窗口控制时读 `crates/tgui-runtime/src/foundation/window_control.rs` 和 `examples/frameless_window/src/main.rs`
11. `crates/tgui-runtime/src/ui/widget/core/` 中的 `element.rs`、`layout.rs`、`render.rs`、`tree.rs` 等布局和渲染输出相关部分
12. `crates/tgui-runtime/src/runtime/mod.rs`、`crates/tgui-runtime/src/runtime/commands.rs`、`crates/tgui-runtime/src/runtime/theme.rs` 的事件处理部分
13. 需要改渲染时再读 `crates/tgui-runtime/src/rendering/renderer.rs` 与 `crates/tgui-runtime/src/rendering/shader/*`
