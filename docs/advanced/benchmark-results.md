# 基准测试覆盖与结果

日期: 2026-06-14  
平台: Windows 11 Pro 10.0.26200, AMD Ryzen 5 9600X, 6C/12T  
工具链: rustc 1.96.0, cargo 1.96.0

## 结论

当前 `benches/` 原本覆盖不够完整，也不够可靠。

本次发现并修复了几个基础问题：

- `Cargo.toml` 没有为 Criterion targets 显式设置 `harness = false`，`cargo bench` 会进入 libtest harness，而不是正常运行 Criterion。
- 旧 benchmark 存在编译错误：`text_processing` 的临时字符串引用和 `&f32` 转型、`event_handling` 的 helper 参数类型、`widget_core_layout` 的重复解引用、`animation` 的所有权使用。
- 多个旧 benchmark 是合成/占位 helper，不代表真实框架热路径。例如 `text_processing` 没有调用 cosmic-text，`event_handling` 多数 dispatch 为空操作，`scene_collection`/`hit_test` 存在皮秒级结果，说明被优化成接近空操作。
- 缺少真实 widget frame pipeline、Canvas scene、media source、audio output、video buffering 入口。

本次新增并跑通：

- `benches/real_widget_pipeline.rs`
- `benches/canvas_scene.rs`
- `benches/media_source.rs`
- `benches/audio_output.rs`
- `benches/video_buffering.rs`
- `benches/single_property_patch.rs` 的 deep-leaf full-recollect 对照和 scene patch 快路径

本次随后修复了 deep-leaf benchmark 的两个栈问题：`patch_single_deep_leaf_scene` 现在可在 Criterion warm-up/measurement 中稳定运行；deep-leaf full recollect 的构造也已可稳定覆盖 depth 16。

## 执行命令

```powershell
cargo check --features bench-support --benches
cargo bench --features bench-support --bench real_widget_pipeline -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench canvas_scene -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench media_source -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench single_property_patch -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench state_signal -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench widget_core_layout -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench scene_rendering -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench text_processing -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench animation -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features bench-support --bench event_handling -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features "bench-support audio" --bench audio_output -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo bench --features "bench-support video" --bench video_buffering -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
```

Criterion 使用 plotters backend；本机未安装 gnuplot。为节省时间，本次使用较短 sample/measurement 参数，适合作为本机趋势基线，不适合作为发布级稳定跑分。

## 真实 Widget 管线

| Benchmark | Median | Mean |
| --- | ---: | ---: |
| `real_widget_full_layout_and_scene/50` | 9.222 ms | 9.862 ms |
| `real_widget_full_layout_and_scene/200` | 25.243 ms | 25.593 ms |
| `real_widget_full_layout_and_scene/500` | 59.785 ms | 60.300 ms |
| `real_widget_scene_recollect_only/50` | 3.271 ms | 3.380 ms |
| `real_widget_scene_recollect_only/200` | 3.668 ms | 3.780 ms |
| `real_widget_scene_recollect_only/500` | 3.727 ms | 3.964 ms |
| `real_widget_text_heavy_scene_recollect/25` | 780.458 us | 787.472 us |
| `real_widget_text_heavy_scene_recollect/100` | 843.765 us | 855.851 us |
| `real_widget_text_heavy_scene_recollect/250` | 1.041 ms | 1.057 ms |
| `single_property_deep_leaf_full_recollect/4` | 873.492 us | 929.407 us |
| `single_property_deep_leaf_full_recollect/8` | 1.229 ms | 1.224 ms |
| `single_property_deep_leaf_full_recollect/16` | 2.608 ms | 2.603 ms |
| `single_property_deep_leaf_scene_patch/4` | 225.383 us | 222.760 us |
| `single_property_deep_leaf_scene_patch/8` | 247.117 us | 247.560 us |
| `single_property_deep_leaf_scene_patch/16` | 281.017 us | 281.869 us |

## Canvas

| Benchmark | Median | Mean |
| --- | ---: | ---: |
| `canvas_scene_build/50` | 22.480 us | 23.053 us |
| `canvas_scene_build/200` | 97.748 us | 98.264 us |
| `canvas_scene_build/1000` | 467.788 us | 468.276 us |
| `canvas_scene_query_point_all/50` | 5.252 ms | 5.259 ms |
| `canvas_scene_query_point_all/200` | 5.583 ms | 5.629 ms |
| `canvas_scene_query_point_all/1000` | 6.093 ms | 6.027 ms |
| `canvas_debug_export_json/50` | 165.605 us | 165.137 us |
| `canvas_debug_export_json/200` | 669.341 us | 679.795 us |
| `canvas_debug_export_json/1000` | 3.318 ms | 3.331 ms |
| `canvas_path_builder_cubic/1024` | 8.074 us | 8.048 us |

