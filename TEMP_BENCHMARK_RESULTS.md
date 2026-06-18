# 基准测试覆盖与结果

日期: 2026-06-14  
性质: 根目录临时文件，仅用于本轮人工查看；不属于 `docs/` 站点内容，可删除。  
平台: macOS 26.5.1 (25F80), Apple M5  
工具链: rustc 1.96.0, cargo 1.96.0  
优化前 baseline: `490adda8`  
优化后: 当前工作区 / `4ccffbe1`

## 结论

本轮重新跑了与本次优化直接相关的基准：

- `canvas_scene`: A/B 对比显示默认 point query 从毫秒级降到微秒级，50/200/1000 items 分别约快 453x / 110x / 24x。
- `single_property_patch`: deep-leaf scene patch 基本无显著变化；本次修改主要提升安全性和避免重复扫描风险，不改变主要 patch 成本。
- `widget_core_layout`: 新增“单行更新路径”对照，局部 layout-root patch 相比 full layout+scene 约快 9x。
- `video_buffering`: 新增 high-FPS / 4K 队列压力基准，队列 accounting 保持亚微秒级。

Criterion 使用 plotters backend；本机未安装 gnuplot。本次参数为 `--sample-size 10 --warm-up-time 0.2 --measurement-time 0.5`，适合看趋势，不作为发布级稳定跑分。

## 执行命令

```sh
CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench canvas_scene \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5 --save-baseline before

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench canvas_scene \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5 --baseline before

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench canvas_scene \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench single_property_patch \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5 --save-baseline before

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench single_property_patch \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5 --baseline before

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features bench-support --bench widget_core_layout \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5

CARGO_TARGET_DIR=/Users/sky/Desktop/Project/Rust/libs/tgui/target/bench-ab \
  cargo bench --features "bench-support video" --bench video_buffering \
  -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
```

备注: `canvas_scene --baseline before` 在完成旧基准可比项后，到新增 `canvas_scene_query_point_all_geometry_only` 时因旧 baseline 不存在该 case 而停止；随后已无 baseline 重跑当前版本以记录新增 case。

## Canvas A/B

旧版默认查询每次构造默认 query context / FontManager，存在明显固定成本。当前版本复用默认 `CanvasSceneQueryOptions`，点查询进入线性扫描本身的微秒级成本。

| Benchmark | 优化前 | 优化后 | Criterion 变化 | 约快 |
| --- | ---: | ---: | ---: | ---: |
| `canvas_scene_query_point_all/50` | 6.3669 ms | 14.058 us | -99.783% | 453x |
| `canvas_scene_query_point_all/200` | 6.3984 ms | 58.353 us | -99.090% | 110x |
| `canvas_scene_query_point_all/1000` | 6.6372 ms | 274.44 us | -95.899% | 24x |

当前版本完整 `canvas_scene` 代表结果：

| Benchmark | 当前估计值 |
| --- | ---: |
| `canvas_scene_build/50` | 16.168 us |
| `canvas_scene_build/200` | 61.556 us |
| `canvas_scene_build/1000` | 302.43 us |
| `canvas_scene_query_point_all/50` | 13.961 us |
| `canvas_scene_query_point_all/200` | 57.515 us |
| `canvas_scene_query_point_all/1000` | 274.21 us |
| `canvas_scene_query_point_all_geometry_only/50` | 13.801 us |
| `canvas_scene_query_point_all_geometry_only/200` | 57.654 us |
| `canvas_scene_query_point_all_geometry_only/1000` | 273.85 us |
| `canvas_debug_export_json/50` | 119.24 us |
| `canvas_debug_export_json/200` | 468.59 us |
| `canvas_debug_export_json/1000` | 2.3376 ms |
| `canvas_path_builder_cubic/1024` | 6.4728 us |

## Single Property Patch A/B

本轮 `deepest_leaf_id` 改为基于 parent/depth metadata 的 O(N) 叶子定位，并补充最浅复现测试。Criterion 显示 scene patch 本身无显著性能变化，说明主要成本仍在子树 scene collect 与祖先 chunk 重合成。

