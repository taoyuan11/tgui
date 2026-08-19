# tgui 实施任务清单

> 本清单根据 [`ARCHITECTURE_PLAN.md`](ARCHITECTURE_PLAN.md) 拆分，面向当前几乎为空的 `tgui` crate。按阶段从上到下执行；同一阶段内尽量按编号顺序完成。每个任务完成后勾选，并在对应代码、测试和文档中留下可追溯记录。

## 使用约定

- `[ ]` 未开始，`[x]` 已完成，`[-]` 暂缓或明确不适用。
- 每个任务都应同时补充必要的单元测试/快照测试；没有测试的实现不算完成。
- 阶段退出条件全部满足后，才能进入下一阶段；发现设计取舍时，回写 `ARCHITECTURE_PLAN.md`。
- 公共 API 只暴露应用开发所需的能力；Element Tree、Dirty Tree、缓存代际、预算和大部分 Render Tree 细节默认保持 `pub(crate)`。

## 推荐依赖顺序

```text
P0 契约与骨架
 └─> P1 Widget / Element / State / Event
      └─> P2 Layout / Dirty
           └─> P3 Render / Paint IR / Compiler / wgpu
                └─> P4 Text / Media / Resource
                     └─> P5 Animation / VirtualList
                          └─> P6 Native Host / Accessibility
                               └─> P7 整合 / 性能 / 发布
```

跨阶段约束（线程所有权、代际 ID、Revision、预算、原子提交和回退路径）从 P0 建立，后续阶段不得绕过。

---

## P0：契约、骨架与测试基础

### P0.1 工具链与仓库基线

- [x] 记录当前 Rust stable 工具链、目标平台和最低支持版本（MSRV）；建立统一的 `rustfmt`、`clippy` 和测试命令。
- [x] 为默认 feature、`--no-default-features`、`desktop`、`render`、`text`、`image`、`svg`、`accessibility`、`webview` 组合建立可重复的 `cargo check` 矩阵。
- [x] 在根 `Cargo.toml` 集中声明依赖和 feature 边界；WebView、AccessKit adapter、完整图片/SVG 编解码不得进入最小核心路径。
- [x] 建立 CI（或等价本地脚本）入口，至少运行格式检查、核心构建、默认构建和核心测试。

### P0.2 模块边界

- [x] 创建计划中的模块目录和最小 `mod`/导出骨架：`application`、`core`、`state`、`widget`、`event`、`layout`、`dirty`、`render`、`text`、`media`、`animation`、`virtualization`、`native`、`accessibility`、`diagnostics`、`platform`、`widgets`、`test_support`。
- [x] 在模块文档中标注公开 API、`pub(crate)` 内部 API、线程归属和允许的依赖方向。
- [x] 增加架构不变量清单，并把“普通控件不得走 Native Host”“UI 树只归 UI 线程”等规则写成代码审查/测试检查项。

### P0.3 Core 数据结构

- [x] 实现 `ElementId`、`RenderNodeId`、资源句柄和其他带 `slot + generation` 的代际 ID；旧 generation 在 slot 重用后必须失效。
- [x] 实现可验证的 AoS/dense arena：分配、读取、修改、释放、slot 重用、generation 校验和遍历。
- [x] 用 ID/索引表达父节点、首子节点、下一兄弟等关系；避免常见节点默认“一节点一个堆分配”。
- [x] 实现基础几何和值类型（逻辑像素矩形、点、尺寸、变换、裁剪、颜色、圆角、DPI scale）及边界条件校验。
- [x] 建立统一错误类型，区分用户输入错误、编译错误、资源错误、平台错误和可恢复降级。
- [x] 定义 `NodeKey`/`WidgetKey`、`PropertyId`、`ItemKey` 和窗口句柄等基础标识的相等、哈希和调试格式。

### P0.4 窗口、事务与 Revision 契约

