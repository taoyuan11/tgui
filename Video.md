# tgui 视频能力现状与维护说明

本文基于当前工作区代码更新，用来记录 `tgui` 视频能力的实际 API、内部链路和仍未实现的边界。旧文档中很多内容是早期设计建议；当前代码已经落地了 `video` feature、`VideoController`、`VideoSurface` 和 FFmpeg 后端，因此这里以现状为准。

## 1. 当前定位

视频能力目前是可选功能：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["video"] }
```

`Cargo.toml` 中的相关 feature：

* `video = ["audio"]`
* `video-static = ["video", "ffmpeg-next/static"]`

启用 `video` 后，公共 API 会通过 `tgui::video`、`tgui::prelude` 和部分 `tgui::widgets` re-export 暴露。当前公开类型包括：

* `VideoController`
* `VideoSurface`
* `VideoSource`
* `VideoPlaybackState`
* `VideoMetrics`
* `VideoSize`

当前已经实现的是低层视频 surface 与控制器组合；还没有内建的完整 `Video` 播放器控件，也没有默认 controls、poster、looping 或 autoplay 组件。

## 2. 当前架构

实际链路如下：

1. ViewModel 持有 `VideoController`，通过 `ViewModelContext` 创建。
2. 用户代码调用 `controller.load(VideoSource)`、`play`、`pause`、`seek` 等方法。
3. `VideoController` 把命令转发给内部 `VideoBackend`，并通过 `State` / `Signal` 暴露播放状态。
4. `VideoSurface` 参与 widget tree、布局、样式解析、命中测试和媒体事件派发。
5. scene 收集阶段根据 `VideoSurface` 的实际布局尺寸设置目标 `RasterRequest`，并在有当前帧时生成 `VideoTexturePrimitive`，否则生成 loading/error/idle placeholder。
6. FFmpeg 后端在线程中打开媒体、解码视频和音频，把视频帧缩放并转换成 RGBA `TextureFrame`。
7. 渲染器使用普通 texture cache 上传 `TextureFrame`；同一视频流尺寸不变时通过 revision 更新 GPU texture。

这里的后端 trait 目前是 `pub(crate)`，不是外部可插拔的公共扩展点。若以后要支持第三方后端，需要先设计稳定的 public backend API。

## 3. 平台和后端状态

当前代码中的视频后端是 `src/video/backend/ffmpeg/` 下的 FFmpeg 实现。它会懒启动两个后台线程：

* decode worker：打开输入、解复用、解码视频/音频、填充缓冲队列。
* present worker：处理控制命令、推进播放状态、按时钟展示帧、更新 metrics 和 surface snapshot。

音频输出复用 `audio` feature 的共享输出能力；如果视频没有音频流，则使用软件时钟推进视频。

当前没有 Android / OHOS / 原生平台视频后端。`VideoBackend` trait 中预留了 `on_surface_lost`、`on_surface_restored`、`on_app_background`、`on_app_foreground` 默认空实现，但目前没有移动端后端落地。

Windows 启用 `video` feature 时，`build.rs` 会额外链接 `strmiids` 和 `mfuuid`。

## 4. 公共 API

### 4.1 `VideoSource`

当前定义：

```rust
pub enum VideoSource {
    File(std::path::PathBuf),
    Url {
        url: String,
        headers: Vec<(String, String)>,
    },
}
```

已支持：

* 本地文件：`VideoSource::File(path)`
* URL：`VideoSource::url(url)`
* URL 请求头：`with_header` / `with_headers`

尚未支持：

* `Bytes`
* 自定义 reader
* HLS / DASH 层面的专用 API

注意：`impl From<String>` 和 `impl From<&str>` 当前会创建 `VideoSource::Url`，不会自动判断本地路径。加载本地文件时应显式使用 `VideoSource::File(PathBuf::from(path))`。

示例：

```rust
let file = VideoSource::File(std::path::PathBuf::from("demo.mp4"));

let url = VideoSource::url("https://example.com/demo.mp4")
    .with_header("Authorization", "Bearer <token>")
    .with_headers([
        ("Referer", "https://example.com/player"),
        ("Cookie", "session=abc123"),
    ]);
```

实际可播放格式取决于本机 FFmpeg 构建和可用解码器。代码不会把能力限制为 MP4/H264/AAC；后端会使用 FFmpeg 的 best stream/decoder 逻辑，并对 AV1 优先尝试 `libdav1d`、`libaom-av1`、`av1`。

### 4.2 `VideoPlaybackState`

当前状态枚举：

```rust
pub enum VideoPlaybackState {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Buffering,
    Ended,
    Error(String),
}
```

不要把播放状态简化成 `bool is_playing`。UI 层需要区分 loading、ready、buffering、ended 和 error。

### 4.3 `VideoMetrics`

当前指标：

```rust
pub struct VideoMetrics {
    pub duration: Option<std::time::Duration>,
    pub position: std::time::Duration,
    pub buffered: Option<std::time::Duration>,
    pub video_width: u32,
    pub video_height: u32,
}
```

`position()`、`duration()` 和 `buffered_position()` 会启用 metrics 观测。未读取这些 Signal 时，后端会避免持续写 metrics，减少不必要的 invalidation。

### 4.4 `VideoController`

当前控制器方法包括：

```rust
impl VideoController {
    pub fn new(ctx: &ViewModelContext) -> Self;

