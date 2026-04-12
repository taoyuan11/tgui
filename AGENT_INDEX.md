# tgui AI Agent 项目索引

本文件面向需要快速理解和修改 `tgui` 的 AI Agent。目标不是替代 `README.md`，而是提供一份高信号的代码导航图，帮助快速判断入口、职责边界和修改落点。

## 1. 项目概览

- 项目类型: Rust 单 crate GUI 框架
- crate 名称: `tgui`
- 核心定位: 基于 `wgpu` + `winit` + `cosmic-text` + `taffy` 的现代 GPU 加速 GUI 框架
- 编程模型: MVVM 风格
- 主要公开能力:
  - `Application` 应用入口与窗口配置
  - `Observable<T>` / `Binding<T>` 响应式状态
  - `Command` / `ValueCommand` 事件命令
  - `Text` / `Button` / `Input` / `Container` 等组件
  - 布局、主题、动画、文本测量与 GPU 渲染

## 2. 先读哪些文件

推荐阅读顺序:

1. `README.md`
   了解项目定位、公开 API、示例风格和当前能力边界。
2. `src/lib.rs`
   这是公共 API 总出口，适合先确认框架对外暴露了什么。
3. `src/application/mod.rs`
   应用构建器、ViewModel 绑定、窗口配置都从这里接入。
4. `src/runtime.rs`
   事件循环、窗口生命周期、输入处理、重绘调度的核心实现。
5. `src/ui/widget/core.rs`
   组件树、布局树构建、交互挂载、场景收集的中心文件。
6. `src/rendering/renderer.rs`
   `wgpu` 渲染管线、文本缓存、矩形与文本绘制。
7. `src/animation.rs`
   绑定动画、时间线动画、补间和播放控制。

如果任务是定向修改，可直接看下面的“模块地图”。

## 3. 模块地图

### `src/lib.rs`

- 作用: crate 入口与公共导出
- 适合处理:
  - 判断某类型是否为公开 API
  - 新增公共导出
  - 查找模块总体边界

### `src/application/`

- 核心文件: `src/application/mod.rs`
- 作用:
  - `Application` builder
  - 窗口标题、尺寸、主题、字体、清屏色配置
  - 绑定 ViewModel、根视图、窗口级输入命令
  - 将配置交给 `Runtime` / `BoundRuntime`
- 适合处理:
  - 启动参数
  - 应用级绑定
  - Window 级 API 扩展

### `src/foundation/`

- `binding.rs`
  - `Observable<T>`、`Binding<T>`、`ViewModelContext`
  - 负责脏标记和响应式读取
- `view_model.rs`
  - `ViewModel`、`Command`、`ValueCommand`
- `color.rs`
  - 颜色结构与基础颜色工具
- `event.rs`
  - 输入触发类型定义
- `error.rs`
  - 框架错误类型
- 适合处理:
  - 响应式状态
  - ViewModel 命令模型
  - 基础类型与错误传播

### `src/ui/`

- `layout.rs`
  - 布局枚举与样式值
  - `Insets`、`Align`、`Justify`、`Overflow`、`ScrollbarStyle` 等
- `theme.rs`
  - `Theme`、`ThemeMode`、颜色板、排版与间距
- `widget/`
  - `core.rs`
    - 组件树中心
    - 负责元素定义、布局树构建、命中测试、场景收集
  - `common.rs`
    - 共享结构，如场景数据、视觉样式、命中区域、滚动相关状态
  - `container.rs`
    - `Container`、`Row`、`Column`、`Grid`、`Flex`、`Stack`
  - `button.rs`
    - `Button`
  - `input.rs`
    - `Input`
  - `text.rs`
    - `Text`
- 适合处理:
  - 新组件
  - 交互事件
  - 样式与布局扩展
  - 滚动、焦点、命中测试

### `src/rendering/`

- `renderer.rs`
  - `wgpu` 初始化
  - surface / device / pipeline 配置
  - 矩形与文本绘制
  - 文本缓存
- `shader/rect.wgsl`
  - 矩形、边框、裁剪相关 shader
- `shader/text.wgsl`
  - 文本纹理绘制 shader
- 适合处理:
  - 绘制结果异常
  - 渲染性能
  - shader 改动
  - 文本渲染表现

### `src/text/`

- `font.rs`
  - 字体注册、字体别名、默认字体
  - 文本测量
  - `cosmic-text` 字体系统接入
- 适合处理:
  - 字体加载
  - 跨平台字体问题
  - 文本测量与字重解析

### `src/animation.rs`

- 作用:
  - `Transition`
  - `AnimationSpec`
  - `Playback`
  - `AnimatedValue`
  - 时间线与补间调度
- 适合处理:
  - 绑定动画
  - 时间线控制
  - 插值类型扩展
  - 动画调度与帧率行为

### `src/runtime.rs`

