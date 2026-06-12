# 细粒度响应式终极形态 · 开发实施计划

> 目标：把 `tgui` 从「细粒度依赖跟踪 + 保留式分块场景图子树增量 patch」推进到「Signal `set()` 直达渲染节点、单属性 O(1) 更新、滚动/动画零重收集」的终极形态（SolidJS / Leptos 级落地粒度）。

## 实施状态（2026-06-12 更正）

**所有阶段均未实施。** 此前版本的状态块声称 Phase 0–5 已完成，经核对为误记：那些工作出自一次未提交的实验会话（代码从未合入仓库），仓库中不存在以下任何产物——

- `benches/single_property_update.rs`、`src/runtime/action_stats.rs`（Phase 0 度量护栏）
- `command_spans` 索引 / splice 快路径（Phase 1）
- `PropertySlot` / `track_property_scope` 属性作用域机制（Phase 2）
- feature gate `fine-grained-splice` / `property-deps` / `incremental-upload` / `transform-only-*`（Phase 1–4）
- `IMPLEMENTATION_COMPLETE_SUMMARY.md`、`最终实施报告.md`

当前实际进度：

- ✅ **Phase 0**（度量与护栏）：**已实施**（2026-06-12）。三件工具均落地仓库——
  `benches/single_property_update.rs` + `WidgetBenchmarkContext::patch_single_deep_leaf_scene`、
  `src/runtime/action_stats.rs`（action 命中计数器，`bench-support` 门控、关闭零成本）、
  `collect_profile.rs` 的 `record_node_visible`（recollect/visible 比值探针）。正式基线见附录 A.2。
- ✅ **Phase 1**（命令区间 splice）：**已实施**（2026-06-12）。落地内容见下方 Phase 1 章节「实施记录」。
- ✅ **Phase 2**（属性级依赖归因）：**已实施**（2026-06-12）。落地内容见下方 Phase 2 章节「实施记录」。
- 🟡 **Phase 3**（GPU 区间增量上传）：**CPU 可证安全部分已实施**（2026-06-12，feature `incremental-upload`，**默认关闭**）。落地内容见下方 Phase 3 章节「实施记录」。**GPU 视觉验证未完成**——本机无 GPU 设备，renderer 无法 headless 构造，路线图要求的「实跑示例 + 截图比对」需在真实硬件上补做后才能决定是否设为默认。
- 🟡 **Phase 4**（滚动 transform-only 快路径）：**CPU 可证等价部分已实施**（2026-06-12，feature `transform-only-scroll`，**默认关闭**）。落地内容见下方 Phase 4 章节「实施记录」。已落地「纯滚动帧 → 仅重收集滚动子树」的安全快路径（与整帧重收集逐项等价、已单测）；路线图描述的「每-draw 平移 uniform + 收集不剔除 + draw 阶段视口裁剪」GPU 可见变体属最高风险，需真实硬件验证，未实施。
- ✅ **Phase 5**（收尾、文档与发布）：**已实施**（2026-06-12）。落地内容见下方 Phase 5 章节「实施记录」。feature gate 默认值已定稿、文档已同步、全 feature 矩阵 `cargo check` 通过、crate 打包清单已核对。
- 该实验会话留下的两条**方向性结论**仍然有效，已纳入路线图正文：
  1. 单属性更新成本随树规模**超线性**增长（祖先链 recompose 是主因）——印证 Phase 1 的必要性；
  2. 定高滚动容器下 collect 的节点游走已是视口受限的（recollect/visible ≈ 1.0），Phase 4 的收益重心应在**避免每帧重跑 layout / epoch 全量 pass**，而非减少收集节点数（详见附录 A.2 注记）。

---

## 0. 现状基线（开工前必读）

当前架构已经具备终极形态的**上半身**，缺的是**下半身**：

| 维度 | 现状 | 终极形态 | 差距 |
|---|---|---|---|
| VDOM Diff | 无，依赖驱动 | 无 | ✅ 已达成 |
| Signal 知道订阅者 | 是，精确到 `{widget_id, phase}` | 精确到属性 | 🟡 粒度偏粗 |
| 视图闭包整体重跑 | 否，只重解析受影响子树 | 否 | ✅ 已达成 |
| `set()` 落地 | 标脏 + 延迟 pull（`mark_dependency_dirty`） | push 直达节点 | 🟡 可保留 pull，但要直达 |
| 更新粒度 | 子树重收集 + 祖先链 `recompose_scene_chunk` 向上重合成 | 单属性原地改 | ❌ 核心差距 |
| 滚动 / 动画 | 每帧重跑 layout / epoch 全量 pass + 重收集（collect 节点游走已视口受限，但整帧成本仍随树规模增长，见附录 A.2） | 仅改 transform / uniform | ❌ 核心差距 |
| GPU 提交 | 每帧 `prepare` 整张命令表成顶点 | 脏区间增量上传 | ❌ 核心差距 |

关键代码锚点（实施时对照）：

