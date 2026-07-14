# tgui 音视频组件优化点记录

本文记录对当前音视频组件的一轮只读检查结论，重点面向后续维护和优化排期。检查范围覆盖 `audio` / `video` feature、控制器、FFmpeg/CPAL 后端、`Audio` 隐形组件、`Video` / `VideoSurface` 组件、渲染接入和现有相关测试。

## 当前状态

- `audio`、`video`、`video-static` feature 在当前环境下均可编译通过。
- 音频相关过滤测试通过 24 个，覆盖控制器命令转发、播放状态、循环、停止、HTTP header 校验、widget 挂载/卸载和媒体事件。
- 视频相关过滤测试通过 47 个，覆盖控制器、播放器控件、seek/volume/mute、surface placeholder/texture、队列 generation、缓冲策略和部分 present/decode 状态机。
- 当前音视频链路已经具备可用基础：音频由 FFmpeg 解码并经 CPAL 输出，视频由 decode/present 两类后台线程协作，将帧转为 `TextureFrame` 后接入渲染器。

已通过的检查命令：

```bash
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
cargo test -p tgui-runtime --features audio audio --lib -- --test-threads=1
cargo test -p tgui-runtime --features video video --lib -- --test-threads=1
```

## P0: 正确性和稳定性优先

### 1. `load` 失败后 shared state 可能停留在 Loading

- 状态：已完成并验证。`AudioController::load` / `VideoController::load` 已在后端同步返回 `Err` 时写回 shared error；真实 FFmpeg 后端会先校验 source，非法 header / 空 bytes 这类同步失败不会启动 worker。
- 现象：`AudioController::load` 和 `VideoController::load` 会先调用 `reset_for_load()`，再把 source 交给后端；如果后端在同步校验阶段返回错误，shared state 已经进入 `Loading`，错误状态可能没有被写回。
- 涉及模块：`crates/tgui-runtime/src/audio/controller/mod.rs`、`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：在 controller 的 `load` 中捕获后端返回的 `Err`，调用 shared `set_error(...)` 后再返回错误；或者调整为先 validate source，再 reset shared state。
- 风险/验证要点：需要保证成功加载仍然只触发一次 loading media event；新增测试覆盖非法 header、空路径/非法 URL 等同步失败场景，并断言 `playback_state == Error(...)`、`error()` 和 snapshot 一致。
- 实施记录：新增/覆盖 `load_failure_sets_error_state_and_snapshot`、`load_failure_sets_error_state_and_surface_snapshot`、`controller_load_rejects_invalid_source_without_starting_worker`、`controller_load_rejects_invalid_source_without_starting_workers`；已运行对应 audio/video 单测。

### 2. FFmpeg 初始化错误被忽略

- 状态：已完成并验证。音频和视频后端共用 `ensure_ffmpeg_initialized()`，通过 `OnceLock<Result<_, _>>` 缓存成功或失败结果，初始化失败会以明确 `TguiError::Media` 暴露。
- 现象：音频和视频后端都使用 `FFMPEG_INIT.call_once(|| { let _ = ffmpeg::init(); })`，初始化失败会被吞掉，后续错误可能延迟到 open/decode 阶段，用户看到的原因不够明确。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：保存 `ffmpeg::init()` 的结果，初始化失败时让 `load` 直接返回 `TguiError::Media`；考虑提供统一的 FFmpeg init helper，避免音频和视频重复实现。
- 风险/验证要点：`Once` 不方便携带失败结果，改动时要避免重复初始化竞态；新增单测可通过抽象 init hook 或小型 helper 测试错误缓存行为。
- 实施记录：`crates/tgui-runtime/src/audio/backend/shared/init.rs` 提供共享 helper；测试覆盖 init 成功/失败只调用一次并缓存结果。测试侧 resampler 初始化也改为使用该 helper，不再吞掉 `ffmpeg::init()` 结果。

### 3. `shutdown` / `join` 在阻塞 IO 下可能卡住

- 状态：已完成基础实现并验证。已加入 `join_with_timeout` 并让 audio/video shutdown 使用超时 join；网络源仍保留 FFmpeg `timeout` / `rw_timeout`。慢网络、断网、设备 IO 的真实退出耗时仍建议补充人工端到端验证。
- 现象：音频 `shutdown` join 一个 worker，视频 `shutdown` join present/decode 两个 worker；如果 FFmpeg 阻塞在网络读取或设备 IO，关闭窗口、drop controller 或视频 `stop` 可能被拖住。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/worker/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/worker.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/present.rs`。
- 建议方向：引入可取消的 open/read 策略、关闭超时或非阻塞回收路径；网络源继续保留 `rw_timeout`，同时评估是否需要更短的 shutdown 专用退出机制。
- 风险/验证要点：不能泄漏音频输出流、decode queue 或最新帧；需要用本地文件、慢网络 URL、断网 URL 分别验证 stop/drop 不长时间阻塞。
- 实施记录：`crates/tgui-runtime/src/foundation/threading.rs` 已覆盖完成线程 join 与超时 detach；audio/video backend shutdown 均走该 helper。新增 `shutdown_returns_after_timeout_when_worker_is_blocked` 和 `shutdown_returns_after_timeout_when_workers_are_blocked`，覆盖 backend 层遇到阻塞 worker 时能在超时后返回并清理 handle。