- [x] 定义 `Application`、`WindowSpec`、窗口调度和提交接口的最小契约；先支持单主窗口，但不把 ID/runtime 设计锁死为单窗口。
- [x] 定义 `UpdateTxn`、`UiDispatcher` 和“后台只发送带代际消息”的消息格式；明确 UI 线程所有权。
- [x] 定义 `LayoutRevision`、`SceneRevision`、`ResourceRevision`、`SemanticRevision`，要求单调递增且只在对应可观察输出变化时递增。
- [x] 定义原子 CPU 快照的结构和提交接口，使 Layout、Render/Paint、语义树和资源引用成组替换；编译失败时保留上一份可提交快照。

### P0.5 Headless 测试与诊断骨架

- [x] 实现 headless `TestRenderer`、命令快照工具和确定性/fake clock；无 GPU 也能测试树逻辑。
- [x] 定义 `FrameMetrics`、`CacheBudget`、资源/场景成本报告和 revision 报告的数据结构，先支持空实现/采样。
- [x] 建立固定预算的测试资源管理器，记录 current、peak、hit、miss、evict、upload bytes、失败原因和 in-flight 引用。
- [x] 建立基准 harness，先覆盖 10、100、1,000 个 synthetic arena 节点的分配、遍历、释放和每帧分配统计。
- [x] 为所有核心 ID、arena、几何、revision、事务、快照和预算类型补充单元测试。

### P0 退出条件

- [x] `cargo fmt --check`、`cargo check`、`cargo test`（核心与默认 feature）通过。
- [x] 关闭 `desktop`、`webview`、`accessibility` 后，core、状态契约、布局契约、headless Paint/命令测试仍可构建。
- [x] 有一份可运行的最小 headless 程序，能创建窗口上下文、分配/释放代际节点、生成指标并提交空快照。
- [x] 已记录 P0 实际取舍、工具链和基线结果到 `ARCHITECTURE_PLAN.md`。

---

## P1：Widget、Element、State 与 Event

### P1.1 Widget 声明模型

- [x] 定义 `WidgetNode`、`Widget`/`View` 构建接口和 `WidgetKey`；Widget 构建结果为不可变声明，不直接持有可变 Element。
- [x] 明确 WidgetType、key、属性、子节点和可选构建上下文的比较规则。
- [x] 提供最小 `Container`、`Text` 占位节点和 `Button` 示例，全部走统一 Widget 管线。

### P1.2 Element Tree 与 reconciliation

- [x] 在 arena 中实现 Element 节点、父子/兄弟索引、稳定 `ElementId`、挂载/卸载/更新生命周期。
- [x] 按 `Key + WidgetType` 对齐旧 Element 与新 Widget；稳定 key 的节点保留身份、状态和焦点。
- [x] 处理插入、删除、类型替换、重复 key、key 缺失和 keyed reorder，并提供可诊断错误/安全回退。
- [x] 实现状态槽位、依赖订阅、生命周期清理和卸载时取消资源/动画订阅。
- [x] 记录每节点分配数、slot 使用、generation 和内存占用，供基准和诊断读取。

### P1.3 State、Signal 与事务

- [x] 实现 `State<T>` 可写句柄、`Signal<T>` 只读/派生句柄和一致的读取/写入 API。
- [x] 在 build、measure、paint、semantics 等读取上下文自动记录依赖；禁止这些阶段直接修改应用状态。
- [x] 实现 `UpdateTxn`：同一事务多次写入合并，事件传播结束后批量提交，并产生最小失效集合。
- [x] 实现派生 Signal 的依赖图、失效传播、订阅清理和循环/重入保护。
- [x] 实现 `UiDispatcher` 消费后台结果；消息必须携带 source generation 和相关 revision，过期结果丢弃。

### P1.4 事件、焦点与输入

- [x] 定义覆盖鼠标、触摸、滚轮、拖拽、键盘、焦点、文本/IME、窗口变化和 Accessibility action 的 `UiEvent`。
- [x] 实现命中路径上的 capture → target → bubble 三阶段传播及稳定的事件路径快照。
- [x] 实现 `EventContext`：`stop_propagation`、`prevent_default`、事务命令、`request_focus`/`release_focus`。
- [x] 实现 pointer capture/release、焦点管理、快捷键入口和基础 IME 路由。
- [x] 命中测试只读取上一帧已提交的 Layout/Render 快照，事件中间更新不得改变当前传播路径。
- [x] 为窗口 resize、DPI、激活和关闭请求保留统一事件入口。

