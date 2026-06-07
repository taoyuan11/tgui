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
