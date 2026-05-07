# tgui 视频组件最佳实践

## 1. 目标

本文档定义在 `tgui` 中实现视频组件时的推荐设计方式，目标是：

* 与现有 `MVVM + 响应式状态 + 声明式 Widget Tree` 架构保持一致
* 与 `wgpu` 渲染链路保持统一
* 为 desktop、Android、OHOS 预留稳定的跨平台扩展点
* 避免把“播放器逻辑”“平台解码”“UI 控件”“渲染上传”耦合在一起
* 先稳定 API，再逐步扩展硬件解码、流媒体、字幕、全屏等能力

---

## 2. 为什么视频组件不能直接照搬 `Image`

`tgui` 当前已经具备 `Image` 组件，并支持本地文件和 `http/https` 资源加载，还暴露了 `on_loading / on_success / on_error` 之类的媒体加载回调。与此同时，框架整体是围绕 `Application`、`Observable<T>`、`Binding<T>`、`Command`、布局容器和核心控件来组织的。

但视频和图片在本质上不同：

* `Image` 是一次性资源加载
* `Video` 是持续运行的状态机
* `Image` 不涉及音频同步
* `Video` 需要解码、缓冲、时钟推进、帧刷新、前后台恢复、播放控制

因此，**不推荐把视频做成“Image 的增强版”**。
最佳实践是：

> **上层做成声明式 `Video` Widget，下层拆分为 `Controller` 与 `Backend`，视频帧统一进入 `wgpu texture` 渲染链。**

---

## 3. 推荐的总体架构

建议将视频能力拆成三层：

### 3.1 `Video` Widget 层

职责：

* 参与布局
* 参与样式系统
* 参与事件系统
* 组合控制栏、加载态、错误态、封面图等 UI

这一层应该表现得像普通 `Widget` 一样，支持与现有组件一致的链式 API，例如：

* `width / height / fill_width`
* `background / border / border_radius / opacity`
* `offset / overflow`
* `on_click / on_mouse_move`

### 3.2 `VideoController` 层

职责：

* 管理播放状态
* 提供播放命令
* 向 ViewModel 和 Widget 暴露响应式状态

这是视频能力与 `MVVM` 体系结合的关键层。

### 3.3 `VideoBackend` 层

职责：

* 打开媒体源
* 解析音视频流
* 解码音频 / 视频
* 管理缓冲与同步
* 输出视频帧
* 输出音频
* 处理平台生命周期差异

这一层不直接关心 UI 布局，只提供播放器能力。

---

## 4. 推荐的核心原则

## 4.1 原则一：UI 与解码分离

不要在 `Video` widget 内直接处理：

* demux
* decode
* audio output
* seek pipeline
* 网络缓冲

这些逻辑都应该放到 `VideoBackend` 中。

`Video` widget 只负责：

* 展示当前帧
* 展示当前播放状态
* 将用户操作转成 `Controller` 调用

---

## 4.2 原则二：播放器状态必须是响应式的

`tgui` 当前以 `Observable<T>`、`Binding<T>`、`Command` 为核心，这意味着视频状态也应该走同样的机制，而不是自行维护一套独立的 UI 通知系统。

推荐暴露以下状态：

* `playback_state`
* `current_position`
* `duration`
* `buffered_position`
* `volume`
* `muted`
* `playback_rate`
* `video_size`
* `error`

这些都应当能自然接到：

* `Text`
* 进度条
* 播放/暂停按钮
* 加载动画
* 错误提示 UI

另外，建议把“什么时候可以播放”和“最多继续缓冲到哪里”拆成两个概念：

* `buffer target`：达到后表示已经可播放/可恢复播放
* `memory limit`：达到后继续缓冲停止增长，但不裁掉已缓冲内容

---

## 4.3 原则三：视频帧统一走 `wgpu texture`

`tgui` 已经是以 `wgpu` 为底层渲染引擎的 GUI 框架。视频最推荐的接入方式是：

> **解码得到视频帧 → 转换或上传为 GPU texture → 由 `Video` widget 在当前渲染流程中绘制。**

这样做的好处：

* 渲染体系统一
* 容易支持圆角、透明度、裁剪、叠加层
* 控制栏、字幕、封面、错误层都可以与 widget tree 直接组合
* 桌面、Android、OHOS 的表现更容易保持一致

不推荐将平台原生视频视图直接嵌入主 widget tree，因为那通常会带来：

* 样式不一致
* 圆角和裁剪困难
* 悬浮控件叠加复杂
* 不同平台行为分裂