### P1.5 P1 验证与退出条件

- [x] 测试 keyed reorder 后 `ElementId`、State、焦点和订阅保持正确；旧 generation 永不重新指向新节点。
- [x] 测试事务合并、派生依赖、卸载清理、后台过期消息和重入保护。
- [x] 测试事件顺序、停止传播、prevent default、pointer capture、焦点切换和 IME 路由。
- [x] 跑通 10/100/1,000 节点的 initial build、idle、single state update、keyed reorder 基准；idle 无动画时不得持续调度工作。
- [x] 用 headless 示例展示 Button 状态更新和事件传播，并记录阶段结果到架构计划。

---

## P2：Taffy Layout 与 Dirty Tree

### P2.1 Layout 契约与快照

- [x] 接入 Taffy，封装 `LayoutStyle`、Flex/Grid/绝对定位/滚动所需的最小公开样式接口。
- [x] 定义统一 `Measure` 回调；Text、Image、VirtualList、NativeHost 均通过该接口提供固有尺寸。
- [x] 实现逻辑像素布局，DPI 只在物理渲染和字形 atlas 阶段转换。
- [x] 实现测量缓存；key 至少包含可用宽高、样式、内容代际、字体代际和缩放比例。
- [x] 定义 `LayoutSnapshot`，保存矩形、基线、裁剪、滚动偏移、命中信息和 `LayoutRevision`。
- [x] 实现布局边界、滚动几何和从 Element 到 Taffy 节点的增量同步/安全重建。

### P2.2 Dirty Tree

- [x] 定义 `DirtyFlags`：`STRUCTURE`、`LAYOUT`、`PAINT`、`HIT_TEST`、`SEMANTICS`、`RESOURCE`。
- [x] 为每个节点保存 `self_flags` 与 `subtree_flags`；子树失效不能错误升级为父节点自身 paint/layout。
- [x] 实现 Dirty Root 队列去重、提交 epoch、可重复消费和合并；同一 epoch 重复标记不得重复入队。
- [x] 实现结构、布局、绘制、命中、语义、资源失效传播及最近 Layout/Render/Hit/Semantics Boundary 选择。
- [x] `RESOURCE` 至少触发绘制；固有尺寸改变时额外触发布局。
- [x] 无法精确识别属性依赖时安全退化为整个 Element 失效；不得漏更新。

### P2.3 增量提交与等价性

- [x] 将 State、Event、Layout、资源完成统一接入 Dirty Tree，并计算最小工作根。
- [x] 实现“增量布局结果等价于整树重建”的 headless 比较器，比较几何、裁剪、命中和 revision。
- [x] 覆盖结构变更、局部 layout、叶子 paint、hit-test、semantics、resource 及 full-layout fallback 的传播矩阵。
- [x] 记录 dirty element 数、各类 root 数、增量/全量重建次数和阶段耗时到 `FrameMetrics`。

### P2.4 P2 退出条件

- [x] Taffy Flex、Grid、绝对定位、滚动和自定义 Measure 有最小测试集。
- [x] Dirty Tree 的所有传播规则有单元测试和至少一组整树/增量快照测试。
- [x] 叶子 PAINT 更新不会重建无关布局；结构/未知依赖能安全回退。
- [x] `LayoutRevision` 单调且只在布局快照可观察变化时递增。

---

## P3：Render Tree、Paint IR、Render Compiler 与 wgpu

### P3.1 保留式 Render Tree

- [ ] 定义 `RenderNode`、`RenderNodeId`、边界/变换/裁剪/透明度/z-order 和命令区间。
- [ ] 实现 Element 到 Render Tree 的收集、Render Boundary 和可缓存 Scene Chunk。
- [ ] 为每个 chunk 记录 revision 元组、失效原因和前置条件；paint-only 更新只重录受影响 chunk。
- [ ] 实现前置条件失效时的子树/整树重收集回退，禁止提交半成品。
- [ ] 保证 Render Tree 不直接调用平台 GPU API。