| Benchmark | 优化前 | 优化后 | 判断 |
| --- | ---: | ---: | --- |
| `single_property_deep_leaf_full_recollect/4` | 158.84 us | 156.10 us | 噪声内 |
| `single_property_deep_leaf_full_recollect/8` | 267.09 us | 262.53 us | 无显著变化 |
| `single_property_deep_leaf_full_recollect/16` | 552.96 us | 523.04 us | 改善约 5.4% |
| `single_property_deep_leaf_scene_patch/4` | 78.650 us | 77.463 us | 无显著变化 |
| `single_property_deep_leaf_scene_patch/8` | 105.14 us | 104.75 us | 无显著变化 |
| `single_property_deep_leaf_scene_patch/16` | 158.62 us | 157.43 us | 无显著变化 |

## Widget Core Layout

当前版本代表结果：

| Benchmark | 当前估计值 |
| --- | ---: |
| `widget_flat_full_layout/10` | 244.92 us |
| `widget_flat_full_layout/50` | 1.1187 ms |
| `widget_flat_full_layout/100` | 2.5371 ms |
| `widget_flat_full_layout/200` | 5.5975 ms |
| `widget_flat_full_layout/500` | 14.533 ms |
| `widget_full_layout_and_scene/50` | 4.0261 ms |
| `widget_full_layout_and_scene/200` | 14.755 ms |
| `widget_full_layout_and_scene/500` | 41.367 ms |
| `widget_scene_recollect_cached_layout/50` | 1.8906 ms |
| `widget_scene_recollect_cached_layout/200` | 7.4528 ms |
| `widget_scene_recollect_cached_layout/500` | 18.430 ms |
| `widget_scroll_scene_recollect_cached_layout/50` | 919.43 us |
| `widget_scroll_scene_recollect_cached_layout/200` | 968.61 us |
| `widget_scroll_scene_recollect_cached_layout/500` | 1.1297 ms |
| `widget_cached_scene_hit_path/50` | 919.46 ns |
| `widget_cached_scene_hit_path/200` | 3.4514 us |
| `widget_cached_scene_hit_path/500` | 9.0209 us |

新增“单行更新路径”对照：

| Rows | Full layout+scene | Layout-root patch+scene | Scene-only recollect | Patch vs full |
| ---: | ---: | ---: | ---: | ---: |
| 500 | 40.437 ms | 4.4824 ms | 18.730 ms | 9.0x |
| 1000 | 89.320 ms | 9.6140 ms | 38.039 ms | 9.3x |

## Video Buffering

当前版本代表结果：

| Benchmark | 当前估计值 |
| --- | ---: |
| `video_buffer_decision_helpers` | 4.4249 ns |
| `video_queue_accounting/30` | 94.363 ns |
| `video_queue_accounting/300` | 707.59 ns |
| `video_queue_accounting/1200` | 2.3893 us |
| `video_high_fps_queue_accounting/1080p_60fps_2s/120` | 258.71 ns |
| `video_high_fps_queue_accounting/4k_60fps_2s/120` | 252.27 ns |
| `video_high_fps_queue_accounting/4k_120fps_2s/240` | 504.88 ns |
| `video_pts_to_duration/time_base/1/1000` | 1.7021 ns |
| `video_pts_to_duration/time_base/1/90000` | 2.0440 ns |
| `video_pts_to_duration/time_base/1001/30000` | 1.6809 ns |
| `video_compressed_byte_distribution/30` | 142.65 ns |
| `video_compressed_byte_distribution/300` | 1.1019 us |
| `video_compressed_byte_distribution/1200` | 4.4135 us |

## 保留说明

- 本文件覆盖了旧的 Windows 结果，不再保留未重新运行的 `real_widget_pipeline`、`media_source`、`audio_output`、`state_signal`、`text_processing`、`scene_rendering`、`event_handling` 数字。
- 若后续需要完整发布基线，应在安静环境下提高 sample / measurement time，并一次性重跑全部 bench target。
- 本文件是根目录临时记录，查看后可删除。