### 4. `buffer_memory_limit_bytes` 主要按压缩字节统计

- 状态：已完成基础实现并有单元覆盖。音频队列统计 decoded sample bytes，视频 ready queue 统计 decoded frame bytes，buffering 判断使用 pending packet、ready frame 与音频输出的实际持有内存。后续仍可补真实大媒体/低内存端到端场景。
- 现象：音频和视频缓存上限当前更多统计 compressed bytes；视频 ready frame 实际持有 RGBA `TextureFrame`，音频队列实际持有 `Vec<f32>` samples，真实内存可能明显高于压缩字节。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/shared/output.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/session/decode.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/buffering.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/queue.rs`。
- 建议方向：将缓存统计拆成 compressed bytes、decoded audio bytes、decoded video frame bytes；`buffer_memory_limit_bytes` 优先约束真实持有内存，保留压缩字节作为吞吐/估算指标。
- 风险/验证要点：改变统计口径会影响缓冲策略和启动时机；需要补低内存限制、高清视频、大音频 chunk、网络视频缓存受限时的状态机测试。
- 实施记录：覆盖 `decoded_audio_memory_*`、`large_audio_chunks_release_decoded_memory_incrementally`、`ready_memory_counts_decoded_frame_bytes`、`average_non_zero_bytes_*` 等测试。

## P1: 性能和资源优化

### 1. 音频重采样和队列写入有分配优化空间

- 状态：已完成阶段性代码优化并验证。已完成会话级重采样 `AudioFrame` 复用、按声道对齐的 sample batch、较大 chunk 的增量内存释放；已扩展并运行 `audio_output` Criterion benchmark，覆盖普通写入、倍速写入、碎片化 queue 压力和持续 callback 仿真；`bench-support` 下新增音频输出 callback/lock miss/underflow/written samples 诊断计数，后续真实设备播放可直接量化回调压力。本轮未引入无锁/ring buffer，因当前持续回调仿真未复现 lock miss / underflow，真实设备结果若显示竞争再进入下一轮队列替换。
- 现象：重采样路径为每个 decoded frame 分配新的 `AudioFrame`，再复制为 `Vec<f32>`；输出回调使用互斥队列取 chunk，频繁小 chunk 下可能增加分配和锁竞争。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/session/decode.rs`、`crates/tgui-runtime/src/audio/backend/shared/output.rs`。
- 建议方向：评估复用 resampled frame buffer、合并小 chunk、使用 ring buffer 或更实时友好的无锁/低锁音频队列。
- 风险/验证要点：音频回调不能做重分配或耗时锁等待；需保留 muted 状态仍推进 clock、volume 变更实时生效、underflow 标记正确等现有行为。
- 实施记录：`ReusableAudioFrame`、`AudioSampleBatch::new_for_channels`、resampler flush/receive helper 已落地；`AudioSession` 和视频 `DecodeSession` 的内置音轨路径现在复用同一个会话级 resample buffer，避免每个 packet/EOF flush 重新创建输出 frame 缓存。已运行 `reusable_audio_frame_*`、`audio_sample_batch_aligns_chunks_to_channel_frames`、`rate_adjusted_audio_drops_partial_frame_tail_without_leaking_queue_counters`。新增 bench-support playback-rate/queue 状态 hook 和 `AudioOutputDiagnostics`，扩展 `benches/audio_output.rs`；已运行 `cargo test -p tgui-runtime --features "audio bench-support" bench_output_playback_rate_updates_rate_and_resets_fraction --lib -- --test-threads=1 --nocapture`、`cargo test -p tgui-runtime --features "audio bench-support" bench_output_diagnostics --lib -- --test-threads=1 --nocapture`、`cargo bench -p tgui-benchmarks --bench audio_output --no-run --features audio`，并用 `--sample-size 10 --measurement-time 1 --warm-up-time 1` 获取本机短样本基线：512 帧 f32 正常写入约 2.02-2.58µs，512 帧 0.5x 写入约 9.96-11.74µs，1.5x/2x 写入约 10.43-14.88µs，碎片 queue 在 1/4/16/128 帧每 chunk 时分别约 24.5-30.6µs、7.35-12.34µs、6.16-8.03µs、1.60-1.90µs。已补充 `audio_output_sustained_callback_simulation`，用持续 512-frame callback + producer 补料仿真量化 lock miss / underflow / written sample 压力；新增 `bench_output_sustained_callbacks_stay_buffered_without_lock_misses` 断言 96 次回调无 lock miss / underflow。新增短样本结果：normal 64-frame producer chunk 约 111.43-128.62µs，normal 512-frame chunk 约 80.68-104.48µs，1.5x 512-frame chunk 约 662.20-857.45µs。