### P3.2 typed Paint IR 与 Canvas

- [ ] 将 `Canvas` 实现为后端无关的命令记录/回放器，而不是 CPU 像素 buffer。
- [ ] 定义语义化 `PaintCommand`：矩形、圆角矩形、路径、描边、颜色/渐变/阴影/透明度/变换、clip push/pop、Text Run、Image、Glyph Atlas、Layer/Backdrop、`NativeSurface` 占位。
- [ ] 保持中等粒度；Path 内部线段由 Path 值聚合，禁止退化为大量 `SetColor`/`MoveTo`/`LineTo` 状态命令。
- [ ] 为 Paint IR 提供稳定 debug/JSON 或二进制快照格式，并验证 clip/layer 栈平衡。

### P3.3 Render Compiler 与 Compiled Scene

- [ ] 定义 `RenderCompiler`、`CompiledScene`、render pass、batch、pipeline key、顶点/索引/instance 范围、纹理绑定、上传请求和资源代际。
- [ ] 编译前验证 clip/layer 栈、变换、z-order、透明度和 Native Surface 边界，输出可诊断错误。
- [ ] 将相邻兼容矩形编译为 instance buffer（多个 Paint Command 可对应一个 draw），Text/Image 编译为资源引用。
- [ ] 按 chunk revision、Renderer capability、DPI、主题、字体、图片、glyph 和 Resource revision 生成缓存 key。
- [ ] 复用未失效 chunk 的已编译结果；失败时保留上一份可提交场景或安全退化到子树/整树。

### P3.4 Batch、Cache 与成本诊断

- [ ] 实现 Scene Chunk、Compiled Scene、pipeline、顶点/索引/instance、纹理绑定的缓存接口。
- [ ] 在裁剪、透明度、离屏层和 Native Surface 边界切分 batch，并记录原因。
- [ ] 记录 Paint Command 数、batch/pass 数、缓存命中率、GPU 上传字节、chunk 重建数。
- [ ] 为 Layer/Backdrop/透明度隔离记录 offscreen 尺寸、pass 数和 transient VRAM；超预算时返回错误或降级效果。
- [ ] 首期明确不实现全局像素级脏矩形；保留稳定 Scene Chunk 后再评估。

### P3.5 wgpu 与 headless 后端

- [ ] 实现 wgpu 初始化、窗口 surface、resize、DPI、交换链/提交和基础错误处理。
- [ ] 实现矩形 instance batching、路径、clip、透明度和基础合成；普通 Button/Text/List 不得绕过该管线。
- [ ] 实现 glyph/image 资源绑定接口、pipeline cache、设备重建和设备丢失恢复。
- [ ] 实现基于 submission/fence 的延迟资源回收；committed/in-flight 引用的资源不得提前淘汰。
- [ ] 提供无 GPU 的命令 renderer，稳定生成 Compiled Scene/Batch 快照。

### P3.6 P3 退出条件

- [ ] Canvas、Paint IR、Render Compiler、Compiled Scene 和 GPU draw/batch 边界可分别快照和诊断。
- [ ] 相邻五个 FillRect 能合并为一个包含五个 instance 的 QuadBatch。
- [ ] clip/layer 栈错误、Native Surface 不兼容能力和 transient 超预算均可测试且不会提交半成品。
- [ ] wgpu 后端至少通过 resize、DPI、透明度、纹理上传、设备丢失恢复测试；无 GPU 测试全数通过。

---

## P4：Text、Glyph Atlas、Image Cache 与资源预算

### P4.1 Text System

- [ ] 接入 `cosmic-text`，实现字体注册、字体 fallback、Unicode shaping、双向文本、换行和行布局。
- [ ] 对外提供 TextLayout 测量、布局、命中测试、光标/选择几何和渲染 runs；逻辑文本与 Glyph Atlas 解耦。
- [ ] 实现布局缓存，key 包含文本/span 代际、字体、字号、字重、语言、方向、宽度、换行策略和 DPI。
- [ ] 处理 Latin、中文、RTL、emoji、字体切换和 DPI 切换；测试 shaping 不因 atlas 淘汰重复执行。

