# 性能与资源指南

本文档汇总 `tgui` 在性能与资源管理方面的设计目标、当前 benchmark 基线，以及常见调优手段。`PRODUCTION_READINESS.md` 第八章会引用本文。

## 1. Benchmark 基线

仓库 `benches/` 目录含 9 个 Criterion benchmark，按 feature gate 划分：

| Bench                  | Feature gate              | 关注点                                      |
| ---------------------- | ------------------------- | ------------------------------------------- |
| `state_signal`         | `bench-support`           | `State` / `Signal` 写入、派发与求值         |
| `widget_core_layout`   | `bench-support`           | `taffy` 布局 + scene 收集                   |
| `animation_engine`     | `bench-support`           | 时间线动画与每帧 refresh                    |
| `theme_resolution`     | （无）                    | `ThemeSet` 解析、Stateful 求值              |
| `text_controller`      | `bench-support`           | Rope 缓冲区编辑、IME 边界                   |
| `canvas_scene`         | （无）                    | Canvas tessellation 与命中                  |
| `color_interpolation`  | （无）                    | 主题颜色过渡的浮点路径                      |
| `audio_pipeline`       | `bench-support, audio`    | FFmpeg 解码到内存采样                       |
| `video_pipeline`       | `bench-support, video`    | FFmpeg 解码到 RGBA 帧                       |

### 1.1 当前观察值（参考，非 SLA）

下表是 2026-05-18 在 Windows 11 / 高性能桌面 CPU 上跑 `widget_core_layout` 的代表性中位数（`--warm-up-time 1 --measurement-time 3 --sample-size 10`）：

| 场景                                              | 中位耗时 |
| ------------------------------------------------- | -------- |
| `many_widgets_layout/layout_only/200`             | ~11 µs   |
| `many_widgets_layout/layout_and_scene/200`        | ~29 µs   |
| `many_widgets_layout/layout_only/1000`            | ~17 ms   |
| `many_widgets_layout/layout_and_scene/1000`       | ~38 ms   |
| `text_heavy_layout/layout_only/many_short_lines`  | ~2.1 µs  |
| `text_heavy_layout/layout_and_scene/many_short_lines` | ~5.5 µs |
| `text_heavy_layout/layout_only/few_long_blocks`   | ~370 ns  |
| `text_heavy_layout/layout_and_scene/few_long_blocks` | ~800 ns |
| `animated_scene_recompute/animated_visual_only`   | ~9 µs    |
| `animated_scene_recompute/animated_layout_affecting` | ~10 µs |

注意：`many_widgets_layout/1000` 节点构造了 1000 张卡片，每张卡片含 5 个子节点（约 5k 实际 widget），单帧 layout + scene 已经超过 16.6 ms 的 60 FPS 预算。这是有意的压力测试场景，用于回归检测；常见业务页面节点数远少于此。

### 1.2 性能目标（SLA 草案）

下面是 0.x 阶段尝试维持的目标：

- **60 FPS 单帧布局 + scene 收集**（`many_widgets_layout/layout_and_scene`）
  - 200 节点：< 200 µs
  - 1000 节点：< 30 ms（因布局复杂度升级，60 FPS 不再要求严格 ≤ 16.6 ms）
- **动画 refresh**（`animated_scene_recompute`）：单根更新 < 50 µs，patch 路径不应分配新的整树 scene。
- **文本编辑**（`text_controller`）：`insert_str` / `replace_byte_range` / `snapshot` 在 100 KB rope 上 < 50 µs。
- **状态写入**（`state_signal`）：`State::set` + `Signal::value` 一轮 < 200 ns（无依赖订阅）。

任何 PR 让某项 benchmark 中位耗时退步 > 20% 都需要在描述里说明原因。CI 的 `bench-publish` job（PRODUCTION_READINESS 第二章）已为接 Bencher.dev 留出钩子；启用后回归会出现在 PR review。

## 2. 资源预算（`ResourceBudget`）

`tgui::application::ResourceBudget` 暴露了几个 LRU 缓存的容量上限，可以通过 `Application::resource_budget(...)` 注入。字段如下：

| 字段                            | 默认值 | 含义                                          |
| ------------------------------- | ------ | --------------------------------------------- |
| `canvas_shadow_cache_entries`   | 16     | Canvas 阴影离屏纹理缓存                       |
| `widget_shadow_cache_entries`   | 24     | 普通 widget 阴影离屏纹理缓存                  |
| `image_raster_cache_entries`    | 8      | 单个位图文档保留的多分辨率纹理数               |
| `svg_raster_cache_entries`      | 4      | 单个 SVG 文档保留的多分辨率纹理数              |

任意字段为 `0` 时禁用对应缓存（每次都会重新生成纹理，仅适合内存极度紧张的场景）。`ResourceBudget::compact()` 提供一组适合内存受限环境的紧凑值。