### 2. 视频每帧 CPU RGBA 转换与 `TextureFrame` 分配可复用

- 状态：已完成阶段性代码优化并验证。短期 RGBA buffer/texture id revision 复用已落地；NV12/YUV420P 直接 YUV render frame、renderer YUV texture cache 与 shader 转换路径已落地；已新增并运行短样本 `video_frame_conversion` Criterion benchmark，建立 CPU frame conversion / direct YUV 基线；`bench-support` 下新增 renderer texture diagnostics，可记录 RGBA/YUV cache hit、texture create、full/dirty upload 和上传字节数；持续 120 帧转换仿真已覆盖播放器稳态帧流。真实播放器场景/GPU 上传诊断仍建议在具备窗口和样片的环境做人工补测，用于后续阈值或管线调优。
- 现象：视频帧经 scaler 转为 RGBA 后生成新的 `TextureFrame`，每帧都有 CPU 转换和分配成本；高分辨率或高帧率视频下会放大内存带宽压力。
- 涉及模块：`crates/tgui-runtime/src/video/backend/ffmpeg/decode.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/helpers.rs`、`crates/tgui-runtime/src/rendering/renderer/prepare.rs`。
- 建议方向：短期可复用 frame buffer 或 texture id/revision 管理；中长期评估 YUV 纹理上传和 shader 转换，减少 CPU RGBA 转换与拷贝。
- 风险/验证要点：必须保持 renderer 对 frame revision 的缓存更新语义；需要测试缩放、seek、source 切换和透明/裁剪/圆角场景。
- 实施记录：覆盖 RGBA 复用、dirty row、YUV frame/cache/signature、`video_surface_renders_yuv_texture_when_render_frame_exists`、`video_frame_converter_keeps_yuv_when_target_raster_scales_frame`、`video_frame_converter_downscales_yuv_with_rgba_scaler` 等测试。新增 `BenchVideoFrameConverter` / `BenchVideoFrameKind` bench-support hook 与 `benches/video_frame_conversion.rs`；新增 `RendererTextureDiagnostics` 并通过 `video::bench_support` 暴露 `renderer_texture_diagnostics` / `reset_renderer_texture_diagnostics`。已运行 `cargo test -p tgui-runtime --features "video bench-support" bench_video_frame_converter --lib -- --test-threads=1 --nocapture`、`cargo test -p tgui-runtime --features "video bench-support" renderer_texture_diagnostics_reset_and_snapshot_counts_uploads --lib -- --test-threads=1 --nocapture`、`cargo bench -p tgui-benchmarks --bench video_frame_conversion --no-run --features video`、`cargo check -p tgui --features video`，并用 `--sample-size 10 --measurement-time 1 --warm-up-time 1` 获取本机短样本基线：1080p RGBA passthrough 约 1.86-2.11ms，1080p RGB24 expand 约 4.90-6.43ms，1080p NV12 direct YUV 约 1.18-1.66ms，1080p NV12 downscale RGBA 约 4.17-5.62ms，4K YUV420P direct YUV 约 2.69-3.27ms。已补充 `video_frame_sequence_conversion`，用同一 converter / texture id 连续 120 帧转换模拟播放器稳态帧流，观察 YUV 直通和 RGBA 缩放路径的持续成本；新增 `bench_video_frame_converter_sequence_keeps_*` 测试覆盖连续 revision 与路径保持。新增短样本结果：120 帧 1080p RGBA passthrough 约 130.85-212.16ms，120 帧 1080p NV12 direct YUV 约 35.63-38.68ms，120 帧 1080p NV12 downscale RGBA 约 222.10-233.30ms。

### 3. `VideoSurface` target raster 更新可合并

