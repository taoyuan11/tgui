# 性能与资源

本文汇总 `tgui` 的性能观察点、资源预算和常见调优方向。这里的数字用于回归参考，不是稳定 SLA。

## Benchmark

仓库 `benches/` 目录包含 Criterion benchmark：

| Bench | 关注点 |
| --- | --- |
| `animation` | 动画 helper、插值和时间线相关路径 |
| `audio_output` | `audio` feature 下的音频输出 callback helper |
| `canvas_scene` | Canvas scene 构建、默认查询、geometry-only 查询和 debug export |
| `event_handling` | 真实 scene hit-region 扫描与 `Command` / `ValueCommand` 执行 |
| `media_source` | `MediaSource` / `MediaBytes` 构造、clone 和 hash |
| `real_widget_pipeline` | 真实 widget 树的 full layout+scene 与 scene-only recollect |
| `scene_rendering` | 真实 widget scene collect、scene-only recollect 和 hit metadata scan |
| `single_property_patch` | 深层叶子 scene patch 与整树 recollect 对照 |
| `state_signal` | `State` / `Signal` 写入、派发与求值 |
| `text_processing` | `Text` / `Textarea` 真实 layout、scene recollect 和 `TextController` |
| `video_buffering` | `video` feature 下的视频队列 accounting helper 与 high-FPS/4K 缓冲压力 |
| `widget_core_layout` | 真实 widget 树的 layout、局部 layout root patch、scene recollect 和 cached hit path |

运行示例：

```sh
cargo bench -p tgui-benchmarks --bench widget_core_layout --features bench-support
```

CI 至少编译所有 `bench-support` Criterion targets：

```sh
cargo bench -p tgui-benchmarks --no-run --features bench-support
```

性能相关 PR 若触及 `crates/tgui-runtime/src/runtime/`、`crates/tgui-runtime/src/ui/widget/core/`、文本输入、Canvas、媒体、音频或视频，需跑对应 target，例如：

```sh
cargo bench -p tgui-benchmarks --features bench-support --bench event_handling
cargo bench -p tgui-benchmarks --features bench-support --bench widget_core_layout
cargo bench -p tgui-benchmarks --features bench-support --bench canvas_scene
cargo bench -p tgui-benchmarks --features "bench-support audio" --bench audio_output
cargo bench -p tgui-benchmarks --features "bench-support video" --bench video_buffering
```

## ResourceBudget

`tgui::application::ResourceBudget` 暴露多个 LRU 缓存容量上限：

| 字段 | 默认含义 |
| --- | --- |
| `canvas_shadow_cache_entries` | Canvas 阴影离屏纹理缓存 |
| `widget_shadow_cache_entries` | 普通 widget 阴影离屏纹理缓存 |
| `image_raster_cache_entries` | 单个位图文档保留的多分辨率纹理数 |
| `svg_raster_cache_entries` | 单个 SVG 文档保留的多分辨率纹理数 |

内存受限环境可以使用紧凑配置：

```rust
Application::new()
    .resource_budget(ResourceBudget::compact())
    .with_view_model(MyVm::new)
    .root_view(MyVm::view)
    .run()
```

## 冷启动

设置 `TGUI_PROFILE_STARTUP=1` 后，第一帧渲染成功时会输出 `tgui-startup` 日志，用于观察事件循环恢复、renderer 初始化、binding 同步、scene 收集和首帧提交的耗时。

## 调优方向

- 大量同质节点：优先使用虚拟列表、表格或 Canvas retained scene。
- 频繁 invalidation：检查 `Signal::map` 是否读取了高频变化源。
- GPU 内存吃紧：降低 shadow 和 raster cache 容量。
- 首屏慢：定位字体、网络图片、主题初始化或 pipeline 编译成本。

## 增量渲染管线（细粒度响应式）

`tgui` 的失效/渲染管线在「细粒度依赖跟踪 + 保留式分块场景图子树增量 patch」之上，默认启用一组**分级降级**的快路径。设计原则是保留现有的 pull 模型并缩短 pull 半径：任一快路径的前置条件不满足时，都会干净回退到「子树 patch → 整帧重收集」，绝不产生错误渲染。每条快路径都带「与全量重收集逐项等价」的单测和对应的回退测试。

| 优化 | 作用 | 状态 |
| --- | --- | --- |
| 场景命令原地拼接 | 叶子/子树 scene-only 改动（如改色）时，把新场景 chunk 原地拼接进根扁平场景与各祖先 chunk 的稳定区间，跳过祖先链向上重合成。命令数量或结构变化即回退。 | 默认内置；可证安全，已全量单测 |
| 属性级依赖归因 | 把 Signal 读取归因到具体视觉属性（背景 / 边框 / 不透明度 / 偏移 / 缩放 / 文本色），为单属性直写提供信息。未识别属性安全退化为整 widget 失效。 | 默认内置 |
| 顶点脏区间增量上传 | 逐帧顶点池只上传相对上次写入发生变化的字节区间（triple-buffer 下安全），完全相同则跳过上传。 | 默认内置；CPU 逻辑已单测，真机视觉验证已通过 |
| 纯滚动 CPU 子树重收集 | 纯滚动帧（仅滚动偏移变化）只重收集滚动子树而非整树（嵌套滚动取最高根、排除虚拟列表）。 | 默认内置；与全量重收集逐项等价，已单测 |
| 纯滚动 GPU 平移 | 对满足严格前置条件的纯滚动帧保留滚动内容的离屏命令，并用 wgpu IMMEDIATES 在 draw 阶段平移 tagged draw，绕开滚动子树 collect。 | 默认内置；adapter 不支持或场景前置不满足时回退到 CPU 子树重收集 |

运行时降级：

- 场景命令原地拼接、顶点增量上传和属性归因不需要应用侧配置。
- 纯滚动 GPU 平移需要 adapter 支持 wgpu IMMEDIATES；不支持时自动走 CPU 子树重收集。
- virtual、嵌套滚动、overlay/portal、IME、可见 scrollbar、复杂 clip 或 composite 等前置不满足时会回退，结果仍与全量重收集一致。

> 性能数字的记录方法见 [基准测试结果](./benchmark-results.md)；细粒度路径重点使用
> `single_property_patch`、`animation_frame_pipeline` 和 `interaction_frame_pipeline`
>（需 `bench-support`）做同机 A/B 对照。