### P4.2 Glyph Atlas

- [ ] 按字体实例、物理字号、变体和颜色字形类型生成 atlas key，区分单色 mask 与彩色 glyph。
- [ ] 实现分页 atlas 和空闲矩形分配器；每页有独立 generation。
- [ ] 实现淘汰/重新栅格化；旧 page generation 不得继续渲染。
- [ ] 资源完成只标记受影响 Text Run 的 `RESOURCE/PAINT`，不触发无关布局。

### P4.3 ImageSource 与 Image Cache

- [ ] 定义支持路径、内存 bytes、URL、SVG 的 `ImageSource`、`ImageHandle` 和请求去重身份。
- [ ] 实现 CPU 解码/栅格化缓存与 GPU Texture Cache；加载期间支持 placeholder 或兼容旧纹理。
- [ ] 将图片/SVG 解码放入后台任务，回 UI 线程携带 source generation；过期结果必须丢弃。
- [ ] 实现 LRU/clock 或等价有上限淘汰策略，统计命中、失败、占用、淘汰和上传。

### P4.4 资源 Revision 与预算

- [ ] 接入 `CpuCacheBudget`、`GpuCacheBudget`、`TransientGpuBudget`，支持软/硬上限、峰值、失败原因和默认/窗口覆盖配置。
- [ ] 淘汰策略综合重建成本、可见性、最近使用和当前帧引用；in-flight 资源进入延迟回收队列。
- [ ] 资源上传完成后做 generation/revision 校验，递增 `ResourceRevision` 并触发相关 `RESOURCE/PAINT`；固有尺寸变化额外触发 `LAYOUT`。
- [ ] 将缓存/预算快照写入每帧 `FrameMetrics`，区分算法工作量、常驻占用和 transient 峰值。

### P4.5 P4 退出条件

- [ ] 文本测量、命中、光标/选择、fallback 和多语言/DPI 测试通过。
- [ ] glyph eviction 只导致受影响 glyph 重新栅格化；图片旧代际不能覆盖新绑定。
- [ ] 超预算时有可诊断淘汰或降级，不会无限增长 RSS/VRAM；placeholder/旧资源可提交一致 CPU 快照。
- [ ] GPU 不可用时 Text/Image 逻辑仍可用 headless 测试。

---

## P5：Animation 与 Virtualized List

### P5.1 Animation System

- [ ] 实现统一 `FrameClock`、`Timeline`、`AnimationHandle` 和 `Animated<T>` 派生值；组件不得创建独立定时器。
- [ ] 动画 key 使用 `(ElementId, PropertyId)`；明确新动画替换、衔接、取消旧动画的策略。
- [ ] 每帧只标记受影响属性的 `PAINT` 或 `LAYOUT`；动画不直接覆盖基础 State。
- [ ] 支持暂停、取消、完成回调、fake clock 和 reduced-motion；无活动动画时事件循环回到等待。
- [ ] 添加动画取消、重建、卸载和 keyed reorder 后身份保持测试。

### P5.2 VirtualList

- [ ] 定义数据源长度、稳定 `ItemKey` 和 item builder 接口。
- [ ] 只物化可见范围加 overscan；使用前缀和结构（优先 Fenwick Tree）维护估算/实测高度并支持按偏移定位。
- [ ] 高度变化时保持滚动锚点，避免内容跳动；状态按 ItemKey 复用而非按可见索引复用。
- [ ] 支持键盘焦点、选择、滚动到 key、异步测量和 item 销毁/重用清理。
- [ ] 输出集合、位置、数量和当前项语义；聚焦未物化 item 时先物化并滚动到目标。
- [ ] 用 fake clock、50,000/100,000 item 数据源和大列表滚动基准验证物化上限。

### P5 退出条件

