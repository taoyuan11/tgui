# 性能与资源

本文汇总 `tgui` 的性能观察点、资源预算和常见调优方向。这里的数字用于回归参考，不是稳定 SLA。

## Benchmark

仓库 `benches/` 目录包含 Criterion benchmark：

| Bench | 关注点 |
| --- | --- |
| `state_signal` | `State` / `Signal` 写入、派发与求值 |
| `widget_core_layout` | `taffy` 布局和 scene 收集 |
| `animation_engine` | 时间线动画与每帧 refresh |
| `theme_resolution` | `ThemeSet` 解析和状态值求值 |
| `text_controller` | Rope 缓冲区编辑和 IME 边界 |
| `canvas_scene` | Canvas tessellation 与命中 |
| `color_interpolation` | 主题颜色过渡 |
| `audio_pipeline` | FFmpeg 音频解码到内存采样 |
| `video_pipeline` | FFmpeg 视频解码到 RGBA 帧 |

运行示例：

```sh
cargo bench --bench widget_core_layout --features bench-support
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

`tgui` 的失效/渲染管线在「细粒度依赖跟踪 + 保留式分块场景图子树增量 patch」之上，提供一组**可独立开关、分级降级**的快路径。设计原则是保留现有的 pull 模型并缩短 pull 半径：任一快路径的前置条件不满足时，都会干净回退到「子树 patch → 整帧重收集」，绝不产生错误渲染。每条快路径都带「与全量重收集逐项等价」的单测和对应的回退测试。

| feature | 默认 | 作用 | 状态 |
| --- | --- | --- | --- |
| `fine-grained-splice` | **开启** | 叶子/子树 scene-only 改动（如改色）时，把新场景 chunk 原地拼接进根扁平场景与各祖先 chunk 的稳定区间，跳过祖先链向上重合成。命令数量或结构变化即回退。 | 可证安全，已全量单测 |
| `property-deps` | 关闭 | 属性级依赖归因：把 Signal 读取归因到具体视觉属性（背景 / 边框 / 不透明度 / 偏移 / 缩放 / 文本色），为单属性直写提供信息。未识别属性安全退化为整 widget 失效。 | 可选归因增强 |
| `incremental-upload` | 关闭 | 逐帧顶点池只上传相对上次写入发生变化的字节区间（triple-buffer 下安全），完全相同则跳过上传。 | CPU 逻辑已单测；GPU 视觉验证待真机完成后再决定默认 |
| `transform-only-scroll` | 关闭 | 纯滚动帧（仅滚动偏移变化）只重收集滚动子树而非整树（嵌套滚动取最高根、排除虚拟列表）。 | 与全量重收集逐项等价，已单测 |

何时开启：

- 绝大多数应用直接用默认即可——`fine-grained-splice` 已把「改一个深层叶子属性」的失效成本从「重收集子树 + 祖先链重合成到根」降为「单区间原地拼接」。
- `incremental-upload` / `transform-only-scroll` 是面向「长列表高频滚动 / 高频属性动画」的进阶开关，目前默认关闭、需要时显式开启；其中 `incremental-upload` 的 GPU 路径建议在目标硬件上实跑验证后再用于生产。
- `property-deps` 是为后续单属性直写预留的归因增强，开启不改变当前失效粒度（消费侧仍按 widget 失效），可安全开启而无回归风险。

> 性能数字与基线方法见仓库根目录的 `FINE_GRAINED_ROADMAP.md`（不随 crate 发布）；相关基准为 `single_property_update`（需 `bench-support`）。