---

## 4.4 原则四：平台差异通过 Backend 抽象，而不是渗透到 Widget API

`tgui` 当前已明确支持 Android 和 OHOS，并且这两个平台已经有各自的 runtime / surface / 字体 / 生命周期适配路径。视频组件也应遵循这一方向，而不是假设所有平台共用一套实现。

推荐做法：

* 对外只有统一的 `Video` / `VideoController` API
* 对内通过 `VideoBackend` trait 做平台分发

例如：

* Desktop：先使用 `ffmpeg` 后端
* Android：后续接原生解码后端
* OHOS：后续接原生媒体能力或平台适配后端

这样可以保证：

* UI 层 API 长期稳定
* 平台能力逐步增强时不需要大改上层使用方式

---

## 5. 推荐的数据模型

## 5.1 视频源

```rust
pub enum VideoSource {
    File(std::path::PathBuf),
    Url {
        url: String,
        headers: Vec<(String, String)>,
    },
    Bytes(std::sync::Arc<[u8]>),
}
```

建议第一版至少支持：

* `File`
* `Url`

`Bytes` 可用于内存流、加密流或上层自定义数据源。

对于需要鉴权、来源校验或 Cookie 的网络视频，可以给 `Url` 同时带上自定义请求头。例如：

```rust
let source = VideoSource::url("https://example.com/demo.mp4")
    .with_header("Authorization", "Bearer <token>")
    .with_headers([
        ("Referer", "https://example.com/player"),
        ("Cookie", "session=abc123"),
    ]);
```

---

## 5.2 播放状态

```rust
pub enum PlaybackState {
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

不建议只用 `bool is_playing`，因为视频远不止“播放/暂停”两态。

---

## 5.3 播放指标

```rust
pub struct VideoMetrics {
    pub duration: Option<std::time::Duration>,
    pub position: std::time::Duration,
    pub buffered: Option<std::time::Duration>,
    pub video_width: u32,
    pub video_height: u32,
}
```

---

## 5.4 控制器

```rust
pub struct VideoController {
    // 内部持有响应式状态与后端句柄
}
```

建议至少提供：

```rust
impl VideoController {
    pub fn play(&self);
    pub fn pause(&self);
    pub fn seek(&self, position: std::time::Duration);
    pub fn set_volume(&self, volume: f32);
    pub fn set_muted(&self, muted: bool);
    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64);
    pub fn set_rate(&self, rate: f32);

    pub fn playback_state(&self) -> tgui::mvvm::Binding<PlaybackState>;
    pub fn position(&self) -> tgui::mvvm::Binding<std::time::Duration>;
    pub fn duration(&self) -> tgui::mvvm::Binding<Option<std::time::Duration>>;
    pub fn muted(&self) -> tgui::mvvm::Binding<bool>;
}
```

---

## 6. 推荐的 Widget API 设计

推荐让 `Video` 保持和现有 `tgui` 控件一致的 builder 风格。README 中当前控件和容器都采用链式声明式 API，这一点应继续保持。

示例：

```rust
Video::new(controller.clone())
    .source(VideoSource::File("demo.mp4".into()))
    .autoplay(false)
    .looping(false)
    .muted(false)
    .controls(true)
    .poster(ImageSource::File("cover.jpg".into()))
    .fit(ContentFit::Contain)
    .border_radius(16.0)
    .fill_width()
    .height(320.0)