- [ ] 动画 fake clock 完全确定，reduced-motion、取消和完成回调稳定。
- [ ] VirtualList 只创建可见区/overscan Element，item 状态、焦点、锚点和语义在滚动/变高后正确保留。
- [ ] `PAINT`/`LAYOUT` 失效范围与动画属性和 item 高度变化一致。

---

## P6：Native Host 与 Accessibility

### P6.1 Native Host 逃生舱

- [ ] 定义 `NativeHost`、`NativeHostFactory`、`HostHandle`、生命周期和错误状态：创建、挂载、卸载、销毁。
- [ ] 定义布局矩形、DPI、可见性、z-order、焦点、键盘/IME、指针转发和合成接口。
- [ ] 定义并验证 `NativeHostCapabilities`（独立 surface/offscreen、transform、alpha、clip、输入转发、batch 合并）与 `NativeHostCost`。
- [ ] 实现平台适配层和 mock host；host 只通过事件/事务回传状态，不直接修改 Widget Tree。
- [ ] 在可选 `webview` feature 下接入一个 WebView/外部 surface 示例；其他平台先可编译并有 mock 行为。
- [ ] Render Scheduler 根据 capabilities/cost 切分 pass、记录独立 surface/同步点成本；不支持的裁剪/透明度/变换返回可诊断错误或使用隔离层。
- [ ] 添加架构测试，确保 Button、Text、List、Input 等普通控件不能通过 NativeHost 实现。

### P6.2 Accessibility Tree

- [ ] 从 Element 语义与 Layout 几何生成内部 `AccessibilityTree`；节点 `NodeId` 由 Element 身份/key 稳定维护。
- [ ] 定义 `Semantics`、角色/名称/值/可操作状态、边界、焦点和 `AccessibilityAction`。
- [ ] 仅在 `SEMANTICS`、焦点、布局边界或可访问滚动状态变化时更新语义树。
- [ ] 接入 AccessKit 及 Windows UIA、macOS NSAccessibility、Linux AT-SPI adapter（按 feature 分层）。
- [ ] 将 AccessKit action 统一回送 Event/Command 系统；文本内容来自逻辑文本而非 Glyph Atlas。
- [ ] 支持 Native Host opaque node/子树桥接和 VirtualList 集合、位置、数量、当前项语义。

### P6 退出条件

- [ ] Native Host 生命周期、z-order、焦点、输入、能力不兼容和失败回退均有 mock/headless 测试。
- [ ] AccessKit action 能到达正确 Element/Command；keyed reorder 不破坏 NodeId。
- [ ] 三平台 adapter 在对应 feature 开启时可编译；关闭 feature 时核心 crate 仍可构建。

---

## P7：整合、性能与发布前整理

### P7.1 综合示例与稳定性

- [ ] 创建综合示例，在同一窗口演示状态更新、事件传播、文本、图片、动画、VirtualList、无障碍和 Native Host。
- [ ] 验证每帧事务顺序：排空更新 → reconciliation/dirty 合并 → pending Layout/Render/Paint/Semantics → 编译/资源引用 → 原子替换 committed 快照 → GPU 提交。
- [ ] 注入编译失败、资源未就绪、设备丢失、预算超限、过期异步消息和窗口 resize，确认上一份一致快照/安全降级可用。
- [ ] 审计普通控件的完整 Widget → Element → Layout → Render Tree → Canvas/Paint IR → Compiler 路径。

### P7.2 完整基准矩阵

- [ ] 为 10、100、1,000、5,000、10,000、50,000 节点建立固定主题、字体、DPI、窗口尺寸、资源集和设备信息。
- [ ] 对每种规模比较 initial render、连续 idle、single paint change、100 个局部变更、isolated layout、结构重排和 full-layout fallback。
- [ ] 增加 VirtualList scroll/可变高度锚点、animation tick、text replacement、字体/DPI 切换、image replacement、glyph eviction/re-rasterization 场景。
- [ ] 增加 layer/backdrop/native-surface stress，观察独立 pass、offscreen 和 transient budget。
- [ ] 每个场景记录 CPU 阶段 p50/p95/p99、分配次数/字节、RSS、GPU 时间、draw/batch/pass、upload bytes、VRAM 峰值、Paint 命令数、chunk 重建、缓存命中/淘汰和 VirtualList 物化数量。
- [ ] 结果附提交版本、平台和设备元数据；建立可比较的基线和回归阈值，不做未经数据支持的 FPS/RAM/包体承诺。

