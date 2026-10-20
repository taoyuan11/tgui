# 应用打包与安装器

`cargo tgui` 将应用构建为桌面安装包，并收集资源、动态库和产物校验和。工具是独立 crate，不会作为 GUI runtime 的依赖进入应用。

| 原生构建目标 | 输出 |
| --- | --- |
| `x86_64-pc-windows-msvc` | NSIS EXE |
| `aarch64-apple-darwin` | APP、DMG |
| `x86_64-apple-darwin` | APP、DMG |
| `x86_64-unknown-linux-gnu` | DEB、AppImage |

首版只接受与 `rustc -vV` 的 host 相同的目标。macOS 两种架构分别构建，不生成 universal binary；Linux CI 使用 Ubuntu 24.04，AppImage 仍受构建时 glibc 基线与目标显卡驱动约束。

## 安装工具

在 tgui 仓库中安装：

```sh
cargo install --path crates/cargo-tgui --locked
cargo tgui --help
```

需要 Rust 1.89 或更新版本。后端固定为 `cargo-packager 0.11.8`；首次生成 NSIS / AppImage 时，后端会下载所需的打包工具，因此需要网络。macOS 使用系统 `hdiutil` 创建 DMG。

本功能不执行 `cargo publish` 或创建 GitHub Release。上面的源码安装命令不依赖 `cargo-tgui` 已发布到 crates.io。

## 配置应用

配置位于**应用包**的 `Cargo.toml`，不能放到虚拟 workspace manifest 中：

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"
description = "My desktop application"
authors = ["Example <hello@example.com>"]

[package.metadata.tgui.bundle]
identifier = "com.example.my-app"
product-name = "My App"
publisher = "Example"
icons = ["assets/icon.png"]
resources = [
    { src = "assets", target = "assets" },
    { src = "config/default.json", target = "config/default.json" },
]
license-files = ["LICENSE", "THIRD-PARTY-NOTICES.txt"]
# 如构建环境无法自行定位动态库，可添加目录：
# library-search-paths = ["vendor/runtime/lib"]

[package.metadata.tgui.bundle.windows]
install-mode = "current-user" # 也可设为 per-machine，需要管理员权限
languages = ["English", "SimpChinese"]

[package.metadata.tgui.bundle.macos]
# 必须与二进制及所有动态库的实际最低系统版本一致。
# minimum-system-version = "15.0"

[package.metadata.tgui.bundle.linux]
depends = [] # 添加 dlopen 等导入表无法发现的 DEB 运行依赖
```

`identifier`、`product-name`、`publisher` 和非空 `icons` 必填。版本、描述、作者从 Cargo metadata 读取，支持 `workspace = true` 继承。应用的 `Application::app_id(...)` 应与 `identifier` 保持一致。

图标推荐提供正方形 512×512 PNG；Windows 可另提供 ICO，macOS 可另提供 ICNS。Windows 安装器图标由打包配置控制；应用窗口图标仍由 `Application::window_icon` 设置。Windows EXE 文件图标需在应用的 `build.rs` 中嵌入 Windows resource（例如使用 `winresource`）；NSIS 快捷方式继承 EXE 图标。`packaging` 示例已包含相应 `build.rs` 和 ICO 资源。

资源来源、图标、许可证、额外库目录，以及相对 `--out-dir` 都相对所选应用 `Cargo.toml` 解析。资源支持文件和目录映射，不支持 glob 或符号链接；目标必须为无 `..` 的可移植相对路径。大小写冲突、文件与目录冲突、Windows 保留文件名会提前报错。许可证被放入资源目录的 `licenses/`，因此同名许可证需要先整理为唯一名称。

## 构建命令

```sh
cargo tgui doctor --manifest-path path/to/app/Cargo.toml
cargo tgui bundle --manifest-path path/to/app/Cargo.toml --dry-run
cargo tgui bundle --manifest-path path/to/app/Cargo.toml --locked

# 从 workspace 选择 package 和 binary：
cargo tgui bundle -p my-app --bin my-app --features media --locked