- 依赖跟踪：`src/foundation/binding/dependency.rs`（`DependencyId`、`DependencyOwner{widget_id, phase}`、`record_dependency_read`、`track_dependency_scope`）
- 失效信号：`src/foundation/binding/invalidation.rs`（`mark_dependency_dirty` → revision++ + 日志 + wake）
- 失效分发：`src/runtime/binding_sync.rs:136` `request_redraw_if_dirty` → `dirty_dependencies_since`
- 失效决策：`src/runtime/scene_patch_invalidation.rs:5` `invalidate_cached_scene_for_dependencies`（决定 patch / recollect / 全量重建）
- 子树 patch：`src/runtime/scene_patch.rs:59` `patch_cached_scene_for_roots`（重收集子树 chunk + 祖先 `recompose_scene_chunk` 向上合成 → `cached.computed`）
- chunk 合成：`src/ui/widget/core/scene_layout.rs:304` `recompose_scene_chunk`（`before_children + Σ child_chunks + after_children`）
- 场景数据：`src/ui/widget/common/hit_scene_state.rs:40` `ComputedScene`（`scene: ScenePrimitives` 扁平命令表）
- GPU 准备：`src/rendering/renderer/prepare.rs`（`RenderCommand` → `PreparedCommand` → 顶点区间）；`vertex_pool.rs` 已三缓冲
- 滚动/动画热路径：`src/runtime/scene_runtime.rs:194+` `collect_scene_cache_from_layout_*`（每帧整树重收集）

> 记忆参考：`recompose` 的「container extend re-aggregation」是当前剩余最大架构成本；滚动/动画 recollect 是已知 jank hotpath；vertex pool 已三缓冲但未视觉验证。

## 1. 设计原则

1. **保留 pull 模型，但缩短 pull 半径。** 不强行改成 push-直写（在有独立 taffy 布局阶段的 GUI 里 push 直写很难正确处理 clip/z-order/布局耦合）。终极形态的「O(1)」落在：失效后只触碰一个 primitive + 一段 GPU 顶点，而不是重收集子树 + 向上重合成到根。
2. **分级降级链必须保留。** 任何新增的「快路径」失败时，都要能干净回退到现有的子树 patch → 全量重建，绝不能因为快路径假设不成立而渲染错误。
3. **正确性优先于性能。** 每个阶段先补单测（runtime + widget core 都有 `mod tests`），再优化。clip、z-order、UTF-8 边界、IME caret、横向滚动是高危回归点。
4. **可观测。** 复用现有 `collect-profile` / `text_profile` 探针机制，新增快路径要打 action 标签（如现有的 `scene_subtree_patch` / `global_full_rebuild`），便于统计命中率。

## 2. 分阶段路线

整体分 6 个阶段（Phase 0 度量打底 + Phase 1–5 主线，依赖递增），每阶段独立可发布、可回退。建议顺序实施。

---

### Phase 0 · 度量与护栏（1 周，低风险，先做）· ✅ 已实施（2026-06-12）

**为什么先做：** 不能盲优化。记忆里已知「recompose 的容器再聚合是大头、滚动/动画 recollect 是 jank」，但缺单属性更新这条具体路径的基准数字，无法判断每阶段是否真有收益。

**改动点：**
- `benches/` 新增 `single_property_update.rs`（需 `bench-support`）：构建 n=1k/10k 节点树，循环 `state.set()` 改一个深层叶子的颜色 / 文本，测端到端 `request_redraw_if_dirty` 耗时。
- 复用 `collect-profile` feature：在 `scene_patch_invalidation.rs` 的每个 action 分支已有日志基础上，补一个「命中计数器」（各 action 触发次数），通过现有 `text_profile` 机制导出。
- 在 `scene_runtime.rs` 滚动/动画路径加单独计时桶（`recollect_duration` 已存在，补「重收集节点数 / 实际可见节点数」比值）。

**验证：** `cargo bench --features bench-support --bench single_property_update` 产出基线数字，记录到本文件「附录 A · 基线数据」。

**回退：** 纯增量，无回退风险。

---

### Phase 1 · 稳定命令区间索引 + 场景原地拼接（3–4 周，🔴 高风险，keystone）

**目标：** 消除叶子级 scene 改动时的「祖先链 `recompose_scene_chunk` 向上重合成到根」。这是当前 `patch_cached_scene_for_roots` 的核心成本（`scene_patch.rs:305-319`：祖先按深度排序后逐级 `composed.extend(child_chunk)`）。终极形态要求改一个叶子只触碰它自己在根扁平场景里的那一段命令，不重新聚合父链。

**核心思路：** 给 `cached.computed.scene`（根扁平 `ScenePrimitives`）建立 `widget_id → 命令区间 [start, end)` 的稳定索引。叶子 scene-only 改动时：
1. 重收集该叶子的 chunk（仅它自己，已有 `collect_scene_cache_for_widget`）；
2. 若新 chunk 的**命令数量与旧的一致**（纯属性变化，无增删），直接 `splice` 替换根扁平场景里对应区间，跳过所有祖先 recompose；
3. 若命令数量变化（结构性），回退到现有 `recompose` 路径。

**改动点：**
- `src/ui/widget/core/scene.rs` / `hit_scene_state.rs`：`ComputedScene` 增加 `command_spans: HashMap<WidgetId, Range<usize>>`（或更紧凑的 `Vec<(WidgetId, u32, u32)>`），在 collect 收尾时构建。
- `src/ui/widget/core/scene_layout.rs`：collect / recompose 时同步维护区间；新增 `splice_widget_commands(widget_id, new_chunk) -> bool`（数量一致才成功）。
- `src/runtime/scene_patch.rs`：`patch_cached_scene_for_roots` 增加「splice 快路径」，失败再走 recompose。
- 注意 z-order：splice 必须保持命令在扁平表里的相对顺序与 z-order 一致——这是正确性红线。

**风险：** 命令区间会因 clip push/pop、portal、overlay 错位。必须严格区分「可原地替换的视觉命令」与「会改变后续命令偏移的结构命令」。

