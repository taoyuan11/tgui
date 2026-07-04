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

- 现象：`AudioController::load` 和 `VideoController::load` 会先调用 `reset_for_load()`，再把 source 交给后端；如果后端在同步校验阶段返回错误，shared state 已经进入 `Loading`，错误状态可能没有被写回。
- 涉及模块：`crates/tgui-runtime/src/audio/controller/mod.rs`、`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：在 controller 的 `load` 中捕获后端返回的 `Err`，调用 shared `set_error(...)` 后再返回错误；或者调整为先 validate source，再 reset shared state。
- 风险/验证要点：需要保证成功加载仍然只触发一次 loading media event；新增测试覆盖非法 header、空路径/非法 URL 等同步失败场景，并断言 `playback_state == Error(...)`、`error()` 和 snapshot 一致。

### 2. FFmpeg 初始化错误被忽略

- 现象：音频和视频后端都使用 `FFMPEG_INIT.call_once(|| { let _ = ffmpeg::init(); })`，初始化失败会被吞掉，后续错误可能延迟到 open/decode 阶段，用户看到的原因不够明确。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：保存 `ffmpeg::init()` 的结果，初始化失败时让 `load` 直接返回 `TguiError::Media`；考虑提供统一的 FFmpeg init helper，避免音频和视频重复实现。
- 风险/验证要点：`Once` 不方便携带失败结果，改动时要避免重复初始化竞态；新增单测可通过抽象 init hook 或小型 helper 测试错误缓存行为。

### 3. `shutdown` / `join` 在阻塞 IO 下可能卡住

- 现象：音频 `shutdown` join 一个 worker，视频 `shutdown` join present/decode 两个 worker；如果 FFmpeg 阻塞在网络读取或设备 IO，关闭窗口、drop controller 或视频 `stop` 可能被拖住。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/worker/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/worker.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/present.rs`。
- 建议方向：引入可取消的 open/read 策略、关闭超时或非阻塞回收路径；网络源继续保留 `rw_timeout`，同时评估是否需要更短的 shutdown 专用退出机制。
- 风险/验证要点：不能泄漏音频输出流、decode queue 或最新帧；需要用本地文件、慢网络 URL、断网 URL 分别验证 stop/drop 不长时间阻塞。

### 4. `buffer_memory_limit_bytes` 主要按压缩字节统计

- 现象：音频和视频缓存上限当前更多统计 compressed bytes；视频 ready frame 实际持有 RGBA `TextureFrame`，音频队列实际持有 `Vec<f32>` samples，真实内存可能明显高于压缩字节。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/shared/output.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/session/decode.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/buffering.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/queue.rs`。
- 建议方向：将缓存统计拆成 compressed bytes、decoded audio bytes、decoded video frame bytes；`buffer_memory_limit_bytes` 优先约束真实持有内存，保留压缩字节作为吞吐/估算指标。
- 风险/验证要点：改变统计口径会影响缓冲策略和启动时机；需要补低内存限制、高清视频、大音频 chunk、网络视频缓存受限时的状态机测试。

## P1: 性能和资源优化

### 1. 音频重采样和队列写入有分配优化空间

- 现象：重采样路径为每个 decoded frame 分配新的 `AudioFrame`，再复制为 `Vec<f32>`；输出回调使用互斥队列取 chunk，频繁小 chunk 下可能增加分配和锁竞争。
- 涉及模块：`crates/tgui-runtime/src/audio/backend/ffmpeg/session/decode.rs`、`crates/tgui-runtime/src/audio/backend/shared/output.rs`。
- 建议方向：评估复用 resampled frame buffer、合并小 chunk、使用 ring buffer 或更实时友好的无锁/低锁音频队列。
- 风险/验证要点：音频回调不能做重分配或耗时锁等待；需保留 muted 状态仍推进 clock、volume 变更实时生效、underflow 标记正确等现有行为。

### 2. 视频每帧 CPU RGBA 转换与 `TextureFrame` 分配可复用

- 现象：视频帧经 scaler 转为 RGBA 后生成新的 `TextureFrame`，每帧都有 CPU 转换和分配成本；高分辨率或高帧率视频下会放大内存带宽压力。
- 涉及模块：`crates/tgui-runtime/src/video/backend/ffmpeg/decode.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/helpers.rs`、`crates/tgui-runtime/src/rendering/renderer/prepare.rs`。
- 建议方向：短期可复用 frame buffer 或 texture id/revision 管理；中长期评估 YUV 纹理上传和 shader 转换，减少 CPU RGBA 转换与拷贝。
- 风险/验证要点：必须保持 renderer 对 frame revision 的缓存更新语义；需要测试缩放、seek、source 切换和透明/裁剪/圆角场景。

### 3. `VideoSurface` target raster 更新可合并

- 现象：`push_video_texture_or_placeholder` 每次 scene collect 都会根据布局计算 `RasterRequest` 并调用 `set_target_raster(...)`；后端内部会比较是否变化，但仍有锁和命令路径成本。
- 涉及模块：`crates/tgui-runtime/src/ui/widget/core/render/media.rs`、`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`。
- 建议方向：在 scene/layout 层缓存上一次 target raster，只有尺寸或 scale factor 变化时再通知后端；或者让 controller 提供轻量的去重接口。
- 风险/验证要点：窗口缩放、DPI 变化、fit 变化、布局动画时必须及时更新；需要覆盖 target raster 变化和不变化两类测试。

### 4. 视频 `stop` 会拆掉 worker，可评估轻量 Stop 命令

- 现象：`VideoController::stop` 当前调用后端 `shutdown()`，会关闭 worker 并 reset shared state；再次 load/play 需要重新创建线程和队列。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/present.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/worker.rs`。
- 建议方向：候选方向是新增内部 `Stop` 命令，只清空当前 session、queue、latest frame 和状态，保留 worker；`shutdown` 继续用于 drop 和应用退出。
- 风险/验证要点：需要确保 stop 后不会继续播放旧 generation，不保留旧音频输出，不泄漏网络输入；测试 stop 后重新 load、stop 后 drop、stop 后 play 空操作。