    pub fn load(&self, source: VideoSource) -> Result<(), TguiError>;
    pub fn play(&self);
    pub fn replay(&self);
    pub fn pause(&self);
    pub fn seek(&self, position: std::time::Duration);
    pub fn set_volume(&self, volume: f32);
    pub fn set_muted(&self, muted: bool);
    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64);

    pub fn playback_state(&self) -> Signal<VideoPlaybackState>;
    pub fn position(&self) -> Signal<std::time::Duration>;
    pub fn duration(&self) -> Signal<Option<std::time::Duration>>;
    pub fn buffered_position(&self) -> Signal<Option<std::time::Duration>>;
    pub fn volume(&self) -> Signal<f32>;
    pub fn muted(&self) -> Signal<bool>;
    pub fn video_size(&self) -> Signal<VideoSize>;
    pub fn error(&self) -> Signal<Option<String>>;
}
```

当前没有 `set_rate`，也没有 autoplay、looping 或完整播放列表 API。`play()` 在当前状态为 `Ended` 时会走 `replay()`，即 seek 到开头后再播放。

音量会被 clamp 到 `0.0..=1.0`。默认缓冲内存限制是 `100 * 1024 * 1024` bytes，可通过 `set_buffer_memory_limit_bytes` 调整。内存限制用于限制继续缓冲，不表示会主动裁剪已经缓冲的内容。

## 5. `VideoSurface` Widget

`VideoSurface` 是当前唯一内建的视频显示 widget。它只负责显示视频帧或 placeholder，不提供播放按钮、进度条、音量条等 controls。

支持的主要 builder 能力：

* 布局：`size`、`width`、`height`、`min_*`、`max_*`、`aspect_ratio`、`margin`、`padding`、`grow`、`shrink`、`basis`、grid row/column、absolute inset。
* 样式：`style`、`style_full`、`cursor`。
* 交互：`on_click`、`on_double_click`、`on_mouse_enter`、`on_mouse_leave`、`on_mouse_move`。
* 生命周期：`on_mount`、`on_unmount`、`on_update`。
* 媒体事件：`on_loading`、`on_success`、`on_error`。

`ContentFit` 不通过 `VideoSurface::fit(...)` 设置；当前应通过 `VideoSurfaceStyle` 设置：

```rust
use tgui::media::ContentFit;
use tgui::widgets::VideoSurfaceStyle;

VideoSurface::new(controller.clone())
    .size(dp(360.0), dp(202.0))
    .style(|style: &mut VideoSurfaceStyle, _| {
        style.fit = ContentFit::Contain;
    })
```

没有当前帧时，`VideoSurface` 会根据状态渲染 loading、error 或 idle placeholder。加载成功并有当前帧后，scene 中会生成 `VideoTexturePrimitive`，由渲染器按普通 sprite 路径绘制。

## 6. 推荐使用方式

ViewModel 中持有 `VideoController`，UI 中用普通 widget 组合控制栏：

```rust
use std::path::PathBuf;

use tgui::core::dp;
use tgui::layout::Axis;
use tgui::mvvm::{Command, Signal, TextController, ViewModelContext};
use tgui::video::{VideoController, VideoPlaybackState, VideoSource, VideoSurface};
use tgui::widgets::{Button, Element, Flex, Input, Text};

struct PlayerVm {
    controller: VideoController,
    source: TextController,
}

impl PlayerVm {
    fn new(ctx: &ViewModelContext) -> Self {
        let controller = VideoController::new(ctx);
        controller.set_volume(0.7);
        Self {
            controller,
            source: ctx.text_controller(""),
        }
    }

    fn status(&self) -> Signal<String> {
        self.controller.playback_state().map(|state| match state {
            VideoPlaybackState::Idle => "等待加载".to_string(),
            VideoPlaybackState::Loading => "加载中".to_string(),
            VideoPlaybackState::Ready => "准备播放".to_string(),
            VideoPlaybackState::Playing => "播放中".to_string(),
            VideoPlaybackState::Paused => "已暂停".to_string(),
            VideoPlaybackState::Buffering => "缓冲中".to_string(),
            VideoPlaybackState::Ended => "播放结束".to_string(),
            VideoPlaybackState::Error(error) => format!("播放失败: {error}"),
        })
    }