**验证：** `src/runtime/tests/` + `src/ui/widget/core/tests/` 补：叶子改色后整树渲染命令逐项 diff 应只在该区间不同；多兄弟、嵌套 clip、overlay 下的 z-order 不变；splice 失败能正确回退。`cargo test`。

**回退：** feature-gate `fine-grained-splice`（默认开，可关）；splice 任何前置条件不满足即落回 recompose。

#### 实施记录（2026-06-12）

落地与上文设计一致，但有一处**关键修正**：原计划只 splice 渲染 `commands` 流，实测发现这不足以保持正确——`ScenePrimitives` 同时维护「按类型分组的并行数组（shapes/texts/…）」与「按插入序的统一 `commands` 流」，且每个 `Container` 在 collect 时**无条件** push 一条 `ScrollRegion`（`layout_media.rs:109`，与是否可滚动无关）。因此 splice 必须同时原地覆盖：所有主渲染流（含并行数组）、`hit_regions`、`scroll_regions`——三者都按子树连续排布。

- `ScenePrimitives::counts()` / `SceneCounts`（`scene_primitives.rs`）：各流命令数量快照；`add_assign` 沿路径累加、`has_no_overlay` 判定。`splice_in_place` 对每条主流做边界检查后 `clone_from_slice` 原地覆盖。
- `ComputedScene::is_simple_for_splice()` / `scene_counts()` / `splice_chunk_in_place()`（`hit_scene_state.rs`）：splice 资格判定（无 overlay/portal/focus/anchor/carousel/virtual/ime/外部 portal）+ 主流 + hit_regions + scroll_regions 三者一并覆盖。
- `ResolvedSceneLayout::scene_splice_ancestor_offsets()`（`scene_layout.rs`）：自顶向下游走 + 后缀和，一次算出目标子树在**每个严格祖先 chunk**（含根）里的 (主流 SceneCounts, hit, scroll) 三类偏移。纯连接合成模型保证偏移稳定。
- `patch_cached_scene_for_roots`（`scene_patch.rs`）：单 root patch、新旧 chunk 均 `is_simple_for_splice` 且 `scene_counts` + `hit_regions.len` 全等时记 splice 计划；随后用 ancestor offsets 把新 chunk 原地覆盖进每个祖先 chunk，**跳过整条 recompose 重合成**。任一前置不满足即走原 recompose。其后的 root-clone / `finalize_portals` / 尾段完全不变，故 `cached.computed` 与 recompose 路径**逐字节等价**。

**正确性证明依据：** 根 chunk 即 `cached.computed`，子树不含 overlay 时 `finalize_portals` 只动 overlay 流，主流位置一致；数量一致 → 后续命令偏移不变 → 仅目标区间字节变化。

**测试（`specialized_patch_tests.rs`）：** 3 个新测试，核心断言「splice 结果与一次从零全量重收集逐项等价」——
- `splice_color_change_matches_full_recollect`：深层 surface 改色，spliced scene 与全量重收集逐项相等；且测试探针 `splice_probe` 断言**确实命中 splice 快路径**（避免「只验证了回退」的假阳性）；diff 仅一项、几何不变、仅颜色变。
- `splice_repeated_updates_stay_consistent`：连续多次改色，offset 不漂移。
- `splice_sibling_zorder_is_preserved`：改中间兄弟，前后兄弟 z-order/位置不变。

特性关闭时同样 3 个测试经 recompose 回退路径通过（探针断言被 cfg 关掉），证明降级链正确。全套 656 测试通过；feature 矩阵（默认 / `--no-default-features` / `audio` / `video` / `video-static`）均 `cargo check` 通过。

> ⚠️ 本机注记：该 crate 的测试二进制在默认 dev profile（crate opt=1 + 全量 debuginfo）下触发 macOS Mach-O 重定位上限「object file too large」——**这是开工前 committed master 上就存在的环境问题，与本改动无关**。规避：`CARGO_PROFILE_DEV_DEBUG=0 cargo test`（保持 opt=1、去 debuginfo；opt=0 则会栈溢出）。

**回退：** feature-gate `fine-grained-splice`（默认开，可关）；splice 任何前置条件不满足即落回 recompose。

---

### Phase 2 · 属性级依赖归因（2–3 周，🔴 高风险，触碰绑定内核）

**目标：** 当前 `DependencyOwner = {widget_id, phase}`，失效只能定位到「哪个 widget 的哪个阶段」，无法知道「改的是颜色还是文本还是 transform」。终极形态要直写单属性，必须知道脏的是哪个属性 → 才能只改对应 primitive 字段。

**核心思路：** 在 `record_dependency_read` 记录依赖时，附带一个轻量「属性槽」标识（如 `PropertySlot { kind: Fill | Opacity | Transform | Text | … }`）。绑定属性时（`impl Into<Value<T>>` 的解析处）把当前正在解析的属性种类压入 owner 栈，使 Signal 读取被归因到具体属性。

**改动点：**
- `src/foundation/binding/dependency.rs`：`DependencyOwner` 增加 `property: Option<PropertySlot>`；`track_dependency_scope` 支持嵌套属性作用域。
- 属性解析处（`src/ui/widget/core/element_resolve.rs`、`style.rs`、`resolved_*.rs`）：在解析每个绑定属性前后包一层属性作用域。**工作量主要在这里**——要把每个声明式属性映射到 `PropertySlot`。
- `DependencyGraph`：`owners_for` 返回的 owner 现在带属性，失效决策可据此选「直写哪个字段」。

