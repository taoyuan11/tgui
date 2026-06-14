# 媒体

媒体系统负责图片、SVG、网络资源、本地资源、内存 bytes，以及可选的音频和视频能力。

## 图片

`MediaSource` 支持：

- 本地路径。
- URL。
- 内存 bytes。

`Image` 支持 raster 图片和 SVG。raster 图片会按目标物理像素异步解码并纹理化；SVG 会按目标尺寸栅格化并使用小型缓存。加载失败时会显示 placeholder。

常用相关类型：

- `MediaSource`
- `MediaBytes`
- `ContentFit`

## 内存 bytes 与零拷贝

如果图片来自资源包、缓存或其他已经共享的数据结构，优先把底层缓冲区保存为
`Arc<[u8]>`，再通过 `MediaBytes::from_shared(...)` 或 `MediaSource::bytes(...)`
传入媒体系统。这样 clone、hash 和跨组件复用只复制指针与引用计数，不会重复复制整张图片。

```rust
use std::sync::Arc;
use tgui::media::{MediaBytes, MediaSource};

let shared: Arc<[u8]> = load_asset_bytes();
let source = MediaSource::bytes(MediaBytes::from_shared(shared.clone()));
```

如果当前只有新建的 `Vec<u8>`，可以直接传给 `MediaSource::bytes(vec)`；之后的
`MediaBytes` clone 仍会共享同一份缓冲区。需要在多处复用同一资源时，尽量在加载层先
缓存 `Arc<[u8]>`，再分发给 `Image`、Canvas image 或背景图片。

## 音频

启用 `audio` feature 后可使用：

- `Audio`
- `AudioController`
- `AudioSource`
- `PlaybackState`
- `AudioMetrics`

`Audio` 是挂接到组件树上的隐形播放组件；控制器负责加载、播放、暂停、seek、音量和静音。

## 视频

启用 `video` feature 后可使用：

- `Video`
- `VideoSurface`
- `VideoController`
- `VideoSource`

`Video` 是浏览器式内置控制栏播放器，组合了画面、底部 SVG 图标控制栏、播放/暂停、seek、缓冲、时间、音量/静音和状态文本。`VideoSurface` 参与布局和渲染，适合把视频画面作为普通 UI 区域嵌入应用并自行组合控制栏。视频 feature 会带上音频能力。

## 资源预算

应用可以通过 `Application::resource_budget(...)` 调整图片、SVG、Canvas shadow 和 widget shadow 等缓存容量。内存敏感环境可使用 `ResourceBudget::compact()`。
