# 媒体

媒体系统负责图片、SVG、网络资源、本地资源、内存 bytes，以及可选的音频和视频能力。应用侧通常只接触 `MediaSource`、`Image`、`ContentFit`、`AudioController` 和 `VideoController`；解码、栅格化、纹理缓存和加载完成后的 invalidation 由运行时处理。

## MediaSource

`MediaSource` 表示资源来源：

| 来源 | 适合场景 |
| --- | --- |
| 本地路径 | 用户选择的图片、应用资源目录中的图片。 |
| URL | 头像、封面、远程缩略图。 |
| 内存 bytes | 打包资源、缓存、数据库或网络层已经拿到的字节。 |

```rust
Image::from_path("assets/logo.png")
    .size(dp(160.0), dp(80.0))
    .style(|style, _ctx| {
        style.fit = ContentFit::Contain;
    })
```

```rust
Image::from_url("https://example.com/avatar.png")
    .size(dp(48.0), dp(48.0))
    .style(|style, _ctx| {
        style.fit = ContentFit::Cover;
        style.surface.border_radius = Some(dp(24.0).into());
    })
    .on_error(ValueCommand::new(|vm: &mut AppVm, message| {
        vm.status.set(format!("图片加载失败: {message}"));
    }))
```

如果资源来自内存，优先把字节保存为共享缓冲：

```rust
use std::sync::Arc;
use tgui::media::{MediaBytes, MediaSource};

let shared: Arc<[u8]> = load_asset_bytes();
let source = MediaSource::bytes(MediaBytes::from_shared(shared.clone()));
let image = Image::new(source).style(|style, _ctx| {
    style.fit = ContentFit::Contain;
});
```

`MediaBytes::from_shared(...)` 让 clone、hash 和跨组件复用只复制指针与引用计数，不会重复复制整张图片。如果当前只有 `Vec<u8>`，也可以直接传给 `MediaSource::bytes(vec)`。

## Image

`Image` 支持 raster 图片和 SVG。raster 图片会按目标物理像素异步解码并纹理化；SVG 会按目标尺寸栅格化并使用小型缓存。加载失败时会显示 placeholder。

常用 API：

| API | 说明 |
| --- | --- |
| `Image::new(source)` | 使用任意 `MediaSource`。 |
| `Image::from_path(path)` | 从本地路径加载。 |
| `Image::from_url(url)` | 从网络 URL 加载。 |
| `Image::from_bytes(bytes)` | 从内存 bytes 加载。 |
| `on_loading(...)` | 首次开始加载时回调。 |
| `on_success(...)` | 资源加载成功时回调。 |
| `on_error(...)` | 加载失败时回调，payload 为错误文本。 |
| `style(...)` / `style_full(...)` | 覆盖图片样式。 |

图片填充方式通过 `ImageStyle.fit` 设置：

```rust
Image::from_path("assets/photo.jpg")
    .style(|style, _ctx| {
        style.fit = ContentFit::Cover;
    })
```

`ContentFit` 常见选择：

| 模式 | 行为 |
| --- | --- |
| `Contain` | 完整显示资源，可能留空白。适合 logo、图标、截图。 |
| `Cover` | 填满区域，可能裁剪。适合头像、封面、卡片图。 |
| `Fill` | 拉伸到区域尺寸。适合不要求比例的纹理或背景。 |
| `None` | 使用资源自身尺寸。适合像素精确内容。 |
| `ScaleDown` | 小图不放大，大图缩小到区域内。 |

## 背景图片

普通容器也可以设置背景图片，用于卡片封面、窗口背景或仪表盘装饰层。背景图片属于视觉样式，仍参与媒体缓存和异步刷新。

```rust
Stack::new()
    .size(dp(420.0), dp(220.0))
    .style(|style, _ctx| {
        style.surface.background_image =
            Some(BackgroundImage::from_path("assets/cover.jpg").fit(ContentFit::Cover).into());
        style.surface.border_radius = Some(dp(8.0).into());
    })
    .child(Text::new("Project Atlas"))
```

当背景图片只是内容的一部分、需要独立 loading/error 回调或需要参与布局时，优先使用 `Image`；当图片只是容器表面的一层视觉装饰时，使用背景图片。

## Canvas 图片

Canvas recorder 和 retained scene 也可以绘制媒体资源：

```rust
CanvasRecorder::build(|canvas| {
    canvas.draw_image_with_options(
        Rect::from_xywh(dp(0.0), dp(0.0), dp(320.0), dp(180.0)),
        MediaSource::path("assets/chart-bg.svg"),
        CanvasImageOptions::new()
            .fit(ContentFit::Cover)
            .corner_radius(dp(8.0)),
    );
})
```

Canvas 图片适合图表、白板、编辑器和自定义节点，不适合替代普通布局中的 `Image`。

## 音频

启用 `audio` feature 后可使用音频能力：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["audio"] }
```

核心类型：

| 类型 | 说明 |
| --- | --- |
| `AudioController` | 加载、播放、暂停、seek、音量、静音和状态查询。 |
| `Audio` | 挂接到 widget tree 的隐形播放组件。 |
| `AudioSource` | 音频资源来源。 |
| `PlaybackState` | 播放状态。 |
| `AudioMetrics` | 播放时长、位置等指标。 |

典型结构是 ViewModel 持有 `AudioController`，视图树中放一个 `Audio::new(controller.clone())`，按钮命令调用控制器方法。`Audio` 本身不渲染 UI，但它负责把播放生命周期接入组件树。

## 视频

启用 `video` feature 后可使用视频能力，`video` 会自动带上音频能力：

```toml
[dependencies]
tgui = { version = "0.2.0", features = ["video"] }
```

核心类型：

| 类型 | 说明 |
| --- | --- |
| `VideoController` | 管理加载、播放、暂停、seek、音量、静音和缓冲限制。 |
| `Video` | 带内置控制栏的视频播放器。 |
| `VideoSurface` | 只渲染视频画面，适合自定义控制栏。 |
| `VideoSource` | 视频资源来源。 |

```rust
Video::new(self.video.clone())
    .show_controls(true)
    .show_status(true)
    .show_volume(true)
    .fit(ContentFit::Contain)
```

需要完全自定义播放器 UI 时使用 `VideoSurface`，再用普通 `Button`、`Slider`、`Text` 组合控制栏。

## 资源预算

应用可以通过 `Application::resource_budget(...)` 调整图片、SVG、Canvas shadow 和 widget shadow 等缓存容量。

```rust
Application::new()
    .resource_budget(ResourceBudget::compact())
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

调优建议：

- 图片很多且尺寸重复：尽量复用同一个 `MediaSource` 或共享 `MediaBytes`。
- SVG 图标很多：优先统一目标尺寸，减少同一 SVG 的多尺寸栅格缓存。
- 网络图片加载慢：保留旧纹理或展示 skeleton，不要阻塞 ViewModel 构建。
- 内存敏感应用：从 `ResourceBudget::compact()` 开始，再按实际界面调大。
