# 环境配置

本文说明如何在 Windows、macOS 和 Linux 上配置 `tgui` 开发环境，并构建仓库示例。普通窗口、布局、组件和 Canvas 示例只需要 Rust 与平台编译工具；`examples/demo` 默认启用了 `audio` / `video`，还需要 FFmpeg 开发库和 `libclang`。

## 通用要求

- Rust stable。
- Git。
- 平台 C/C++ 编译工具。
- 支持 `wgpu` 的显卡驱动。

安装 Rust 后确认工具链：

```sh
rustup show
cargo --version
rustc --version
```

基础示例可用于确认普通 GUI 链路：

```sh
cargo build --manifest-path examples/basic_window/Cargo.toml
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
```

## Windows

Windows 推荐使用 MSVC 工具链：

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc
```

同时安装 Visual Studio 2022 或 Build Tools，并在 Visual Studio Installer 中选择“使用 C++ 的桌面开发”。至少需要：

- MSVC v143 x64/x86 build tools
- Windows 10 或 Windows 11 SDK
- C++ CMake tools for Windows

### FFmpeg

Windows MSVC 下推荐用 `vcpkg` 安装 FFmpeg 动态库：

```powershell
git clone https://github.com/microsoft/vcpkg.git $env:USERPROFILE\vcpkg
& $env:USERPROFILE\vcpkg\bootstrap-vcpkg.bat -disableMetrics
& $env:USERPROFILE\vcpkg\vcpkg.exe install ffmpeg[avcodec,avformat,swresample,swscale]:x64-windows
```

`examples/demo` 启用 `video`，vcpkg 可能额外安装 `avdevice` / `avfilter` 等组件，这是正常的。

设置用户环境变量：

```powershell
[Environment]::SetEnvironmentVariable('VCPKG_ROOT', "$env:USERPROFILE\vcpkg", 'User')
[Environment]::SetEnvironmentVariable('VCPKGRS_DYNAMIC', '1', 'User')
[Environment]::SetEnvironmentVariable('VCPKG_DEFAULT_TRIPLET', 'x64-windows', 'User')
```

动态链接 FFmpeg 时，运行环境还需要能找到 vcpkg 的 DLL：

```powershell
$vcpkgBin = "$env:USERPROFILE\vcpkg\installed\x64-windows\bin"
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if (($userPath -split ';') -notcontains $vcpkgBin) {
    [Environment]::SetEnvironmentVariable('PATH', "$userPath;$vcpkgBin", 'User')
}
```

### libclang

`ffmpeg-sys-next` 会使用 `bindgen` 生成绑定，因此需要 `libclang.dll`。推荐安装 LLVM：

```powershell
winget install --id LLVM.LLVM -e --accept-source-agreements --accept-package-agreements
```

设置 `LIBCLANG_PATH`：

```powershell
[Environment]::SetEnvironmentVariable('LIBCLANG_PATH', 'C:\Program Files\LLVM\bin', 'User')
```

当前终端临时设置：

```powershell
$env:VCPKG_ROOT = "$env:USERPROFILE\vcpkg"
$env:VCPKGRS_DYNAMIC = '1'
$env:VCPKG_DEFAULT_TRIPLET = 'x64-windows'
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
$env:PATH = 'C:\Program Files\LLVM\bin;' + "$env:USERPROFILE\vcpkg\installed\x64-windows\bin;" + $env:PATH
```

### 构建 demo

```powershell
cargo build --manifest-path examples\demo\Cargo.toml
```

debug 产物位于：

```text
examples\demo\target\debug\demo.exe
```

## macOS

macOS 推荐先安装 Xcode Command Line Tools：

```sh
xcode-select --install
rustup default stable
```

### FFmpeg 和 libclang

推荐用 Homebrew 安装 FFmpeg、LLVM 和 pkg-config：

```sh
brew install ffmpeg llvm pkg-config
```

设置当前 shell 环境变量：

```sh
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
export PATH="$(brew --prefix llvm)/bin:$PATH"
export PKG_CONFIG_PATH="$(brew --prefix ffmpeg)/lib/pkgconfig:$PKG_CONFIG_PATH"
```

如果使用 zsh，可以写入 `~/.zshrc`：

```sh
cat >> ~/.zshrc <<'EOF'
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
export PATH="$(brew --prefix llvm)/bin:$PATH"
export PKG_CONFIG_PATH="$(brew --prefix ffmpeg)/lib/pkgconfig:$PKG_CONFIG_PATH"
EOF
```

### 构建 demo

```sh
cargo build --manifest-path examples/demo/Cargo.toml
```

debug 产物位于：

```text
examples/demo/target/debug/demo
```

macOS 打包为 `.app` 时，需要同时考虑 FFmpeg 动态库的查找路径。开发阶段可通过 `DYLD_LIBRARY_PATH` 临时验证：

```sh
export DYLD_LIBRARY_PATH="$(brew --prefix ffmpeg)/lib:$DYLD_LIBRARY_PATH"
```

正式分发时建议使用打包工具或 `install_name_tool` 处理动态库路径。

## Linux

Linux 需要系统 C/C++ 工具链、pkg-config、窗口系统相关开发包，以及音视频场景下的 FFmpeg 开发包。不同发行版包名略有差异。

### Ubuntu / Debian

```sh
sudo apt update
sudo apt install -y build-essential pkg-config clang libclang-dev \
  libavcodec-dev libavformat-dev libavutil-dev libswresample-dev libswscale-dev \
  libx11-dev libxkbcommon-dev libwayland-dev libasound2-dev