- 状态：已完成并验证。采用 controller 轻量 atomic key 去重接口，scene collect 可以继续上报目标 raster，但相同尺寸不会进入后端命令路径；尺寸变化会及时更新。
- 现象：`push_video_texture_or_placeholder` 每次 scene collect 都会根据布局计算 `RasterRequest` 并调用 `set_target_raster(...)`；后端内部会比较是否变化，但仍有锁和命令路径成本。
- 涉及模块：`crates/tgui-runtime/src/ui/widget/core/render/media.rs`、`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：在 scene/layout 层缓存上一次 target raster，只有尺寸或 scale factor 变化时再通知后端；或者让 controller 提供轻量的去重接口。
- 风险/验证要点：窗口缩放、DPI 变化、fit 变化、布局动画时必须及时更新；需要覆盖 target raster 变化和不变化两类测试。
- 实施记录：`target_raster_updates_are_deduplicated` 覆盖 controller 去重；`video_surface_target_raster_updates_only_when_layout_size_changes` 覆盖重复 collect 和布局尺寸变化。

### 4. 视频 `stop` 会拆掉 worker，可评估轻量 Stop 命令

- 状态：已完成并验证。`VideoBackend::stop` 已走内部 Stop 命令清空当前 session、queue、latest frame 和 shared state，保留 worker；无 worker 时 stop 只重置状态。
- 现象：`VideoController::stop` 当前调用后端 `shutdown()`，会关闭 worker 并 reset shared state；再次 load/play 需要重新创建线程和队列。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/present.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/worker.rs`。
- 建议方向：候选方向是新增内部 `Stop` 命令，只清空当前 session、queue、latest frame 和状态，保留 worker；`shutdown` 继续用于 drop 和应用退出。
- 风险/验证要点：需要确保 stop 后不会继续播放旧 generation，不保留旧音频输出，不泄漏网络输入；测试 stop 后重新 load、stop 后 drop、stop 后 play 空操作。
- 实施记录：覆盖 `stop_clears_session_without_shutting_down_decode_worker`、`stop_command_clears_queue_and_clock_without_exiting_worker`、`stop_without_workers_resets_state_without_starting_workers`。

## P2: API、体验和长期能力

### 1. 视频公开 API 文档可补齐

- 状态：已完成并验证。视频 controller/source/state/metrics/player/surface 的公开文档已补齐到 API 层，`cargo doc -p tgui --features video --no-deps` 可生成。
- 现象：音频类型有较完整中文 doc comment；视频 controller/source/state/style 的公开文档相对少，README 有使用说明但 API 级文档仍可增强。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/types.rs`、`crates/tgui-runtime/src/ui/widget/video.rs`、`README.md`。
- 建议方向：为 `VideoController`、`VideoSource`、`VideoPlaybackState`、`VideoMetrics`、`Video`、`VideoSurface` 补齐参数、行为和 feature 要求说明。
- 风险/验证要点：文档不能承诺尚未实现的能力；补 doc 后可跑 `cargo doc -p tgui --features video --no-deps` 检查公开文档生成。
- 实施记录：补充了 looping、playback rate、audio/subtitle track、subtitle cue、bytes source、VideoSurface/Video 控制项等说明；最近一次 `cargo doc -p tgui --features video --no-deps` 已通过。

### 2. 音视频 source 类型重复，且缺少 bytes source

- 状态：已完成并验证。`AudioSource::Bytes` / `VideoSource::Bytes` 已公开，FFmpeg 后端通过临时只读媒体文件加载并清理；header 校验、临时文件创建、extension hint 归一化等共用内部 helper；controller `load` 已接受 `impl Into<...Source>`，并支持从共享 `MediaSource` 和新的公开 `MediaPlaybackSource` 转入音视频 source。`AudioSource` / `VideoSource` 旧 builder 仍保留兼容，跨音视频复用场景可改用 `MediaPlaybackSource`。
- 现象：`AudioSource` 和 `VideoSource` 都是 `File(PathBuf)` / `Url { url, headers }`，header API 也重复；图片媒体已有 bytes 来源，音视频暂不支持内存 bytes。
- 涉及模块：`crates/tgui-runtime/src/audio/types.rs`、`crates/tgui-runtime/src/video/types.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/session/source.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/open.rs`。
- 建议方向：已抽出共享 `MediaPlaybackSource` builder；`Bytes` / `Arc<[u8]>` source 当前通过临时只读媒体文件实现，后续若要减少临时文件 IO，可再单独评估 FFmpeg custom IO。
- 风险/验证要点：这是 public API 候选变更，必须保持现有 `AudioSource` / `VideoSource` 构造方式兼容；bytes source 需要明确生命周期、seek 能力和内存上限。
- 实施记录：覆盖 `audio_source_tests`、`video_source_tests`、`temporary_media_file_*`、`empty_bytes_source_is_rejected_before_open`、`empty_bytes_video_source_is_rejected_before_open`、`bytes_sources_use_local_buffer_profile_and_no_http_options`。新增 `From<MediaSource>` / `From<MediaPlaybackSource>` for `AudioSource` / `VideoSource` 与 controller `load` 泛型入口；新增 `media_source_converts_to_*_source`、`controller_load_accepts_media_source`、`media_playback_source_preserves_*_headers_and_extension`、`controller_load_accepts_media_playback_source` 测试。已运行 `cargo check -p tgui --no-default-features`、`cargo test -p tgui-runtime --features audio media_playback_source --lib -- --test-threads=1 --nocapture`、`cargo test -p tgui-runtime --features video media_playback_source --lib -- --test-threads=1 --nocapture`。

### 3. 视频缺少 looping、playback rate、轨道和字幕能力

- 状态：已完成基础能力并验证。视频 controller/backend/player 已支持 looping、playback rate、音轨选择、字幕轨选择、文本字幕 overlay 和 bitmap subtitle overlay；UI 控件可按需显示/隐藏。
- 现象：音频已有 `set_looping`；视频播放器具备基础播放/暂停/seek/音量/静音，但没有公开 looping、倍速、音轨/字幕轨选择等媒体播放器常见能力。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/*`、`crates/tgui-runtime/src/ui/widget/video.rs`。
- 建议方向：候选方向是先补 `set_looping` 与 `looping()`，再评估 playback rate；轨道和字幕需要先设计公开类型、选择策略和 UI 是否内置。
- 风险/验证要点：looping 与 EOF、seek、buffering、audio clock 同步强相关；倍速涉及音频重采样和 clock；字幕涉及文本布局、时间轴和样式。
- 实施记录：覆盖 `looping_updates_shared_state_and_backend`、`set_looping_updates_worker_and_shared_state`、`playback_rate_is_clamped_and_exposed_as_signal`、`set_playback_rate_updates_worker_shared_state_and_decode_worker`、`video_player_*track_selector*`、`video_player_renders_active_subtitle_overlay`、`video_surface_renders_bitmap_subtitle_overlay`。