**风险：** 这是项目两大高危区之一（绑定内核），改错会导致漏更新（最危险）或过度更新（仅性能）。务必保留「未识别属性 → 退化为整 widget Scene 失效」的兜底。

**验证：** `src/foundation/binding/tests.rs` 补：每种属性绑定改动后，dirty 的 owner 属性槽正确；未覆盖的属性安全退化。对照 Phase 0 基线确认无漏更新（关键：跑全套现有 UI 测试，渲染输出 0 回归）。

**回退：** `property` 为 `Option`，全 `None` 时行为与现状完全一致；feature-gate `property-deps`。

#### 实施记录（2026-06-12）

落地与上文设计一致，并贯彻了「正确性优先、归因失败安全退化」的红线。一处与原计划的**取舍说明**：属性作用域只包裹**场景阶段已确定为视觉属性**的解析点（在 `collect` 路径内），而非在 `element_resolve.rs` / `style.rs` 的每个 `impl Into<Value<T>>` 入口铺设——后者会把作用域散落到布局/结构阶段且难以与 `PropertySlot` 一一对应。当前覆盖的属性槽已足以支撑 Phase 4 的 transform-only / 单属性直写；未覆盖的视觉属性自然落在裸 `Scene` owner 上，安全退化为整 widget 失效。

- `src/foundation/binding/dependency.rs`：`PropertySlot`（`Background` / `BorderColor` / `BorderWidth` / `BorderRadius` / `Opacity` / `Offset` / `Scale` / `TextColor`）；`DependencyOwner` 增 `property: Option<PropertySlot>`（`property-deps` 关闭时恒 `None`）。`track_property_scope(slot, f)`：复制栈顶 owner、把 `property` 设为 `slot` 后压栈，`f` 内的 `Signal` 读取即被归因；`f` 返回自动弹栈恢复外层 owner。栈空（无外层作用域）时不引入任何 owner，与 `record_dependency_read` 的「无作用域即不记录」语义一致。`property-deps` **关闭**时 `track_property_scope` 编译为 `#[inline(always)]` 直接调 `f()`，不触碰作用域栈——逐字节透明。
- `src/ui/widget/core/resolved/collect/chrome/visual_state.rs`：`offset` / `scale` / `opacity` / `border_width` / `border_radius` / `border_color` / `background` 七处解析各包一层对应 `PropertySlot` 作用域。
- `src/ui/widget/core/render/text.rs`：`push_text_primitives` 中 `text.color` 的 `resolve_widget` 包 `TextColor` 作用域。
- `src/foundation/binding/mod.rs` / `src/ui/widget/core/mod.rs`：导出 `track_property_scope` / `PropertySlot`，经既有 `super::*` 链传播到各 collect 子模块。

**安全退化的本质（兜底红线）：** 失效消费侧 `scene_patch_invalidation.rs` 当前**只读 `owner.widget_id` + `owner.phase`**，从不读 `property`。因此引入属性槽对失效决策**零行为影响**——属性元信息只是被记录下来供 Phase 4 消费，今天每个属性仍退化为整 widget 的 `Scene` 失效。这从结构上保证「绝不漏更新」：归因正确则未来可精细化，归因缺失/错误也至多回到现状粒度。

**测试（`src/foundation/binding/tests.rs`）：**
- `property_scope_attributes_signal_read_to_slot`（`property-deps` 门控）：`Background` 作用域内的信号读取被归因到 `Some(Background)`，且不残留裸 `Scene` owner。
- `property_scope_restores_outer_owner_after_drop`（`property-deps` 门控）：作用域退出后读取归因回外层 `property: None` owner，验证嵌套压/弹栈正确。
- `property_scope_is_transparent_when_feature_disabled`（`not(property-deps)` 门控）：关闭归因时 owner 恒 `None`，与归因前等价。
- `property_scope_without_outer_scope_records_nothing`（两配置通用）：栈空时不引入 owner、不退化为 global。

全套测试：默认 658 通过、`--features property-deps` 659 通过（门控测试差异），**0 回归**。feature 矩阵（默认 / `--no-default-features` / `property-deps` / `no-default + property-deps` / `audio` / `video` / `video-static` / `video,property-deps`）均 `cargo check` 通过；`cargo fmt --check` 干净。

> ⚠️ 本机注记：测试沿用 Phase 1 的规避方式 `CARGO_PROFILE_DEV_DEBUG=0 cargo test`（macOS Mach-O 重定位上限，committed master 既有环境问题，与本改动无关）。

**回退：** `property` 为 `Option`，全 `None` 时行为与现状完全一致；feature-gate `property-deps`（**默认不开启**，作为可选归因增强）。

**目标：** 即便场景命令原地 splice 了（Phase 1），renderer 当前仍每帧 `prepare` 整张命令表成顶点再 draw（`prepare.rs`）。终极形态要求只重新生成 + 上传变化命令对应的那一段顶点。

**核心思路：** vertex pool 已三缓冲（见记忆 `render-hotpath-optimizations`）。建立 `widget_id / command_index → 顶点区间` 映射，缓存上一帧的 `PreparedCommand` 列表；本帧只对「Phase 1 标记为脏的命令区间」重跑 prepare，其余顶点区间从上一帧缓冲复用（或仅做 transform uniform 偏移，见 Phase 4）。

**改动点：**
- `src/rendering/renderer/prepare.rs`：`prepare` 接收「脏命令集合」，未脏命令复用上帧 `PreparedCommand` + 顶点偏移。
- `src/rendering/renderer/vertex_pool.rs` / `vertex.rs`：支持区间级 `write_buffer`（部分上传）而非整缓冲重写；处理三缓冲下「哪几帧前的数据可复用」。
- `cached.computed` → renderer 的接口：传递脏区间信息（与 Phase 1 的 `command_spans` 对接）。

