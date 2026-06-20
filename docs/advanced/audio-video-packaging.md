# 音视频打包与 FFmpeg

`audio` 和 `video` feature 依赖 FFmpeg 与平台音频输出。开发阶段只要能编译运行即可；
分发应用时还需要处理动态库、许可和平台身份。

## Feature 选择

- `audio`：启用 FFmpeg 音频解码和 CPAL 输出。
- `video`：启用视频解码、画面呈现和音频同步，包含 `audio`。
- `video-static`：启用 `ffmpeg-next/static`，适合希望减少运行时动态库查找问题的构建。

静态链接并不自动解决 FFmpeg 许可义务。发布前需要确认所用 FFmpeg build 的 license、
codec 配置和再分发要求。

## Windows

推荐用 vcpkg 安装动态 FFmpeg，并把 DLL 随应用一起分发：

```powershell
vcpkg install ffmpeg[avcodec,avformat,swresample,swscale]:x64-windows
```

打包时确认：

- `avcodec`、`avformat`、`avutil`、`swresample`、`swscale` DLL 可被 exe 找到。
- `LIBCLANG_PATH` 只影响构建，不应成为运行时依赖。
- 通知功能需要稳定 `Application::app_id(...)` 和安装后 shortcut 身份。

## macOS

开发阶段可用 Homebrew：

```sh
brew install ffmpeg llvm pkg-config
```

`.app` 分发时确认：

- FFmpeg dylib 被复制到 bundle 或通过 `install_name_tool` 修正查找路径。
- 签名和 notarization 覆盖主程序与嵌入 dylib。
- 通知 action 需要 bundle 身份；裸二进制只能走普通通知 fallback。

## Linux

优先依赖发行版 FFmpeg 包，或在 AppImage/Flatpak/deb/rpm 中明确声明/携带动态库。

需要验证：

- `libavcodec.so`、`libavformat.so`、`libavutil.so`、`libswresample.so`、`libswscale.so`。
- ALSA/PulseAudio/PipeWire 输出路径。
- Wayland/X11、DBus、通知服务和沙盒权限。

## 故障排查

- 构建找不到 FFmpeg：检查 `pkg-config --libs --cflags libavcodec libavformat`。
- 构建找不到 libclang：设置 `LIBCLANG_PATH`。
- 运行找不到动态库：检查 `PATH`、`DYLD_LIBRARY_PATH`、`LD_LIBRARY_PATH` 或打包工具的依赖收集。
- AV1/特殊 codec 解码失败：确认 FFmpeg build 包含对应 decoder，例如 `dav1d` 或 `aom`。