## 媒体

| Benchmark | Median | Mean |
| --- | ---: | ---: |
| `media_bytes_from_vec/1024` | 67.320 ns | 68.652 ns |
| `media_bytes_from_vec/262144` | 14.519 us | 16.686 us |
| `media_bytes_from_vec/4194304` | 1.020 ms | 1.018 ms |
| `media_bytes_clone_and_hash/4194304` | 15.437 ns | 15.722 ns |
| `media_source_path_url_bytes_construction` | 42.324 ns | 42.583 ns |

## 音频与视频

| Benchmark | Median | Mean |
| --- | ---: | ---: |
| `audio_output_write_f32/128` | 632.848 ns | 603.940 ns |
| `audio_output_write_f32/512` | 1.395 us | 1.337 us |
| `audio_output_write_f32/2048` | 1.989 us | 1.872 us |
| `audio_output_write_i16/128` | 566.781 ns | 453.505 ns |
| `audio_output_write_i16/2048` | 2.812 us | 3.059 us |
| `audio_ffmpeg_http_options/0` | 1.823 us | 1.851 us |
| `audio_ffmpeg_http_options/16` | 3.969 us | 3.946 us |
| `video_buffer_decision_helpers` | 5.052 ns | 5.147 ns |
| `video_queue_accounting/30` | 387.190 ns | 378.550 ns |
| `video_queue_accounting/300` | 2.754 us | 2.795 us |
| `video_queue_accounting/1200` | 7.999 us | 7.771 us |
| `video_compressed_byte_distribution/1200` | 10.623 us | 10.763 us |

## 既有基准的代表性结果

| Benchmark | Median | Mean | Note |
| --- | ---: | ---: | --- |
| `state_creation` | 70.189 ns | 70.524 ns | Real MVVM state |
| `state_read` | 8.530 ns | 8.662 ns | Real MVVM state |
| `state_write` | 121.828 ns | 121.494 ns | Real MVVM state |
| `signal_creation` | 294.190 ns | 295.738 ns | Real MVVM signal |
| `dependency_tracking/50` | 423.638 ns | 426.292 ns | Real state reads |
| `invalidation_propagation/20` | 436.809 ns | 443.456 ns | Real signal graph |
| `flat_layout/500` | 31.214 ms | 31.162 ms | Uses real layout helper |
| `complex_grid/30` | 47.795 ms | 50.895 ms | Uses real layout helper |
| `mixed_complex_layout` | 9.544 ms | 10.205 ms | Uses real layout helper |
| `scene_splice/200` | 40.417 us | 37.547 us | Synthetic vector splice |
| `text_layout/100` | 4.553 us | 4.744 us | Synthetic line split, not cosmic-text |
| `animation_update/200` | 53.466 ns | 57.225 ns | Synthetic animation helper |
| `command_dispatch/50` | 5.948 ns | 6.157 ns | Mostly empty dispatch helper |

## 已解决问题与保留说明

- `single_property_deep_leaf_scene_patch` 的 Windows warm-up 栈溢出已修复：`WidgetBenchmarkContext` 的 patch 列表不再用 `SmallVec<[ScenePatch; 8]>` 把多份 `CollectedSceneCache` 预留在函数栈帧内，改为按 roots 数量堆分配 `Vec`。
- deep-leaf full recollect 在 depth 8 以上的栈溢出已修复：benchmark 深树构造不再递归按值返回 `Flex` builder，改为迭代包裹子树；当前稳定记录覆盖 depth 4、8、16。
- `criterion::black_box` 弃用警告已清理，bench 文件统一使用 `std::hint::black_box`。
- 旧的合成 benchmark 仍保留为编译/形状 smoke test；真实性能判断以 `real_widget_pipeline`、`canvas_scene`、`media_source`、`audio_output`、`video_buffering` 和 `single_property_patch` 为主。

## 最终校验

```powershell
cargo fmt --check
cargo check --features bench-support --benches
cargo bench --features bench-support --bench single_property_patch -- --sample-size 10 --warm-up-time 0.2 --measurement-time 0.5
cargo check --features "bench-support audio" --bench audio_output
cargo check --features "bench-support video" --bench video_buffering
```

以上校验均已通过；`cargo check --features bench-support --benches` 不再报告 `criterion::black_box` 弃用警告或默认构建下的 icon dead-code 警告。