**风险：** 三缓冲 + 部分上传的帧间一致性极易出错（上传到正在被 GPU 读的缓冲、或复用了已失效的旧顶点）。记忆明确标注 vertex pool「未视觉验证」——本阶段必须先补视觉验证。

**验证：** 用 `/verify` 或 `skills/run` 实跑示例（`examples/canvas`、含动画的示例），肉眼 + 截图比对；`src/rendering/renderer/tests.rs` 补区间上传正确性。改色 / 改文本 / 滚动三种场景各验证一遍。

**回退：** feature-gate `incremental-upload`；关闭即回到整表 prepare（现状）。

#### 实施记录（2026-06-12）

落地了**可证安全、可单测的核心**：逐帧顶点池 flush 时的「脏区间增量上传」。范围比原计划保守——没有引入 `widget_id/command_index → 顶点区间` 映射，也没有跨帧复用 `PreparedCommand`（这两项与 Phase 1 的 `command_spans` 对接、且强依赖 GPU 视觉验证）；而是在**已有的整段上传出口**处做字节级 diff，把「每帧整表 `write_buffer`」压成「只上传变化区间」。这一层独立成立、对上层零侵入，且正确性可在无 GPU 环境穷举单测。

- `src/rendering/renderer/vertex_pool.rs`：`VertexBufferPool` 新增 `last_uploaded: [Vec<u8>; 3]`——各轮转缓冲「上次写入 GPU 的字节镜像」。`flush` 在 `incremental-upload` 开启时：① 缓冲刚扩容重建（GPU 内容未定义）→ 整段写并重置镜像；② 否则用纯函数 `dirty_range(new, old)` 求本帧 staging 相对镜像的变化区间 `[start,end)`，按 `COPY_BUFFER_ALIGNMENT` 对齐后只 `write_buffer` 这一段，完全相同则**跳过上传**；③ 无论整写还是区间写，写后把镜像 `clone` 为本帧 staging——保证镜像与 GPU 缓冲 `[0..write_len]` **逐字节一致**。关闭特性时编译为原来的整段 `write_buffer`，逐字节透明。
- **三缓冲安全性：** 镜像按槽位（`current`）维护，第 N 帧只与「同一槽位上次写入的内容」（即 N−3 帧）做 diff、只覆盖该槽位。该槽位的 GPU 数据此刻已被 GPU 读完（在途帧 ≤ 3），对它部分覆盖无读写竞态——与原整写的安全性论证一致。
- **渲染等价性：** 写后缓冲 `[0..write_len]` 与本帧 staging 逐字节相同，draw 阶段按本帧 `allocate` 返回的 offset/count 取 slice，绝不读取 `write_len` 之外的陈旧尾字节——故输出与整写**完全一致**。

**测试（`dirty_range_tests`，`incremental-upload` 门控）：** 7 个纯函数单测穷举边界——完全相同→`None`（跳过）、空镜像→整段、单字节中间变化、首末 diff 之间连续覆盖、变长尾部追加（含前缀也变的情形）、变短只在公共区间内变化、变短但公共前缀全同→`None`。全部通过。

> ⚠️ **GPU 视觉验证未完成（关键遗留）：** 路线图明确要求本阶段「用 `/verify` / `skills/run` 实跑示例，改色/改文本/滚动三种场景肉眼 + 截图比对」。本机无 GPU 设备、renderer 需真实 adapter 无法 headless 构造，该验证**未能在本环境完成**。`dirty_range` 逻辑已被穷举单测覆盖，且整写/区间写的 GPU 字节等价性有论证，但「设为默认开启」必须等真实硬件视觉验证通过后再决定。当前 `incremental-upload` **默认关闭**。

**回退：** feature-gate `incremental-upload`（**默认关闭**）；关闭即回到整段 `write_buffer`（现状），逐字节透明。

---

### Phase 4 · 滚动 / 动画 transform-only 快路径（4–5 周，🔴 最高风险，最大收益）

**目标：** 干掉「每帧全量重收集」这条最偏离细粒度模型的路径（`scene_runtime.rs:194+`）。滚动只是平移、属性动画（opacity / transform / color）只改少量字段，本不该重收集整树。

> 注：附录 A.2 的实验数据显示，定高滚动容器下 collect 的**节点游走已是视口受限的**（recollect/visible ≈ 1.0），但整帧 wall 仍随树规模增长。因此本阶段的优化重心是**避免每帧重跑 layout / epoch 全量 pass**、用 transform-only 快路径绕开整条 collect，而非减少收集节点数；Container-arm 视口裁剪仍有价值，但属次要收益。Phase 0 重建度量后应先复核这一结论。

**核心思路：** 分两类：
1. **滚动**：滚动只改子树的可见偏移，不改任何 primitive 几何。引入「滚动 = 给该 scroll 容器子树的所有命令施加一个平移 uniform」，渲染时在 shader / draw 阶段按容器 uniform 偏移，不重收集、不重生成顶点。配合视口裁剪（记忆 `push-level-culling-measured` 指出真正的裁剪要在 Container-arm 做）。
2. **属性动画**：`animated()` 信号驱动的 opacity/transform/color，每帧只更新对应 primitive 的字段（接 Phase 2 属性槽 + Phase 3 区间上传），不走 collect。

