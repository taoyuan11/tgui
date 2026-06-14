# 性能优化建议

日期: 2026-06-14  
依据: `docs/advanced/benchmark-results.md`

## 优先级 0：先让基准可信

第一项“优化”不是改热路径，而是把基准体系做可靠。

- 保持所有 Criterion target 都在 `Cargo.toml` 中声明 `harness = false`。
- 把 `cargo check --features bench-support --benches` 加入 CI。
- 涉及 `src/runtime/`、`src/ui/widget/core/`、文本输入、Canvas、媒体、音频或视频的性能 PR，至少要求跑对应的 `cargo bench --features bench-support --bench <target>`。
- 逐步替换旧的 no-op / 合成 helper。尤其是 `text_processing`、`event_handling`、`scene_rendering` 和 `widget_core_layout` 的部分结果过小，不能直接代表真实框架性能。

## 优先级 1：修复 Scene Patch 的可基准性

`WidgetBenchmarkContext::patch_single_deep_leaf_scene` 当前在 Windows 的 Criterion warm-up 阶段触发栈溢出。这阻塞了细粒度 scene patch 路线图里最关键的“patch vs full recollect”对照基准。

建议：

- 增加一个小型单元测试或 ignored test，在最浅复现树上单次调用 `patch_single_deep_leaf_scene`。
- 检查 `deepest_leaf_id` 是否应避免重复递归扫描 `subtree_widget_ids`，改用 `parents/depths` 等 metadata 定位叶子。
- 审计 `patch_scene_roots` 与 `recompose_scene_chunk`，排查 cached scene patch 后是否出现自组合或重复增长。
- 修复后恢复 patch 与 full recollect 的 Criterion 对照，并记录预期加速比。

## 优先级 2：降低全量布局成本

真实 widget 基准显示，全量 layout+scene 成本很高：

- 50 rows: median 9.222 ms
- 200 rows: median 25.243 ms
- 500 rows: median 59.785 ms

scene-only recollect 更平缓：

- 50 rows: median 3.271 ms
- 500 rows: median 3.727 ms

这说明数据密集视图的主要优化目标应放在 layout invalidation 半径上。

建议：

- 局部改动优先 patch layout root，而不是整树布局。
- 验证常见状态变更在只影响文本颜色、透明度、transform 或 paint 时能被归类为 scene-only。
- 对长列表和表格，引导用户与内部组件使用虚拟化和可见窗口收集。
- 新增“500/1000 行树中只修改单行”的基准，对比 full layout、layout-root patch 和 scene-only recollect。

## 优先级 3：调查 Canvas 命中查询成本

Canvas scene 构建速度线性且较快：

- 1000 items: median 467.788 us

但 point query 明显更高：

- 50 items: median 5.252 ms
- 1000 items: median 6.093 ms

这个弱扩展特征暗示存在固定初始化成本，可能来自默认 query context、字体上下文或文本命中准备。

建议：

- 对重复查询缓存 `CanvasSceneQueryOptions` 或底层 font manager/context。
- 增加 path-only 查询模式，在调用方不需要文本 cluster 信息时跳过文本 hit shaping。
- 在昂贵的文本/路径 hit 逻辑前增加 cheap bounds-first fast path。
- 对大场景考虑可选 spatial index，用于高频 pointer move hit test。

## 优先级 4：保持 MediaBytes 零拷贝路径

媒体源在共享数据时表现很好：

- Clone/hash of a 4 MiB `MediaBytes`: median 15.437 ns
- Creating `MediaBytes` from a new 4 MiB `Vec`: median 1.020 ms

建议：

- 已有数据时优先使用 `MediaBytes::from_shared(Arc<[u8]>)`。
- 避免在 media loader、canvas image 和 video frame plumbing 中重复复制 `Vec<u8>`。
- 在用户文档中说明缓存或资源包加载图片时的零拷贝路径。

## 优先级 5：关注音视频队列扩展性

测得的音频输出 callback helper 处于微秒级，状态较好：

- `audio_output_write_f32/2048`: median 1.989 us
- `audio_output_write_i16/2048`: median 2.812 us

视频队列 accounting 与 compressed byte distribution 约为线性：

- `video_queue_accounting/1200`: median 7.999 us
- `video_compressed_byte_distribution/1200`: median 10.623 us

建议：

- 保持 audio callback 路径无分配，并避免 callback 内额外加锁。
- 如果 video queue 增长到数千帧以上，维护聚合 memory/count metadata，避免扫描。
- 增加 4K / high-FPS buffering 压力基准，使用更接近真实的 frame memory size。

## 优先级 6：调 scene collection 前先画像

`real_widget_text_heavy_scene_recollect/250` 约 1.041 ms，而 dashboard scene-only recollect 约 3.7 ms。修改 collection 内部前，应先用 `collect-profile` 拆分 visual state、surface、text 和 bookkeeping 的耗时。

建议命令：

```powershell
cargo test --features collect-profile profile_recollect_breakdown -- --ignored --nocapture
```

然后优先优化最大阶段。没有 phase 数据和等价性测试前，避免对 `src/ui/widget/core/` 做推测性重写。

## 优先级 7：清理旧基准语义

若干旧基准仍在测 helper 近似实现，而不是真实框架行为。

建议替换方向：

- 用公开 `TextController` 操作加真实 text scene recollect 基准替换 `text_processing` 的 shaping/layout helper。
- 用 runtime-level hit region 和 command dispatch hook 替换 no-op event dispatch helper。
- 用 `WidgetBenchmarkContext::run_layout_and_scene` 或 renderer-preparation hook 替换合成 scene primitive collection。
- 只有在明确标注为数据结构微基准时才保留 synthetic microbench。
