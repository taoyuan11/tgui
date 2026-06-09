# Refactor Notes: Coupling & Code Quality

**Status**: 2026/06/09 — rustc warnings: 0 (all targets, all features). Clippy: ~270 suggestions (83 coupling-related: "too many arguments" / "very complex type").

## Coupling Hot Spots (by severity)

### 1. `runtime/` 高风险耦合区 (CLAUDE.md 已标注)
集中了输入/布局/缓存/渲染/平台事件之间的耦合，修改需要极度谨慎：

- `text_cursor_index_at_point` (helpers.rs:151): **12 个参数** — 文本命中测试需要字体、布局、主题、滚动状态、选择状态等，是整个文本输入基础设施的缩影。
- `ensure_text_input_caret_visible` (input/navigation.rs:69): 10 个参数 — 滚动/聚焦/文本输入三者的交汇点。
- `begin_text_selection` (input/focus.rs:380): 10 个参数 — 选择/命中/滚动/文本状态。

**根因**: `runtime/` 本质上是「世界状态协调器」，把平台事件分发到布局/渲染/输入/动画/媒体各子系统。许多函数需要跨子系统的上下文（字体、主题、滚动、选择、IME），导致参数列表膨胀。

**安全重构路径**:
1. **Context struct 分层**: 引入 `TextInputContext` / `ScrollContext` / `HitTestContext` 等领域上下文结构体，把相关参数打包（如 `TextInputContext { font, theme, text_state, selection, ime_state }`），但保持这些结构体「借用包」而非「拥有状态」（避免生命周期复杂化）。
2. **重构边界**: 从叶子辅助函数开始（helpers.rs），逐步向上推；`runtime/mod.rs` 的主循环方法暂不动（它们本身就是边界）。
3. **测试驱动**: `runtime/tests/` 已有大量单测（事件、焦点、文本输入、滚动、命中），重构前后跑全量确保行为不变。

### 2. `widget/core/resolved/collect*.rs` — `collect_scene` 17 个参数
`collect_scene(layout, font, theme, media, anim, reduced_motion, hovered_scrollbar, active_scrollbar, widget_states, select_states, menu_states, menubar_states, context_menu_states, scroll_states, virtual_states, viewport, focused_input, ...)` — 容纳了 widget 渲染所需的全部运行时上下文。

**根因**: 场景收集是「只读渲染遍历」，需要查询布局结果、主题 token、动画状态、交互状态、滚动偏移等，而这些状态分散在 runtime 各处。

**安全重构路径**:
1. **CollectContext 只读包**: 引入 `CollectContext<'a>` 把这 17 个参数打包为一个结构体，`collect_scene` 签名变为 `(&self, ctx: &CollectContext<'_>) -> ComputedScene`。由于是只读借用，生命周期简单（全部 `'a`）。
2. **分阶段推进**: 先在 `collect.rs` 入口创建 `CollectContext`，叶子函数（`collect_layout_media.rs` / `collect_control.rs`）签名逐步改为接受 `&CollectContext`。
3. **不触碰 `resolve` 边界**: `WidgetTree::patch` / `patch_layout_roots` 等入口不动，仅内部传递方式变化。

### 3. 渲染器顶点构建 (rendering/renderer/vertex.rs)
- `quad` (vertex.rs:405): 10 个参数 — 顶点坐标、UV、颜色、变换、裁剪。
- `transformed` (vertex.rs:436): 9 个参数 — 变换矩阵相关。

**根因**: 手动构建 `wgpu` 顶点数据，需要几何/纹理/变换/裁剪的全部细节。

**安全重构路径**:
1. **QuadSpec / TransformSpec 结构体**: 把几何/UV/颜色/变换分别打包为小型配置结构体，`quad(spec: QuadSpec, transform: Transform, clip: ClipRect)`。
2. **Builder pattern (optional)**: 如果调用点繁多，可以引入 `QuadBuilder::new().uv(...).color(...).build()`，但当前调用点集中在 `prepare.rs`，直接改结构体参数即可。

### 4. 其他
- `accessibility::collect_widget`: 8 个参数，但辅助功能模块相对独立，影响面小。
- `overlay::solver`: 弹出层布局求解器，10 个参数（视口、锚点、对齐、偏移、约束…），但这是「算法函数」而非「协调器」，参数数量合理反映问题域复杂度。

## 非耦合 Clippy 建议 (低优先级)

- **field assignment outside of initializer** (25): `let mut x = Foo::default(); x.field = ...;` → 改 `Foo { field: ..., ..Default::default() }` 或自定义构造函数。低风险，逐步清理。
- **this `impl` can be derived** (13): 手写的 `Default` / `Clone` 可自动派生。安全，机械修改。
- **large size difference between variants** (5): 枚举某变体特别大，boxing 它可减少栈拷贝。需要 profiling 确认是否热路径。
- **manual implementation of an assign operation** (8): `x = x + y;` → `x += y;`。机械清理。

## 不建议修改的「警告」

- **too many arguments** 在 `runtime/mod.rs` 的事件循环主方法（如 `handle_window_event`）: 这些是系统边界，参数数量反映了它们需要协调的子系统数量，强行拆分会把复杂度推到调用点或引入全局状态，得不偿失。
- **complex type** 涉及泛型闭包 / `impl Trait` 嵌套: Rust 编译器自动推导的类型，人工命名反而降低可读性。

## 推荐阅读顺序（重构前）

1. `AGENTS.md` § 运行时 (runtime/) — 理解事件循环、输入状态、场景缓存的交互。
2. `src/runtime/tests/` — 行为契约，重构后必须全部 pass。
3. `CLAUDE.md` § 高风险区 — 明确哪些模块修改需要额外谨慎。
4. 本文档 — 具体重构路径。

## 行动建议

**立即**: 无 — 当前 0 rustc 警告，系统功能完整，性能已大幅优化。

**计划内重构** (估算 16-24 小时):
1. Phase 1 (6-8h): `CollectContext` 重构 `collect_scene` 签名，覆盖 `widget/core/resolved/collect*.rs`。
2. Phase 2 (6-8h): `TextInputContext` / `ScrollContext` 重构 `runtime/helpers.rs` 和 `runtime/input/*.rs` 的文本/滚动相关函数。
3. Phase 3 (4-6h): 渲染器 `QuadSpec` / `TransformSpec`。
4. Phase 4 (2-4h): 清理机械 clippy 建议（derived impl / field init / manual assign）。

**前置条件**: 每个 Phase 开始前跑 `cargo test --lib`，结束后再跑，确保 644+ tests 全绿。
