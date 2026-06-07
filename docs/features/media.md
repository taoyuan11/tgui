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

- `VideoSurface`
- `VideoController`
- `VideoSource`

`VideoSurface` 参与布局和渲染，适合把视频画面作为普通 UI 区域嵌入应用。视频 feature 会带上音频能力。

## 资源预算

应用可以通过 `Application::resource_budget(...)` 调整图片、SVG、Canvas shadow 和 widget shadow 等缓存容量。内存敏感环境可使用 `ResourceBudget::compact()`。