- 作用:
  - `winit` 事件循环
  - 窗口和 renderer 生命周期
  - 鼠标、键盘、IME、滚轮、触摸
  - WidgetTree 驱动与重绘节流
  - Android 平台分支
- 这是项目最“系统级”的文件之一，改动前建议先通读相关分支逻辑。

## 4. 公开 API 心智模型

典型调用链:

1. 用户通过 `Application::new()` 配置窗口和主题。
2. 如果使用 MVVM，则通过 `with_view_model(...)` 构造 ViewModel。
3. `root_view(...)` 生成 `Element<VM>` 组件树。
4. `WidgetTree` 在运行时构建布局、处理事件并产出可渲染场景。
5. `Renderer` 把场景转换为 GPU 绘制结果。

关键导出位于 `src/lib.rs`，新增公共能力时通常需要同步更新这里。

## 5. 大文件与高影响区

以下文件体量较大，且改动影响范围广:

- `src/runtime.rs`
- `src/ui/widget/core.rs`
- `src/rendering/renderer.rs`
- `src/animation.rs`
- `src/ui/widget/container.rs`

处理这类文件时建议:

- 先定位调用链再改
- 尽量做局部改动
- 优先复用已有数据结构和风格
- 注意公开 API 是否会被 examples 间接依赖

## 6. 示例工程索引

`examples/` 下是多个独立示例目录，可用于反查 API 使用方式。
这些目录是独立 Cargo 工程，不是根 crate 的 `cargo run --example ...` 形式。

- `basic_window`: 最小窗口示例
- `mvvm_counter`: MVVM 计数器
- `static`: 静态组件树
- `widgets_showcase`: 组件展示
- `layout`: 布局示例
- `layout_theme_showcase`: 布局与主题组合展示
- `theme`: 主题切换/主题使用
- `input`: 输入框与输入交互
- `scroll`: 滚动与裁剪
- `animation_showcase`: 声明式动画展示
- `timeline_controller`: 时间线动画控制
- `android_basic_window`: Android NativeActivity 入口

当你不确定某个公开 API 的典型用法时，优先去 `examples/` 搜同名类型。

## 7. 常见修改落点

### 新增窗口级配置

- 首先看 `src/application/mod.rs`
- 若涉及运行时效果，再看 `src/runtime.rs`
- 若最终影响渲染，再补看 `src/rendering/renderer.rs`

### 新增或扩展组件

- 组件定义通常在 `src/ui/widget/`
- 公共样式或交互接线通常在 `src/ui/widget/core.rs`
- 容器型能力优先看 `src/ui/widget/container.rs`
- 完成后检查 `src/ui/widget/mod.rs` 和 `src/lib.rs` 是否需要导出

### 调整布局行为

- 入口: `src/ui/layout.rs`
- 实际落地: `src/ui/widget/core.rs` 中对 `taffy` 的转换逻辑

### 调整主题/默认颜色

- 入口: `src/ui/theme.rs`
- 若影响窗口背景或动态切换，再看 `src/application/mod.rs` 和 `src/runtime.rs`

### 调整文本表现

- 字体与测量: `src/text/font.rs`
- 文本渲染: `src/rendering/renderer.rs`
- 文本组件: `src/ui/widget/text.rs`
- 输入文字行为: `src/ui/widget/input.rs`

### 调整动画

- 主要入口: `src/animation.rs`
- 若动画作用于组件属性，再联动看 `src/ui/widget/core.rs`
- 若动画作用于主题或窗口属性，再联动看 `src/runtime.rs`

## 8. 需要优先忽略的目录

为避免 AI Agent 被构建产物干扰，阅读时应优先忽略:

- 根目录 `target/`
- `examples/*/target/`
- `.git/`
- `.idea/`

## 9. 常用命令

在仓库根目录执行:

```powershell
cargo check
cargo test
cargo fmt
```

单独验证某个示例时，进入对应示例目录后执行:

```powershell
cargo run
```

Android 相关改动需要额外平台环境，默认不要把 Android 失败视为桌面功能回归。

## 10. AI Agent 工作建议

- 先从 `src/lib.rs` 确认公开边界，再深入实现文件。
- 改公共 API 时，检查 `README.md` 和 `examples/` 是否需要同步。
- 改 `runtime`、`renderer`、`widget/core` 时，优先避免跨模块重构式修改。
- 如果只是在补功能，尽量沿用现有 MVVM、`Binding`、`Value<T>`、`Command` 模式。
- 如果要排查交互问题，通常需要同时看:
  - `src/runtime.rs`
  - `src/ui/widget/core.rs`
  - 具体组件文件

## 11. 一句话定位

可以把这个项目理解为:

`Application` 负责装配应用，`Runtime` 负责驱动事件循环，`WidgetTree` 负责布局和交互，`Renderer` 负责 GPU 输出，`foundation` 和 `animation` 为整套 MVVM 响应式 UI 提供状态与动画基础设施。