**改动点：**
- `src/runtime/scene_runtime.rs`：epoch 路径分流——纯滚动 / 纯属性动画走新 transform-only 路径，结构变化才 recollect。
- `src/runtime/scene_patch_roots.rs`：`patch_animation_scene_widgets` 已是「按 widget patch」雏形，扩展为「仅更新 transform/uniform，不重收集 chunk」。
- shader（`rect.wgsl` / `mesh.wgsl` / `text.wgsl`）：支持每-draw transform uniform（滚动偏移）。
- Container-arm 视口裁剪：在 collect / draw 时跳过完全在视口外的子树（接记忆里已测的 push-level culling，补齐 Container 级）。

**风险：** 滚动 + clip + 嵌套滚动 + sticky/portal 的组合极其容易错位；动画与布局变化同帧发生时的优先级。这是全计划最难的一阶段，建议拆成「先滚动、后动画」两个子里程碑。

**验证：** `src/runtime/tests/` 滚动 / canvas / video 命中测试必须全过；实跑滚动示例测 jank（对照 Phase 0 的「重收集节点数 / 可见节点数」比值应趋近 1）；嵌套滚动 + overlay 专项测试。

**回退：** feature-gate `transform-only-scroll` / `transform-only-anim`，各自独立；任一组合不支持即回退整树 recollect。

#### 实施记录（2026-06-12）

落地了**可证等价、可单测的子里程碑「先滚动」的安全部分**，并对 GPU 可见的激进变体保持克制。关键认识改变了实现取舍：附录 A.2 与 collect 代码表明，滚动当前把 `-scroll_offset` **烘焙进每个子图元坐标**、并裁剪掉完全离屏的子节点（`should_skip_fully_clipped_child`）。因此「平移缓存子树图元」是**不正确的近似**——被裁掉的离屏子节点不在缓存 chunk 里，平移无法让它们重新出现。真正既正确又高覆盖的快路径不是「平移图元」，而是「**只重收集滚动子树**」：复用已存在且已测的 `patch_cached_scene_for_roots`，把作用域从整树收窄到滚动容器子树，用**同一个 collect 函数**重算几何，结果与整帧重收集**逐项等价**。

- `src/runtime/mod.rs` / `input/interaction.rs`：新增 `scroll_dirty_widgets: HashSet<WidgetId>`，在唯一的滚动写入口 `set_scroll_offset` 累积发生过偏移变化的容器；纯滚动快路径命中后清空，整帧重收集后也清空（已被全树重算覆盖）。
- `src/runtime/cache_support.rs`：把 `scene_cache_matches` 拆出 `scene_cache_fields_match_ignoring_scroll`（除 `scroll_epoch`/`computed_valid` 外所有匹配字段），供快路径复用，避免字段列表漂移。
- `src/runtime/scene_runtime.rs`：`try_pure_scroll_fast_path`——仅当本帧失配**只有** `scroll_epoch`（其余字段全匹配、`layout_valid`、不含 virtual）时，取实际发生滚动且仍在树中的脏容器，经 `highest_layout_roots_smallvec` 取最高根（**嵌套滚动安全**：祖孙同脏只取最高根，重收集整棵子树含内层），调 `patch_cached_scene_for_roots(roots, now, /*sync_runtime_scene_state*/ true)` 完成子树重收集 + Phase 1 splice，并同步 `scroll_epoch` 等运行时状态。任一前置不满足 / patch 失败即返回 false，落回 `computed_scene` 下方的整帧重收集。接入点在文本输入 patch 之后、`!cache_valid` 整帧分支之前。
- **virtual 容器排除**：virtual 滚动会改变窗口化 plan（结构性），`layout.contains_virtual()` 时直接不走快路径。

**正确性依据：** 快路径与整帧重收集调用**同一个收集函数**，仅作用域不同；`patch_resolved_roots` 用最新 `scroll_states` 重解析子树几何（含 `child_origin = frame − scroll_offset` 平移与离屏裁剪），故对滚动子树逐项等价，对树其余部分（未变）保持缓存。这不是近似，是「只重算受影响子树」的精确等价。

**测试（`pure_scroll_patch_matches_full_recollect`，`specialized_patch_tests.rs`）：** 构造内容溢出、可纵向滚动的容器（三个色块），`set_scroll_offset` 后求场景，断言：① 探针确认**确实命中纯滚动快路径**恰一次（避免假阳性「只验证了回退」）；② 快路径结果与随后强制的一次从零全量重收集**逐项等价**。特性关闭时同一测试经整帧重收集路径通过（探针断言被 cfg 关掉），证明降级链正确。

> ⚠️ **GPU 可见变体未实施 + 收益边界：** 路线图原描述的「给滚动子树施加一个**平移 uniform**、shader/draw 阶段按容器偏移、收集不剔除、Container-arm 视口裁剪」是最高风险的 GPU 可见改动，且需真实硬件视觉验证（嵌套滚动 + clip + portal 错位极易出错）——本环境无 GPU，**未实施**。已落地的子树重收集快路径**避免了整树重收集**（O(滚动子树) 而非 O(总节点)），但仍会重跑该子树的 collect；它不消除 collect、而是收窄 collect 作用域。属性动画 transform-only（`transform-only-anim`）同理依赖 GPU 验证，**未实施**。

**回退：** feature-gate `transform-only-scroll`（**默认关闭**）；任一前置不满足即回退整树 recollect，行为与未开启时逐字节一致。

---

### Phase 5 · 收尾、文档与发布（1–2 周）