```rust
use tgui::application::{Application, ResourceBudget};

Application::new()
    .resource_budget(ResourceBudget::compact())
    .with_view_model(MyVm::new)
    .root_view(MyVm::view)
    .run()
```

不暴露的内部缓存（`computed_scene` 缓存、字体 shaping 缓存、`taffy` 布局节点、动画引擎的 `WidgetId` 表）随 widget 树规模和窗口生命周期自然增长，没有 LRU；它们会在 widget 不再出现时随 invalidation 流程释放。

### 2.1 GPU 内存影响

每条阴影 / 位图缓存条目对应一张 `wgpu::Texture`（RGBA8）。最坏情形粗略估算：

```
canvas_shadow + widget_shadow 默认上限 = 40 张
单张最大尺寸 ≈ 2048 × 2048 × 4 字节 = 16 MiB
理论上限 ≈ 40 × 16 MiB ≈ 640 MiB
```

实测多数纹理远小于 2048²；但在富 Canvas 或高 DPI 屏上调小 `canvas_shadow_cache_entries` 仍是有效手段。`MAX_IMAGE_DIMENSION = 2048`（见 `src/media/types.rs`）会先把过大的图片缩到 2048 长边再纹理化。

## 3. 冷启动

通过环境变量 `TGUI_PROFILE_STARTUP=1` 启动后，第一帧渲染成功（`RenderStatus::Rendered`）时会输出一条 `tgui-startup` tag 的 INFO 日志：

```
[INFO][tgui-startup] first_frame took 142.318ms window_key=main
```

这里 "0 时刻" 是 `BoundRuntimeHandler::new` 被构造的瞬间（即 `Application::run()` 进入事件循环的第一阶段），覆盖了：

1. winit 事件循环的 `Resumed` 派发；
2. `Renderer::new` 创建 `wgpu::Surface` / pipeline；
3. 首帧 `sync_bindings` → `dispatch_media_events` → `computed_scene` → `renderer.render`。

**目标**：默认主题、空 ViewModel、单窗口的桌面冷启动 < 200 ms。第三方字体或大量初始位图会显著推高这个值。

## 4. 空闲帧 CPU

`tgui` 使用 winit 的 `ControlFlow::Wait`（或 `WaitUntil(deadline)`）来挂起事件循环，无事件 / 无动画 / 无 caret 闪烁时事件循环不会空转。具体规则在 `src/runtime/timing.rs::drive_animations` 里：

- 有动画 / smooth scroll / pending click / caret blink / key repeat 时，`WaitUntil(next_deadline)`；
- 其它时刻，`Wait`。

如果在 `cargo flamegraph` / `dtrace` 下观察到空闲时仍有持续 CPU 占用，可优先排查：

- 用户业务在 `Command` / `ValueCommand` 里循环 `State::set`；
- `Signal::map` 求值器内部读了高频源（如 `Instant::now()`）；
- 第三方插件 / hook 在 winit 事件中 `request_redraw`。

## 5. 大尺寸内容

- **`Image` 8K+ 纹理**：被 `clamp_raster_request` 限制到 2048 长边，再走 `image` crate 解码。需要超过该阈值时改 `MAX_IMAGE_DIMENSION` 不向前兼容，建议自己提前裁切再注入 `MediaSource::Bytes`。
- **`Textarea` 百万字节**：`ropey::Rope` 提供 O(log n) 的随机插入，文本布局走 `cosmic-text` 的 viewport 增量 shape；可见区域之外不参与 paint。`text_input_layout_snapshot` 会在编辑时增量 patch，而非整段重新 layout。
- **多窗口**：每个窗口持有独立的 `Renderer`（独立 `Surface`、独立 pipeline 集合），但 `wgpu::Device`/`Queue` 在同一进程里共享。窗口越多 GPU 内存越线性增长；目前没有共享 device 的 frame pacer，多窗口之间的 vsync 由各自 surface 的 `Queue::submit` 独立驱动。

## 6. 调优清单

1. **在 widget 树里有大量 1000+ 同质节点**：考虑虚拟滚动列（roadmap 里的 `Table` widget）或自建 `Canvas` 走 retained scene。
2. **频繁 invalidation**：检查是否有 `Signal::map` 链路依赖了 `State` 之外的可变全局；用 `TGUI_TEXT_PROFILE=1` 打开内置 profiler 看哪一段耗时主导。
3. **GPU 内存吃紧**：`ResourceBudget::compact()`，或把 `widget_shadow_cache_entries` 调到 4–8。
4. **首屏慢**：`TGUI_PROFILE_STARTUP=1` 打开，定位是字体 / 字体回退、网络图片预热、初始化主题，还是渲染 pipeline 编译。
