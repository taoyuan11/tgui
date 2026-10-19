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

## 2026-10-19：demo 媒体轮询与场景分配优化

环境：Apple M5、macOS 27.0.1、rustc 1.99.1；原版为 `51289d8a`，优化版为本次
工作区代码。改动包括媒体订阅分支裁剪、overlay buckets 延迟分配，以及 workspace dev
编译优化。下列两组对照先固定编译模式，区分算法收益和编译配置收益。

### 同为未优化 debug 的真实 demo 采样

`demo` 默认启用 audio/video。在 Forms 页面保持相同窗口尺寸，以同一点依次向下滚动
2 页、向下 1 页、向上 2 页；使用 `TGUI_TEXT_PROFILE=1 TGUI_TEXT_PROFILE_MIN_MS=2`。
优化版采样期间没有后台编译或测试。数字为日志记录的 CPU 阶段中位数：

| 指标 | 原版 | 算法优化后 |
| --- | ---: | ---: |
| 缓存命中帧的媒体轮询 | 15.155 ms | 0.757 ms |
| 纯滚动场景处理（`path=pure_scroll_patch`） | 32.704 ms | 21.059 ms |

媒体统计筛选成功渲染且 `computed_scene_ms < 1` 的帧，样本量分别为 117 / 50。
滚动样本量分别为 7 / 9，范围为 29.951–52.932 ms / 19.276–27.226 ms。
事件合并会导致同一输入序列产生不同样本量，因此这是交互采样，不是固定帧数 benchmark，
也不能据此换算实际 FPS。这次采样仍走 CPU 滚动子树重收集；未放宽 GPU 平移的裁剪、
IME、弹层或可见滚动条限制。

### 新 dev profile 的 demo 复测

项目 `opt-level = 1`、第三方依赖 `opt-level = 3`，保留调试符号和断言。在相同 Forms
页面及滚动序列下，纯滚动场景处理的中位数进一步降到 **8.057 ms**（20 个样本，
6.655–16.450 ms）。缓存命中帧的媒体轮询中位数为 **0.349 ms**（42 个样本）。
这组数字同时包含算法改动和编译优化的收益；采样阈值设为 `TGUI_TEXT_PROFILE_MIN_MS=0`，
以免漏掉较短事件。滚动、Select 弹层打开、选项命中和关闭均在实际 demo 中验证。

### 同为 release 的 Criterion 对照

成功完成的 `scene_rendering` 基准复用布局，测量 250 行普通组件的 CPU 场景重收集：

```sh
cargo bench -p tgui-benchmarks --features bench-support --bench scene_rendering -- \
  '^scene_recollect_cached_layout/250$' \
  --warm-up-time 1 --measurement-time 3 --sample-size 20
```

| 场景 | 原版估计值（区间） | 优化版估计值（区间） | 估计降幅 |
| --- | ---: | ---: | ---: |
| 250 行场景重收集 | 5.5640 ms（5.5497–5.5878） | 3.7004 ms（3.6885–3.7108） | 33.5% |

另一个 `widget_core_layout` 的 `widget_scroll_scene_recollect_cached_layout/200`
完成采样后，原版和优化版分别得到 454.94 μs / 361.22 μs，Criterion 报告约 20.6% 改善。
但该可执行程序随后都会运行未被过滤的 Pagination 校验，并在
`benches/widget_core_layout.rs` 的 `bench_pagination_default_options` 中以 `0 != 96`
失败。因此该 target 整体未通过，不能将它报告为成功的 benchmark 检查；以上主要结论使用
已完整通过的 `scene_rendering` 与 demo 采样。

### 正确性验证

- `cargo test -p tgui-runtime --lib -- --test-threads=1`：1,503 项通过。
- 带 `video` feature 的 `media_` 测试：31 项通过，包括媒体 phase 变化去重、动态订阅、
  Portal/Tooltip、关闭的 Modal 与虚拟容器回退。
- 新增的订阅裁剪测试用 1,000 个无关文本节点验证媒体查询不解析其样式，并与全树遍历对照。
- 新增 overlay 测试覆盖空场景、延迟分配前后的 cursor、delta/prefix 重合成和层内/层间顺序；
  全量测试同时覆盖纯滚动/属性拼接与从零重收集的等价性及回退路径。
- `cargo check -p tgui` 的默认、`--no-default-features`、audio、video、video-static 均通过。
- `cargo fmt --all --check` 与 `git diff --check` 通过。