- 统一三个 feature gate 的默认值决策：性能路径默认开、保留关闭逃生口。
- 更新 `AGENTS.md`（架构章节）、`CLAUDE.md`（高危区说明）、`README.md`（如有公开 API 变化）、`docs/` 新增「增量渲染管线」说明。
- 全 feature 矩阵检查：`cargo check` / `--features audio` / `video` / `video-static`。
- 跑 `publish.bat` 流程前确认本文件与新增 doc 已加入 `Cargo.toml` 的 `exclude`（避免进 crate）。
- 性能回归基线：Phase 0 的 bench 对比，单属性更新应从「子树 + 祖先重合成 + 整表 prepare」降到「单区间 splice + 单区间上传」；滚动应从「整树重收集」降到「transform uniform」。

#### 实施记录（2026-06-12）

收尾阶段四件事均落地，并贯彻「默认值决策 = 只把可证安全、已全量单测的快路径设默认，GPU 可见 / 需真机验证的保持默认关闭」的原则。

**① feature gate 默认值定稿。** `Cargo.toml` `default = ["fine-grained-splice"]`：
- `fine-grained-splice`：**默认开启**。它是 CPU 可证安全的纯连接合成模型快路径，`cached.computed` 与 recompose 路径逐字节等价，开关两态各有单测，无 GPU 依赖——满足「性能路径默认开、保留逃生口（`--no-default-features`）」。
- `property-deps` / `incremental-upload` / `transform-only-scroll`：**默认关闭**。`property-deps` 是为后续单属性直写预留的归因增强，开启不改当前失效粒度（消费侧仍按 widget 失效），作为可选项更稳妥；`incremental-upload` 的 GPU 字节上传路径与 `transform-only-scroll` 的 GPU 可见变体均需真实硬件视觉验证（本机无 GPU），按路线图风险登记必须「先补视觉验证再设默认」，故保持 opt-in。

**② 文档同步。**
- `AGENTS.md`：修正过期的 `default = []` → `default = ["fine-grained-splice"]`，新增「细粒度响应式渲染管线」整节（四条快路径的目标 / 兜底红线 / 代码锚点 / feature 状态），并把 `exclude` 维护注记更新为含 `docs/*` 与本文件。
- `CLAUDE.md`：新增「细粒度响应式渲染管线（高风险区）」节与 feature 矩阵检查命令；修正 `exclude` 说明与失效的 `docs/canvas.md` 引用（实际为 `docs/features/canvas.md`），补 `docs/advanced/performance.md` 引用。
- `README.md`：可选 feature 列表新增性能 feature 段。
- `docs/advanced/performance.md`：新增「增量渲染管线（细粒度响应式）」节（feature 对照表 + 何时开启 + 状态）。
- `CHANGELOG.md`：`[Unreleased]` 新增 `### Added` 段记录四条快路径，`### Changed` 记录 `exclude` 变更。

**③ crate 打包清单核对。** 此前 `cargo package --list` 显示**整套 vitepress 文档站点**（`docs/package.json` / `pnpm-lock.yaml` / `.vitepress/config.mts` / 17 个 `.md`）与本路线图都会进 crate。已在 `Cargo.toml` `exclude` 新增 `docs/*` 与 `FINE_GRAINED_ROADMAP.md`，打包清单从 584 → 562 文件，`docs/` 与 ROADMAP 归零，`README.md` / `CHANGELOG.md` 保留。

**④ 全 feature 矩阵 + 测试验证。** `cargo check` 全绿：默认 / `--no-default-features` / `audio` / `video` / `video-static` / `property-deps` / `incremental-upload` / `transform-only-scroll` / 三 gate 同开 / `--no-default-features` + 三 gate 同开。`cargo fmt --check` 干净。全套测试经 `CARGO_PROFILE_DEV_DEBUG=0 cargo test` 通过、0 回归（沿用前几阶段的 macOS Mach-O 重定位上限规避，committed master 既有环境问题）。

> 性能回归基线对比项（路线图验收标准 1–3）中，「单区间 GPU 上传」与「滚动 transform uniform」依赖 `incremental-upload` 的 GPU 路径与 `transform-only-scroll` 的 GPU 可见变体，二者均待真机视觉验证，故这部分客观判据在本环境**未能闭环**；已落地部分（splice 跳过祖先重合成、纯滚动只重收集子树）的 CPU 侧等价性已由单测覆盖。

## 3. 里程碑与依赖关系

```
Phase 0 (度量) ──┬─> Phase 1 (命令区间 splice) ──┬─> Phase 3 (GPU 区间上传) ──┐
                 │                                │                          ├─> Phase 4 (滚动/动画 transform-only) ─> Phase 5
                 └─> Phase 2 (属性级依赖) ────────┘                          ┘
```

- Phase 1 与 Phase 2 可并行（分属渲染侧 / 绑定侧），但 Phase 3 需要 Phase 1 的 `command_spans`，Phase 4 需要 Phase 2 的属性槽 + Phase 3 的区间上传。
- 任何阶段都可独立停下发布——每阶段都带 feature gate 和回退路径。

## 4. 风险登记（高危区清单）

| 风险 | 阶段 | 缓解 |
|---|---|---|
| splice 破坏 z-order / clip 嵌套 | 1 | 严格区分视觉/结构命令；数量不一致即回退 recompose |
| 属性级依赖漏更新（最危险） | 2 | `property: Option`，未识别属性退化为整 widget 失效；全 UI 测试 0 回归 |
| 三缓冲部分上传读写冲突 | 3 | 先补 vertex pool 视觉验证（记忆标注未验证）；feature gate |
| 滚动/嵌套 clip/portal 错位 | 4 | 拆「先滚动后动画」；嵌套场景专项测试；可回退整树 recollect |
| 媒体/video feature 下编译 | 全 | 每阶段跑 `--features video` check（Windows 需 FFmpeg 链接环境） |