### 4. `VideoBackend` lifecycle hook 目前未接入 runtime

- 状态：已完成并验证。runtime 在 surface 创建/恢复后通知 `surface_restored`，隐藏帧后通知 `app_foreground`；`suspend()` 按 `app_background` -> `surface_lost` 顺序通知活跃 video controller，并对同一 controller 去重。
- 现象：`VideoBackend` trait 预留了 `on_surface_lost`、`on_surface_restored`、`on_app_background`、`on_app_foreground`，当前默认 no-op，代码搜索未发现 runtime suspend/resume 调用这些 hook。
- 涉及模块：`crates/tgui-runtime/src/video/backend/mod.rs`、`crates/tgui-runtime/src/runtime/application_handler.rs`、`crates/tgui-runtime/src/runtime/render_cycle.rs`、`crates/tgui-runtime/src/runtime/windows.rs`。
- 建议方向：候选方向是 runtime 在 surface lost/restored、app suspended/resumed 时通知活跃 video controller；后台时可暂停 decode/present 或降级刷新。
- 风险/验证要点：多窗口、多 controller、surface 重建和后台继续播放策略都需要明确；必须保证桌面平台现有行为不被误暂停。
- 实施记录：覆盖 `controller_forwards_lifecycle_hooks_to_backend`、`video_lifecycle_notifications_deduplicate_active_controllers`、`video_lifecycle_notifications_follow_suspend_order`。

## 后续建议

1. 先处理 P0 中的 `load` 失败状态和 FFmpeg init 错误，这两项改动范围较小且直接改善错误可观测性。
2. 再建立真实缓存内存统计，否则后续缓冲阈值调优和性能优化缺少可靠基线。
3. P1 的分配/锁优化建议配合 benchmark 或 profiling 做，不宜只凭直觉重写后端。
4. P2 的 API 候选方向需要先写小型 RFC 或 issue，明确兼容策略后再实现。

## 本轮实施说明

- 当前记录已从最初只读检查演进为逐项实施日志；带“已完成并验证”的条目对应当前工作树中的代码或测试改动。
- P2.2 已新增公开 `MediaPlaybackSource` builder，并保持 `AudioSource` / `VideoSource` 既有构造 API 兼容。
- 未删除、重命名或覆盖与音视频优化无关的现有文档。