```

推荐暴露的配置项：

* `source(...)`
* `autoplay(...)`
* `looping(...)`
* `muted(...)`
* `controls(...)`
* `poster(...)`
* `fit(...)`
* `show_loading(...)`
* `show_error_overlay(...)`

---

## 7. 推荐的内部模块划分

建议采用如下模块组织：

```text
tgui-video/
├─ mod.rs
├─ controller.rs
├─ widget.rs
├─ backend/
│  ├─ mod.rs
│  ├─ ffmpeg.rs
│  ├─ android.rs
│  └─ ohos.rs
├─ render/
│  ├─ frame.rs
│  ├─ texture_cache.rs
│  └─ uploader.rs
├─ audio/
│  ├─ mod.rs
│  └─ output.rs
└─ types.rs
```

### 各模块职责

* `controller.rs`
  响应式状态、命令入口、对外 API

* `widget.rs`
  `Video` / `VideoSurface` / 控制栏组合

* `backend/*`
  不同平台的实际播放实现

* `render/*`
  帧缓存、像素格式转换、纹理上传

* `audio/*`
  音频输出抽象

* `types.rs`
  `VideoSource`、`PlaybackState`、`VideoMetrics`、错误类型等

---

## 8. UI 层的最佳实践

## 8.1 视频显示层与控制层分开

推荐拆成两个组件：

### `VideoSurface`

只负责显示视频帧，不带控制按钮。

### `Video`

通过 `Stack` 组合：

* `VideoSurface`
* 封面图
* 加载态
* 中间播放按钮
* 底部控制栏
* 错误提示层

因为 `tgui` 已经具备 `Stack / Flex / Text / Button / Container` 等构件，这种组合方式最符合现有设计。

---

## 8.2 控制栏不要写死在底层渲染器里

不要让底层 renderer 直接负责：

* 进度条
* 播放按钮
* 时间文本
* 音量条

这些应该由普通 widget 组合出来。原因是：

* 更容易定制样式
* 更容易换皮肤
* 更方便做桌面端和移动端不同交互
* 更符合声明式 UI 的设计习惯

---

## 8.3 布局稳定性优先

README 中提到 `Image` 在媒体加载时为了避免布局跳变，推荐显式设置尺寸或 `aspect_ratio(...)`。视频组件也应遵循同样原则。

最佳实践：

* 在媒体真正 ready 前，优先使用固定高度或宽高比
* 有 `poster` 时先显示封面
* 没有封面时显示占位背景与 loading 状态
* 第一帧就绪后再切换到视频纹理

---

## 9. 后端实现的最佳实践

## 9.1 先做桌面后端验证 API

第一阶段建议优先只做 desktop：

* 文件输入
* MP4（H264/AAC）
* 播放 / 暂停 / seek
* 音量 / 静音
* 首帧显示
* 状态回调
* 错误处理

先把这些 API 跑通，再推广到 Android / OHOS。

---

## 9.2 统一后端 trait

建议定义后端接口：

```rust
pub trait VideoBackend: Send + Sync {
    fn load(&self, source: VideoSource) -> Result<(), VideoError>;
    fn play(&self);
    fn pause(&self);
    fn seek(&self, position: std::time::Duration);
    fn set_volume(&self, volume: f32);
    fn set_muted(&self, muted: bool);
    fn set_rate(&self, rate: f32);
    fn poll_frame(&self) -> Option<VideoFrame>;
}
```

说明：

* `Widget` 不依赖具体后端
* `Controller` 也尽量不依赖具体后端
* 平台切换只影响 backend 创建逻辑

---

## 9.3 线程模型建议

不要在 UI 线程里做解码。

推荐最小线程模型：

* 一个媒体读取/解复用任务
* 一个视频解码任务
* 一个音频输出任务
* UI 线程只做：

    * 读取最新帧
    * 上传 texture
    * 请求重绘

推荐原则：

* UI 线程只拿“当前应该显示的最新帧”
* 不要让 UI 线程阻塞等待解码
* 避免在 widget render 阶段执行 IO 和重 CPU 任务

---

## 9.4 生命周期处理要提前设计

README 已说明 Android 与 OHOS 都涉及 surface lifecycle / 前后台恢复等运行时能力。视频组件必须预留这些钩子，否则后续移动端会非常难补。

建议后端抽象中至少考虑：

* `on_surface_lost`
* `on_surface_restored`
* `on_app_background`
* `on_app_foreground`

哪怕第一版暂时只是 desktop，也建议接口先留好。

---

## 10. ViewModel 层的最佳实践

推荐在 ViewModel 中持有 `VideoController`，而不是让 widget 自己偷偷创建播放器状态。

示例：

```rust
struct PlayerVm {
    player: tgui::video::VideoController,
}

impl PlayerVm {
    fn new(ctx: &tgui::mvvm::ViewModelContext) -> Self {
        let player = tgui::video::VideoController::new(ctx);
        player.set_buffer_memory_limit_bytes(160 * 1024 * 1024);
        Self {
            player,
        }
    }

    fn play(&mut self) {
        self.player.play();
    }

    fn pause(&mut self) {
        self.player.pause();
    }

    fn view(&self) -> tgui::widgets::Element<Self> {
        tgui::video::Video::new(self.player.clone())
            .source(tgui::video::VideoSource::File("demo.mp4".into()))
            .controls(true)
            .height(320.0)
            .fill_width()
            .into()
    }
}
```

这样做的优点：

* 与现有 `MVVM` 设计统一
* 状态和命令都可测试
* 可以很方便做多播放器页面
* 更容易实现“播放器状态和业务状态联动”

---

## 11. 第一版推荐功能边界

为了保证实现质量，第一版建议只做以下内容：

### 必做

* 本地文件播放
* 桌面平台
* MP4 容器
* 播放 / 暂停
* seek
* 当前时间 / 总时长
* 静音 / 音量
* 错误态
* 首帧展示
* `ContentFit`

### 选做

* `autoplay`
* `looping`
* `poster`
* `on_ended`
* `on_error`

### 暂缓

* HLS / DASH
* 字幕
* 全屏
* 画中画
* 倍速
* 视频滤镜
* 硬解码能力调度
* 多音轨 / 多字幕轨

---

## 12. 常见错误设计

以下做法不推荐：

### 12.1 把视频当作 `Image` 的扩展

问题：

* 状态模型太弱
* 无法自然表示 buffering / ended / seek 等行为
* 后期必然返工

### 12.2 在 Widget 内直接启动完整播放器

问题：

* UI 与后端耦合严重
* 状态难以测试
* 生命周期难处理

### 12.3 直接嵌平台原生视频视图

问题：

* 与 `wgpu` 渲染链断裂
* 样式、裁剪、叠加层困难
* 跨平台行为不一致

### 12.4 只暴露 `is_playing: bool`

问题：

* 难以表达真实状态机
* 业务逻辑会不断出现例外判断

---

## 13. 推荐的实施路线

## 阶段一：API 定型

目标：

* 完成 `VideoSource`
* 完成 `PlaybackState`
* 完成 `VideoController`
* 完成 `Video` / `VideoSurface` widget 结构
* 用 mock backend 跑通 UI

## 阶段二：桌面最小可用实现

目标：

* 本地 MP4 播放
* 解码视频帧
* 上传 `wgpu texture`
* 音频输出
* 支持播放 / 暂停 / seek

## 阶段三：网络流与稳定性

目标：

* URL 输入
* buffering 状态
* 错误恢复
* 更多格式支持

## 阶段四：移动端适配

目标：

* Android backend
* OHOS backend
* surface lifecycle
* 前后台恢复
* 触控交互优化

---

## 14. 推荐结论

在 `tgui` 中实现视频组件的最佳实践是：

1. **将视频设计成声明式 `Video` Widget**
2. **将播放状态与命令抽离为 `VideoController`**
3. **将解码与平台能力抽离为 `VideoBackend`**
4. **将视频帧统一接入 `wgpu texture` 渲染**
5. **将控制栏作为普通 widget 组合实现，而不是写死在底层**
6. **先做桌面最小能力，先稳定 API，再扩展移动端与高级特性**

这条路线最符合 `tgui` 当前的响应式 MVVM 架构、统一样式系统、GPU 渲染链路和跨平台目标。

---

## 15. 实现状态（2026-04-18）

本轮已完成：

* [x] 新增 `video` feature，并将视频能力挂到该 feature 下
* [x] 新增 `video-static` feature，可将 FFmpeg 切换为静态链接
* [x] 新增 `VideoSource`、`PlaybackState`、`VideoMetrics`、`VideoSize`
* [x] 新增 `VideoController`
* [x] 新增 `VideoSurface`
* [x] 将 `VideoSurface` 接入 widget tree、测量逻辑与 `wgpu texture` 渲染链
* [x] 新增 desktop `FfmpegVideoBackend`
* [x] 支持本地文件与 `http/https` 直链 MP4
* [x] 支持首帧、播放、暂停、seek、音量、静音、错误态
* [x] 接入基础音频输出与音频主时钟同步
* [x] 增加最小桌面示例 `examples/video_surface`

本轮暂未实现：

* [ ] 默认 controls 组合组件 `Video`
* [ ] HLS / DASH
* [ ] 字幕 / 多音轨 / 多字幕轨
* [ ] 全屏 / 画中画 / 倍速
* [ ] Android / OHOS backend

## 16. FFmpeg 链接方式

默认启用 `video` feature 时，`tgui` 会沿用 `ffmpeg-next` 的常规链接方式。

如果调用方希望把 FFmpeg 库静态链接进最终可执行程序，可以启用：

```toml
[dependencies]
tgui = { version = "0.1.4", features = ["video-static"] }
```

说明：

* `video-static` 会自动包含 `video`
* 它会把 `ffmpeg-next/static` 传递给上游依赖
* 这会把 FFmpeg 的链接类型切换为静态链接，避免运行时再额外分发对应动态库
* 构建环境仍然需要能提供 FFmpeg 的静态库和头文件