### P7.3 跨平台与发布检查

- [ ] Windows、macOS、Linux 完成编译检查；可用平台完成真实窗口 smoke test（resize、DPI、透明度、设备恢复）。
- [ ] 独立运行 `--no-default-features`、默认 feature、全 feature、WebView/无障碍开关组合的构建和测试。
- [ ] 用相同 release/LTO/strip 设置记录各 feature 组合产物大小与依赖树，建立包体回归基线。
- [ ] 检查文档、示例、公共 API、错误信息、日志和诊断输出；明确当前不兼容主分支旧公开 API。
- [ ] 评估架构是否稳定到可以拆分 `core`、`rendering`、`platform`、`media` 等 crate；未满足条件时继续保持单 crate。
- [ ] 将所有阶段的实际取舍、测试结果、性能基线和已知限制回写 `ARCHITECTURE_PLAN.md`。

### P7 退出条件

- [ ] 综合示例可运行，核心路径在无 GPU/headless 和真实窗口两种环境均可验证。
- [ ] 正确性、渲染/平台、性能/诊断验收项全部有测试或基准证据。
- [ ] Feature、三平台、设备丢失、资源预算和原子提交检查通过，形成发布前报告。

---

## 全局验收清单（每个阶段都要回归）

### 架构不变量

- [ ] build、measure、layout、paint、semantics 不直接修改应用状态。
- [ ] 输入、动画、资源完成和后台结果统一进入 UI 线程事务队列。
- [ ] Widget 以 `Key + WidgetType` 对齐 Element；稳定 key 保留状态和身份。
- [ ] Dirty Tree 可合并、可重复消费，并能安全退化为整树重建。
- [ ] Layout、Render、语义和资源引用以一致 CPU 快照原子提交；不等待异步 GPU 上传。
- [ ] UI 树只由 UI 线程拥有；后台消息带 generation/revision 并做过期校验。
- [ ] 常见节点使用代际 ID + dense storage；trait object 仅用于必要冷路径/扩展点。
- [ ] 所有 CPU/GPU/transient 资源有显式预算；committed/in-flight 引用不能提前回收。
- [ ] 四类 Revision 单调递增，诊断可回答“哪个 State/资源导致哪个 chunk 在哪一帧重建”。

### 正确性回归

- [ ] keyed reorder、slot reuse、焦点和状态保留。
- [ ] 增量与全量的布局、命中、Paint Commands、Compiled Scene、Accessibility Tree 等价。
- [ ] 事件传播顺序、停止传播、Pointer Capture、焦点和 IME 可预测。
- [ ] 旧图片/字体/glyph 异步结果不会覆盖新资源；placeholder/旧资源不阻塞事件循环。
- [ ] 动画 fake clock、取消、reduced-motion 和卸载稳定。
- [ ] VirtualList 物化上限、可变高度锚点、按 key 状态和语义正确。
- [ ] Native Host 生命周期/失败回退与 AccessKit action 可测试。

### 完成定义

- [ ] 代码、测试、诊断指标、文档和架构决策同步提交。
- [ ] 任务对应的退出条件有可复现命令、快照或基准结果。
- [ ] 没有以“以后再测”标记的核心不变量；暂缓项已注明原因、替代方案和恢复条件。

## 当前起点与第一批任务

当前 crate 仅有最小 `src/lib.rs` 和基础 `Cargo.toml`，因此下一步从 `P0.1` 开始，依次完成 `P0.2`、`P0.3`、`P0.5`，再进入 P1。第一批可独立提交建议为：

1. 工具链/feature 矩阵和模块骨架。
2. 代际 ID、dense arena、几何和错误类型。
3. headless `TestRenderer`、fake clock、`FrameMetrics`/预算骨架。
4. P0 核心单元测试与架构不变量文档。