# 只生成指定格式；可用逗号分隔同平台格式：
cargo tgui bundle -p my-app --format app,dmg
```

支持 `--profile`（默认 `release`）、`--target`、`--features`、`--all-features`、`--no-default-features`、`--locked`、`--format`、`--out-dir`。`--dry-run` 展示解析后的配置、Cargo 参数与执行步骤，不构建、不创建输出目录、不下载打包工具。多成员虚拟 workspace 和多 binary package 必须显式选择。

默认输出：

```text
<Cargo target directory>/tgui-dist/<package>/<target>/<profile>/
```

使用 Cargo JSON 构建消息查找二进制，因此支持自定义 `CARGO_TARGET_DIR`。默认保留 `.app` 目录，并额外提供保留权限的 `.app.tar.gz`，适合上传 CI artifact。

每次成功构建生成：

- 安装包 / 应用包。
- `SHA256SUMS`：安装包与 APP 压缩包的 SHA-256。
- `bundle-manifest.json`：应用版本、构建参数、后端版本、系统依赖、收集的动态库来源与原始文件校验和、输出清单。

安装包资源根目录还包含 `tgui-native-dependencies.json`，记录依赖来源、导入关系和原始文件 SHA-256；这个文件名由工具保留。DEB 中的库由系统包提供，依赖清单记录构建时解析到的版本来源。

不同 feature 组合应使用不同 `--out-dir`。只有上次清单中记录的同名产物允许被覆盖；不会清空任意指定的目录。并发写入同一目录会失败。中断后确认没有打包进程，再删除遗留 `.cargo-tgui.lock`。失败退出时普通临时目录会自动清理。

## 平台工具与 FFmpeg

普通构建工具见[环境配置](./environment)。`doctor` 检查本地必需工具，第三方打包工具的首次下载发生在实际打包阶段。

### Windows

使用 MSVC Developer PowerShell / Command Prompt，安装 Windows SDK。音视频构建使用动态 FFmpeg，例如 vcpkg 的 `x64-windows` triplet：

```powershell
$env:VCPKGRS_DYNAMIC = '1'
$env:VCPKG_DEFAULT_TRIPLET = 'x64-windows'
$env:PATH = "$env:VCPKG_ROOT\installed\x64-windows\bin;$env:PATH"
cargo tgui bundle --manifest-path path/to/app/Cargo.toml --features media --locked
```

工具检查 PE 架构，递归复制 FFmpeg、其传递依赖及所需 MSVC runtime DLL。会查询配置目录、构建环境 PATH、vcpkg 和 `VCToolsRedistDir`。Windows 系统 DLL 和 API sets 保留为系统依赖。无法定位或出现同名不同内容的库时构建失败，不静默漏包。

默认安装到当前用户目录，包含开始菜单入口、可选桌面快捷方式和卸载入口。升级沿用相同 `identifier`、`product-name`、`publisher`、binary 名称与安装模式；首版不处理应用改名或安装身份迁移。升级和卸载保留用户数据，不提供自动更新。

### macOS

安装 Xcode Command Line Tools；音视频构建还需 FFmpeg 开发库和 pkg-config。依赖分析支持 Mach-O、目标架构匹配的 fat library，以及 `@rpath` / `@loader_path` / `@executable_path`。

收集的 dylib 被复制到 `Contents/Frameworks`，修正库之间及 executable 的加载路径。仅修改暂存副本，随后由内向外执行 ad-hoc 签名并验证，再创建包含 `/Applications` 链接的 DMG。

这些是本地/CI 测试产物，没有 Developer ID 签名和 Apple 公证。下载到其他电脑时可能触发 Gatekeeper；正式对外分发需另接签名与公证流程。环境中的 Apple 发布凭据不会被此工具使用。

### Linux

Ubuntu 24.04 的额外工具：

```sh
sudo apt-get install -y dpkg-dev patchelf squashfs-tools libfuse2t64
```

DEB 使用 `dpkg-shlibdeps` 根据实际二进制生成依赖，目标电脑通过包管理器安装 FFmpeg 等系统库。自编译、没有 Debian shlibs/symbols 元数据的库可能无法生成依赖，需要换用发行版开发包或 AppImage。

AppImage 将可分发动态库纳入镜像，glibc、loader 和宿主显卡驱动保留为系统依赖。支持挂载运行、`--appimage-extract-and-run` 和解包后的 `AppRun`。构建环境要能通过动态链接器解析应用依赖；`library-search-paths` 也会用于构建时检查。

### 动态加载和许可证

自动收集覆盖链接导入表及其传递依赖；应用通过 `dlopen` / `LoadLibrary` 动态选择的插件需自行配置资源和平台依赖。工具不自动下载 FFmpeg，也不改变上游 FFmpeg 构建的许可条件。用 `license-files` 附带实际分发库对应的许可证和 notices。

## 安装后的资源、数据和日志

```rust
use tgui::platform::AppPaths;