```

某些发行版或桌面环境下，对话框、剪贴板、Wayland / X11 后端可能还需要 GTK、DBus、PipeWire 或 XCB 相关开发包：

```sh
sudo apt install -y libgtk-3-dev libdbus-1-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

### Fedora

```sh
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config clang clang-devel \
  ffmpeg-devel libX11-devel libxkbcommon-devel wayland-devel alsa-lib-devel \
  gtk3-devel dbus-devel libxcb-devel
```

### Arch Linux

```sh
sudo pacman -S --needed base-devel pkgconf clang ffmpeg \
  libx11 libxkbcommon wayland alsa-lib gtk3 dbus libxcb
```

### 环境变量

大多数 Linux 发行版安装 `libclang-dev` / `clang-devel` 后，`bindgen` 可以自动找到 libclang。如果仍然失败，可手动设置：

```sh
export LIBCLANG_PATH=/usr/lib/llvm-18/lib
```

实际路径按发行版不同可能是 `/usr/lib/llvm-17/lib`、`/usr/lib/llvm-18/lib`、`/usr/lib64` 等，可用下面命令查找：

```sh
find /usr -name 'libclang.so*' 2>/dev/null
```

### 构建 demo

```sh
cargo build --manifest-path examples/demo/Cargo.toml
```

debug 产物位于：

```text
examples/demo/target/debug/demo
```

如果运行时报找不到 FFmpeg 动态库，请确认系统动态链接器能找到 `libavcodec.so`、`libavformat.so`、`libavutil.so`、`libswresample.so` 和 `libswscale.so`。系统包安装路径通常无需额外设置；自定义安装时可临时使用：

```sh
export LD_LIBRARY_PATH=/path/to/ffmpeg/lib:$LD_LIBRARY_PATH
```

## 常见问题

### 找不到 FFmpeg

Windows 下确认 vcpkg：

```powershell
$env:VCPKG_ROOT
& $env:VCPKG_ROOT\vcpkg.exe list | Select-String ffmpeg
```

macOS / Linux 下确认 pkg-config：

```sh
pkg-config --libs --cflags libavutil
pkg-config --libs --cflags libavcodec libavformat libswresample libswscale
```

### 找不到 libclang

Windows：

```powershell
Test-Path 'C:\Program Files\LLVM\bin\libclang.dll'
$env:LIBCLANG_PATH
```

macOS：

```sh
ls "$(brew --prefix llvm)/lib/libclang.dylib"
echo "$LIBCLANG_PATH"
```

Linux：

```sh
find /usr -name 'libclang.so*' 2>/dev/null
echo "$LIBCLANG_PATH"
```

### 运行时找不到 FFmpeg 动态库

Windows：

```powershell
$env:PATH = "$env:USERPROFILE\vcpkg\installed\x64-windows\bin;" + $env:PATH
```

macOS：

```sh
export DYLD_LIBRARY_PATH="$(brew --prefix ffmpeg)/lib:$DYLD_LIBRARY_PATH"
```

Linux：

```sh
export LD_LIBRARY_PATH=/path/to/ffmpeg/lib:$LD_LIBRARY_PATH
```

Windows 安装 LLVM 后，也可以查看 `demo.exe` 依赖：

```powershell
llvm-objdump -p examples\demo\target\debug\demo.exe | Select-String 'DLL Name'
```