## 5. 验收标准（终极形态达成的客观判据）

1. 改一个深层叶子的颜色：失效路径只触碰 1 个 primitive + 1 段顶点上传，**不重合成祖先链、不整表 prepare**（bench 可证）。
2. 滚动一个长列表：每帧重收集节点数 / 可见节点数 ≈ 1（而非现在的 ≈ 总节点数）。
3. 属性动画（opacity/transform）每帧不触发 scene collect。
4. 全部现有测试 + 新增测试通过；全 feature 矩阵 `cargo check` 通过；示例实跑无视觉回归。
5. 所有快路径都有对应的安全回退，且回退路径同样被测试覆盖。

## 附录 A · 基线数据

> Phase 0 已实施（2026-06-12）：度量工具已落地仓库，下列数字为正式采集的基线。

### A.1 度量护栏设计（Phase 0 · 已实施）

三件工具均已落地仓库：

- **单属性更新基准** `benches/single_property_update.rs`（需 `bench-support`）：n=1k/10k 树，
  最深叶子挂在 `opacity` 受 `State` 驱动的容器下，对比两条路径：
  - `single_leaf_patch` —— `set()` 改深层叶子视觉属性时走的子树 patch（重收集该叶子 chunk +
    沿祖先链 `recompose_scene_chunk` 向上合成到根，即运行时 `scene_subtree_patch` 的核心成本）。
    支撑入口：`WidgetBenchmarkContext::patch_single_deep_leaf_scene`（`bench_support.rs`），
    内部找最深叶子后复用 `patch_scene_roots`。
  - `full_recollect` —— 滚动/动画当前每帧的整树重收集（对照上界）。
  - 跑：`cargo bench --features bench-support --bench single_property_update`
- **失效 action 命中计数器** `src/runtime/action_stats.rs`：`request_redraw_if_dirty` 调用
  `invalidate_cached_scene_for_dependencies` 后记录其返回的 action 标签（`scene_subtree_patch` /
  `global_full_rebuild` / `text_input_scene_patch` / …）。关闭 `bench-support` 时编译成
  `#[inline(always)]` 空操作（热路径零成本）；带 `bench-support` 时用线程局部计数，in-crate 测试可
  `reset()` / `snapshot()` 读出各 action 命中分布。测试 `action_stats_records_scene_subtree_patch_for_color_change`
  （`specialized_patch_tests.rs`，`bench-support` 门控）断言「单深层叶子改色 → 恰好一次 `scene_subtree_patch`」。
- **重收集节点数 / 可见节点数比值** `src/ui/widget/core/collect_profile.rs` 的 `record_node_visible`
  （节点 frame 与 `context.viewport` 相交即记一次），与既有 `record_node`（重收集节点总数）配对。
  `profile_recollect_breakdown`（`cargo test --features collect-profile profile_recollect_breakdown -- --ignored --nocapture`）
  输出新增 `recollect/visible` 一栏：比值≈1 表示重收集贴近可见集合，远大于 1 表示大量浪费在视口外子树。

### A.2 参考数字（2026-06-12 正式采集）

> 环境：macOS（release / `cargo bench`），同一台开发机。下列为本次 Phase 0 工具落地后采集的基线，
> 后续阶段以此为对照。

**单属性更新（`single_property_update` bench，criterion 中位数）：**

| n（节点规模） | `single_leaf_patch`（子树 patch + 祖先重合成） | `full_recollect`（整树重收集） | patch 相对 recollect |
|---|---|---|---|
| 1 000 | ~230.7 µs | ~1.655 ms | 快 ~7.2× |
| 10 000 | ~832.8 µs | ~3.117 ms | 快 ~3.7× |

> 关键观察：`single_leaf_patch` 从 ~231µs（n=1k）涨到 ~833µs（n=10k）—— 节点 ×10、耗时 ×3.6，
> **仍随树规模超线性增长**（理想 O(1) 应近乎持平）。成本来自祖先链 `recompose_scene_chunk` 逐级
> `extend` 合成到根。**这正是 Phase 1 命令区间 splice 要消除的成本** —— 目标是把 `single_leaf_patch`
> 压平到与树规模无关。（注：本次绝对值与旧实验会话的 ~39µs/~742µs 不可直接比较——树形态、
> 测量方法、机器状态均不同；方向性结论「超线性、祖先链是主因」一致。）

**重收集节点数 / 可见节点数比值（`profile_recollect_breakdown`，scroll 树）：**

| n | 重收集节点数 | 可见节点数 | 比值 | 整帧 wall |
|---|---|---|---|---|
| 200 | 142 | 142 | 1.00 | ~2.5 ms |
| 1 000 | 142 | 142 | 1.00 | ~4.0 ms |

> 关键观察：定高滚动容器场景下，scene **collect 的节点游走已是视口受限的**（n=200 与 n=1000
> 收集与可见节点数都恒为 142，比值=1.00）—— 收集阶段已只走可见子树。但整帧 wall 仍随 n 增长
> （~2.5ms → ~4.0ms），说明 Phase 4 的收益重心不在「减少收集节点数」，而在**避免每帧重跑
> layout / epoch 全量 pass**、用 transform-only 快路径绕开整条 collect。此结论与旧实验会话一致，
> 现已用仓库内工具复现确证。