let paths = AppPaths::new("com.example.my-app")?;
let image_path = paths.resource("assets/logo.png")?;
std::fs::create_dir_all(paths.data_dir())?;
std::fs::create_dir_all(paths.log_dir())?;
```

| 系统 | 资源根目录 | 用户数据 | 日志 |
| --- | --- | --- | --- |
| Windows | executable 所在目录 | `%LOCALAPPDATA%/<id>` | 数据目录下 `logs` |
| macOS | `.app/Contents/Resources` | `~/Library/Application Support/<id>` | `~/Library/Logs/<id>` |
| Linux | executable 同级的 `../lib/<binary>` | `$XDG_DATA_HOME/<id>` | `$XDG_STATE_HOME/<id>/logs` |

Linux 未设置有效绝对 XDG 路径时使用 `~/.local/share` 和 `~/.local/state`。AppImage 的资源在镜像内按相同结构解析。`AppPaths` 不创建目录，也不会向安装目录写入用户数据。

开发运行使用显式的绝对资源目录：

```rust
let paths = AppPaths::new("com.example.my-app")?
    .with_resource_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
```

仅将这个覆盖用于开发入口。安装版使用默认路径，不依赖 `current_dir()` 或编译机源码目录。现有 `init_logging_from_cargo_toml!` 的相对路径语义保持不变；可分发应用应使用 `configure_logging` 将 `LogFileConfig::log_dir` 设为 `paths.log_dir()`。

## 示例与 CI

```sh
# 在仓库根目录运行开发版
cargo run -p packaging -- --resource-dir "$PWD/examples/packaging"

# 无窗口 smoke 测试；media 同时实际解码音频和视频帧
cargo run -p packaging --features media -- \
  --resource-dir "$PWD/examples/packaging" --smoke-test

# 本机安装包
cargo tgui bundle -p packaging --locked
cargo tgui bundle -p packaging --features media --locked \
  --out-dir ../../target/packaging-media
```

仓库的 `desktop-packaging` workflow 在四个原生目标上分别构建 basic / media，生成 0.1.0 和 0.1.1，执行安装、运行、升级、卸载和数据保留检查。Smoke 进程有超时，macOS 挂载在 finally 中卸载；测试只管理本次安装的示例。它使用自动生成的媒体测试信号，无需声卡或窗口。图标、窗口表现与实际声音播放仍应在发布前人工验收。

应用仓库可复制下面的基础 workflow，并按上面的要求增加平台依赖安装。将 `TGUI_REV` 固定为包含此工具的已验证提交：

```yaml
name: bundle
on: workflow_dispatch
permissions:
  contents: read
jobs:
  package:
    strategy:
      matrix:
        os: [windows-2022, macos-15, macos-15-intel, ubuntu-24.04]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@1.89.0
      # 在此安装平台编译工具、Linux 打包工具和应用所需 FFmpeg。
      - name: Install cargo-tgui
        run: cargo install --git https://github.com/nandebishitaoyuan/tgui --rev ${{ vars.TGUI_REV }} cargo-tgui --locked
      - run: cargo tgui doctor
      - run: cargo tgui bundle --locked --out-dir dist
      - uses: actions/upload-artifact@v4
        with:
          name: application-${{ matrix.os }}
          path: dist/
```