    fn load_from_input(&mut self) {
        let source = self.source.text();
        let source = source.trim();
        if source.starts_with("http://") || source.starts_with("https://") {
            let _ = self.controller.load(VideoSource::url(source));
        } else {
            let _ = self
                .controller
                .load(VideoSource::File(PathBuf::from(source)));
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .gap(dp(8.0))
            .child(Input::new(self.source.clone()).placeholder("视频文件路径或 URL"))
            .child(Button::new("加载").on_click(Command::new(Self::load_from_input)))
            .child(VideoSurface::new(self.controller.clone()).size(dp(360.0), dp(202.0)))
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(8.0))
                    .child(Button::new("播放").on_click(Command::new(|vm: &mut Self| {
                        vm.controller.play();
                    })))
                    .child(Button::new("暂停").on_click(Command::new(|vm: &mut Self| {
                        vm.controller.pause();
                    }))),
            )
            .child(Text::new(self.status()))
            .into()
    }
}
```

当前 `examples/demo` 的媒体页就是这种模式：输入路径或 URL 后显式加载，`VideoSurface` 固定尺寸展示，播放/暂停由 `VideoController` 控制。

运行示例：

```bash
cargo run --manifest-path examples/demo/Cargo.toml
```

## 7. 当前实现边界

已实现：

* `video` / `video-static` feature。
* `VideoController`、`VideoSurface` 和相关类型导出。
* 本地文件和 FFmpeg 可打开的 URL。
* URL headers。
* 首帧展示。
* 播放、暂停、结束后 replay。
* seek。
* 音量、静音。
* duration、position、buffered、video size、error Signal。
* loading、ready、playing、paused、buffering、ended、error 状态。
* 基于 FFmpeg 的视频解码、音频解码/输出和音频主时钟同步。
* 无音频流时的软件时钟。
* 根据布局目标尺寸设置 raster request，输出 RGBA `TextureFrame`。
* `wgpu` texture cache 上传和 revision 更新。
* `VideoSurfaceStyle` 与 theme/style sheet 集成。

未实现或不应写成已支持：

* 高层 `Video` 组合控件。
* 默认 controls。
* poster。
* autoplay。
* looping。
* 倍速 / `set_rate`。
* 字幕、多音轨、多字幕轨。
* 全屏、画中画。
* HLS / DASH 的专用流媒体抽象。
* 硬件解码调度。
* `VideoSource::Bytes`。
* Android / OHOS / 原生移动端后端。
* 外部可插拔 backend public API。

## 8. 维护注意事项

视频相关代码横跨多个高风险路径：

* `src/video/controller.rs`
* `src/video/types.rs`
* `src/video/backend/mod.rs`
* `src/video/backend/ffmpeg/`
* `src/ui/widget/video.rs`
* `src/ui/widget/core/render/media.rs`
* `src/ui/widget/core/resolved/collect/layout_media.rs`
* `src/rendering/renderer/texture.rs`
* `src/rendering/renderer/prepare.rs`

修改时需要特别注意：

* 不要在 UI 线程执行 demux、decode 或阻塞 IO。
* `VideoSurface` render/collect 阶段只应读取 snapshot、设置 target raster、生成 primitive。
* 后端更新当前帧后必须通过 shared invalidation 请求重绘，否则画面不会刷新。
* seek 会新开 generation，旧 generation 的解码帧必须被忽略。
* metrics 是惰性观测；新增指标时要考虑未订阅时的写入成本。
* 缓冲策略同时受音频缓冲、视频队列、EOF 和内存限制影响，避免只改一侧。
* `TextureFrame` 的 `id` / `revision` 语义影响 texture cache 复用，视频帧更新应保持同一 stream id、递增 revision。
* `VideoSurface` 的 placeholder、loading、error 路径也要覆盖测试，不要只测有帧路径。
* 公共 API 变更要同步检查 `src/lib.rs` 的 `tgui::video`、`prelude` 和 `widgets` re-export。

## 9. 建议测试和检查

文档变更不需要跑完整测试。代码变更建议至少按影响面选择：

```bash
cargo check --features video
CARGO_PROFILE_TEST_DEBUG=0 cargo test --features video video
CARGO_PROFILE_TEST_DEBUG=0 cargo test --features video command_video_tests
CARGO_PROFILE_TEST_DEBUG=0 cargo test --features video audio_video_tests
```

涉及 FFmpeg 链接或真实播放链路时，还应在桌面环境运行 `examples/demo` 并实际加载一个本地视频或 URL。

视频 benchmark 位于 `benches/video_pipeline`，需要：

```bash
cargo bench --features bench-support,video --bench video_pipeline
```

## 10. FFmpeg 链接方式

默认启用 `video` feature 时，`tgui` 沿用 `ffmpeg-next` 的常规链接方式：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["video"] }
```

如果调用方希望静态链接 FFmpeg，可以启用：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["video-static"] }
```

说明：

* `video-static` 会自动包含 `video`。
* 它会把 `ffmpeg-next/static` 传递给上游依赖。
* 构建环境仍然需要提供 FFmpeg 静态库和头文件。
* 实际支持的容器和 codec 仍取决于链接到的 FFmpeg 构建。

