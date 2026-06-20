# 基准测试结果

本文说明如何阅读和记录 `tgui` 的 Criterion benchmark 结果。仓库不把某一次机器上的数字写成稳定 SLA，因为 GUI 性能会受 CPU、GPU、驱动、窗口系统、字体缓存、编译模式和后台负载影响；这里的目标是建立可重复的对照方法。

## 运行基准

编译所有 benchmark：

```sh
cargo bench -p tgui-benchmarks --no-run --features bench-support
```

运行单个 benchmark：

```sh
cargo bench -p tgui-benchmarks --features bench-support --bench widget_core_layout
```

音频 / 视频相关 target 需要对应 feature：

```sh
cargo bench -p tgui-benchmarks --features "bench-support audio" --bench audio_output
cargo bench -p tgui-benchmarks --features "bench-support video" --bench video_buffering
```

Criterion 默认输出位于 `target/criterion/`。比较两次改动时，尽量在同一台机器、同一电源模式、同一 Rust toolchain、同一 feature 集下运行。

## Benchmark 覆盖范围

| Bench | 关注点 | 适合观察 |
| --- | --- | --- |
| `state_signal` | `State` / `Signal` 写入、派发与求值。 | 绑定系统和依赖跟踪变化。 |
| `event_handling` | hit region 扫描、命令执行。 | 交互事件路径改动。 |
| `widget_core_layout` | layout、局部 layout patch、scene recollect、cached hit path。 | 布局核心和组件树缓存。 |
| `real_widget_pipeline` | 真实 widget 树 full layout+scene 与 scene-only recollect。 | 用户界面综合回归。 |
| `scene_rendering` | scene collect、scene-only recollect、hit metadata scan。 | 渲染 primitive 收集。 |
| `single_property_patch` | 深层叶子 scene patch 与整树 recollect 对照。 | 细粒度响应式快路径。 |
| `text_processing` | `Text` / `Textarea` layout、scene recollect、`TextController`。 | 文本输入、选择、IME 和文本渲染。 |
| `canvas_scene` | Canvas scene 构建、查询和 debug export。 | Canvas retained scene。 |
| `media_source` | `MediaSource` / `MediaBytes` 构造、clone、hash。 | 媒体来源和共享 bytes。 |
| `animation` | 插值、动画 helper、时间线。 | 动画系统。 |
| `audio_output` | 音频输出 callback helper。 | `audio` feature。 |
| `video_buffering` | 视频队列 accounting 与缓冲压力。 | `video` feature。 |

## 记录模板

性能相关 PR 建议在描述中记录：

```text
环境:
- OS:
- CPU/GPU:
- Rust:
- feature:
- 命令:

结果:
- widget_core_layout: ...
- single_property_patch: ...
- real_widget_pipeline: ...

结论:
- 改动影响:
- 是否有回退:
- 是否需要后续真机视觉验证:
```

## 判断结果

Criterion 会给出均值、置信区间和 outlier 信息。评估时不要只看一次 run 的单个数字：

- 小于噪声范围的变化，不应作为性能结论。
- 触及运行时、布局或渲染核心时，至少对比相关 benchmark 和一个真实示例。
- 如果优化依赖场景前置条件，要同时验证快路径命中和回退路径。
- 如果数据改善但代码复杂度明显上升，需要说明正确性测试和回退策略。

## 与运行时快路径的关系

细粒度响应式渲染管线有多条内置快路径：场景命令原地拼接、属性级依赖归因、顶点脏区间上传、纯滚动 CPU 子树重收集、纯滚动 GPU 平移。相关性能观察主要看：

- `single_property_patch`：单属性 scene patch 与整树重收集对照。
- `widget_core_layout`：局部 layout 和 scene 缓存路径。
- `real_widget_pipeline`：真实组件树综合成本。
- `scene_rendering`：scene collect 与 hit metadata。

这些快路径都必须在前置条件不满足时干净回退。性能回归排查时，先确认是快路径未命中、命中后变慢，还是回退路径成本变化。