## P2: API、体验和长期能力

### 1. 视频公开 API 文档可补齐

- 现象：音频类型有较完整中文 doc comment；视频 controller/source/state/style 的公开文档相对少，README 有使用说明但 API 级文档仍可增强。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/types.rs`、`crates/tgui-runtime/src/ui/widget/video.rs`、`README.md`。
- 建议方向：为 `VideoController`、`VideoSource`、`VideoPlaybackState`、`VideoMetrics`、`Video`、`VideoSurface` 补齐参数、行为和 feature 要求说明。
- 风险/验证要点：文档不能承诺尚未实现的能力；补 doc 后可跑 `cargo doc -p tgui --features video --no-deps` 检查公开文档生成。

### 2. 音视频 source 类型重复，且缺少 bytes source

- 现象：`AudioSource` 和 `VideoSource` 都是 `File(PathBuf)` / `Url { url, headers }`，header API 也重复；图片媒体已有 bytes 来源，音视频暂不支持内存 bytes。
- 涉及模块：`crates/tgui-runtime/src/audio/types.rs`、`crates/tgui-runtime/src/video/types.rs`、`crates/tgui-runtime/src/audio/backend/ffmpeg/session/source.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/decode/open.rs`。
- 建议方向：候选方向是抽出共享 media source builder 或内部 helper；进一步评估 `Bytes` / `Arc<[u8]>` source，通过 FFmpeg custom IO 或临时资源实现。
- 风险/验证要点：这是 public API 候选变更，必须保持现有 `AudioSource` / `VideoSource` 构造方式兼容；bytes source 需要明确生命周期、seek 能力和内存上限。

### 3. 视频缺少 looping、playback rate、轨道和字幕能力

- 现象：音频已有 `set_looping`；视频播放器具备基础播放/暂停/seek/音量/静音，但没有公开 looping、倍速、音轨/字幕轨选择等媒体播放器常见能力。
- 涉及模块：`crates/tgui-runtime/src/video/controller.rs`、`crates/tgui-runtime/src/video/backend/mod.rs`、`crates/tgui-runtime/src/video/backend/ffmpeg/*`、`crates/tgui-runtime/src/ui/widget/video.rs`。
- 建议方向：候选方向是先补 `set_looping` 与 `looping()`，再评估 playback rate；轨道和字幕需要先设计公开类型、选择策略和 UI 是否内置。
- 风险/验证要点：looping 与 EOF、seek、buffering、audio clock 同步强相关；倍速涉及音频重采样和 clock；字幕涉及文本布局、时间轴和样式。

### 4. `VideoBackend` lifecycle hook 目前未接入 runtime

- 现象：`VideoBackend` trait 预留了 `on_surface_lost`、`on_surface_restored`、`on_app_background`、`on_app_foreground`，当前默认 no-op，代码搜索未发现 runtime suspend/resume 调用这些 hook。
- 涉及模块：`crates/tgui-runtime/src/video/backend/mod.rs`、`crates/tgui-runtime/src/runtime/application_handler.rs`、`crates/tgui-runtime/src/runtime/render_cycle.rs`、`crates/tgui-runtime/src/runtime/windows.rs`。
- 建议方向：候选方向是 runtime 在 surface lost/restored、app suspended/resumed 时通知活跃 video controller；后台时可暂停 decode/present 或降级刷新。
- 风险/验证要点：多窗口、多 controller、surface 重建和后台继续播放策略都需要明确；必须保证桌面平台现有行为不被误暂停。

## 后续建议

1. 先处理 P0 中的 `load` 失败状态和 FFmpeg init 错误，这两项改动范围较小且直接改善错误可观测性。
2. 再建立真实缓存内存统计，否则后续缓冲阈值调优和性能优化缺少可靠基线。
3. P1 的分配/锁优化建议配合 benchmark 或 profiling 做，不宜只凭直觉重写后端。
4. P2 的 API 候选方向需要先写小型 RFC 或 issue，明确兼容策略后再实现。

## 本次文档变更说明

- 本文只新增优化记录，不包含任何 public API、类型、接口或行为变更。
- 涉及未来 API 的条目均标记为“候选方向”，不代表当前版本已经支持。
- 未处理现有未跟踪 `.DS_Store` 文件，也未删除、重命名或覆盖任何现有文档。
