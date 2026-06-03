# tgui 生产可用化清单

> 基于当前仓库（`v0.1.8`，`winit` beta 系列）的现状梳理。条目按"对生产用户的影响"从高到低排序，可作为 roadmap 抓手。

## 一、依赖与发布稳定性（阻塞 1.0）

- **winit 升级到稳定版**：当前锁在 `winit-core/-win32/-wayland/-x11/-appkit 0.31.0-beta.2`，beta 依赖会让下游 lockfile 不稳；需要等 winit 0.31 stable 或回退到稳定线。
- **`wgpu 29` 升级策略**：明确支持的 wgpu 版本范围与升级节奏（每个 minor 写入 CHANGELOG）。
- **MSRV 声明**：`Cargo.toml` 增加 `rust-version = "..."`，并在 CI 矩阵中固定。
- **语义化版本承诺**：在 README/CHANGELOG 中明确 0.x → 1.0 的 breaking 节奏；公共 API（`src/lib.rs` re-export 列表）冻结前需要一次系统 review。
- **`publish.bat` 用 `--allow-dirty`**：发布脚本默认允许脏工作区，正式版本前应改成强制干净 + tag 校验，避免误发。
- **License 完整性**：当前只有 `LICENSE`（MIT）。考虑 dual-license `MIT OR Apache-2.0`（Rust 生态默认），并补 `LICENSE-APACHE`、`NOTICE`。

## 二、CI / 质量基础设施（阻塞 1.0）

> 状态：骨架已落地（`.github/workflows/ci.yml`、`.github/workflows/release.yml`、`.github/dependabot.yml`、`deny.toml`）。后续仅需仓库 owner 配 secret 启用条件门控的 job、按 CI 输出收敛 clippy / deny exception。

- **GitHub Actions 矩阵**（已实现）：
  - OS：`ubuntu-latest`、`windows-latest`、`macos-latest`
  - Feature 组合：`default`、`audio + video + bench-support`（FFmpeg：Linux 用 apt、macOS 用 brew、Windows 走 BtbN 钉版本 + cache）
- **必跑步骤**（已实现）：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`cargo doc --no-deps --all-features`（`RUSTDOCFLAGS=-D warnings`）、`cargo-deny` licenses/advisories/bans/sources
- **基准回归**（骨架）：`bench-compile` job 用 `cargo check --benches --all-features` 守编译；`bench-publish` job 接 Bencher.dev，需仓库 owner 设 `vars.ENABLE_BENCH=true` + `secrets.BENCHER_API_TOKEN` 后启用
- **覆盖率**（骨架）：`coverage` job 用 `cargo-llvm-cov` + Codecov，需仓库 owner 设 `vars.ENABLE_COVERAGE=true` + `secrets.CODECOV_TOKEN` 后启用；目标仍为 `src/runtime/`、`src/ui/widget/core/` ≥ 80%
- **Release 流程**（已实现）：`release.yml` 在 `v*.*.*` tag push 时跑 `cargo publish --dry-run` → `cargo publish` → 抽 CHANGELOG 段 → `gh release create`，需仓库 owner 设 `secrets.CARGO_REGISTRY_TOKEN`
- **Dependabot**（已实现）：`cargo` + `github-actions` 周更新，patch 版本合并 PR 减少噪音

## 三、稳定性与正确性

> 状态：本节列出的核心项已全部落地。后续仅需在新写代码时遵循同样的约定。

- **`unsafe` 审计**：当前 `unsafe` 集中在 `src/media/raster.rs`（WIC/COM 调用）、`src/notification/platform/windows.rs`、`src/dialog/platform/mod.rs`、`src/video/backend/ffmpeg/helpers.rs`、`src/audio/backend/ffmpeg/session/decode.rs`、`src/log/platform.rs`、`src/runtime/bootstrap.rs`、`src/runtime/theme.rs`、`src/runtime/input/platform_keys.rs`、`src/rendering/renderer/surface.rs`。**已为每处补 `// SAFETY:` 注释**，说明指针/句柄来源、生命周期、线程约束。后续 miri/loom 仍可在非 FFI 部分进一步覆盖。
- **去除 `todo!()` / `unimplemented!()`**：`src/runtime/tests.rs` 与 `src/notification/tests.rs` 中的 `TestVm` 占位已替换成最小可用实现（返回空 `Stack`），并把 `ViewModelContext::for_benchmarks` 同时对 `bench-support` 与 `cfg(test)` 开放，恢复 `cargo test` 全绿。
- **panic 清理**：媒体加载路径里 `http_client()` 的 `expect("http client should build")` 已改为返回 `Result`，并把 `media/loader.rs`、`media/svg.rs` 调用方改成可恢复错误；`media/raster.rs` 中 WIC `cast()` 的 `expect` 也改为映射成 `TguiError::Media`。`runtime/` 中剩余的 `expect` 都是布局/缓存层的不变量保护，按"仅在 setup / 真不变量处 panic"的原则保留。
- **错误类型公开化**：`tgui::core::Error`（`TguiError` 的别名）和 `tgui::core::Result<T, E = TguiError>` 已稳定对外暴露，`prelude` 中可一起导入。`DialogError` / `NotificationError` 在原有位置保持不变。
- **线程安全审查**：`README.md` 新增"线程模型与 Send / Sync"章节，列出 `ViewModel` / `Command` / `ValueCommand` / `root_view` / `Signal` 求值器 / 异步通知 & 对话框回调等关键位置的约束表；`DialogParentHandles` 的 `unsafe impl Send/Sync` 已补 SAFETY 注释，说明只在拥有窗口的线程解引用句柄。

## 四、平台覆盖（生产用户最常踩的坑）

- **macOS 通知后端**：`AGENTS.md` 提到"接口已公开但仍依赖 UserNotifications bridge，调用时可能返回 backend error"。需要走通 `objc2-user-notifications`，含权限请求、分类注册、action 回调。
- **Linux 通知**：当前用 `notify-rust`，需要验证 GNOME / KDE / 老桌面环境（无 dbus 时降级）。
- **Wayland**：HiDPI、fractional scaling、IME（fcitx5/ibus）、剪贴板、CSD 在 GNOME/KDE/wlroots 三类 compositor 上的实地验证。
- **Windows**：DPI awareness manifest、HiDPI、暗色标题栏（DWM）、jump list、taskbar progress 至少给一组示例。
- **WebAssembly**：当前不支持，但很多 GUI crate 把 wasm 作为试金石。明确 roadmap（暂不支持也要写在 README）。

## 五、可访问性（a11y）

仓库内 `accesskit` / a11y 零引用，这是企业向用户的硬指标：

- 集成 `accesskit` + `accesskit_winit`，把 `WidgetTree` 的角色、名称、值、命中区暴露成 a11y 树。
- 键盘焦点链：tab 顺序、focus ring 默认样式、`Esc` 关闭 overlay/dialog、`Enter`/`Space` 触发 button。
- 颜色对比度：`Theme` token 标注 WCAG AA 等级；提供高对比度主题。
- 屏幕阅读器实测：NVDA（Windows）、VoiceOver（macOS）、Orca（Linux）至少 smoke test。
- `prefers-reduced-motion`：动画系统响应系统设置，关闭时禁用 `Signal::animated`。

## 六、文本与国际化

- **IME 健壮性**：Windows IMM / TSF、macOS、fcitx5/ibus 的 composition、候选窗位置、commit/revoke 的边界用例补回归测试。
- **复杂脚本**：阿拉伯/希伯来 RTL、印度系连字、表情序列（ZWJ）的 shaping 校验；目前 `cosmic-text` 已支持，需要 example 和测试。
- **字体回退**：系统字体加载失败时的 fallback 链；嵌入一个 `Noto Sans` 子集做最低保证。
- **i18n 框架**：是否提供 `fluent` 或 `gettext` 的 binding 示例；目前所有示例都是中文/英文硬编码。
- **行编辑细节**：双击/三击选择、Home/End 行内 vs. 段首段尾、`Alt+←/→` 词跳、撤销/重做（`Input` / `Textarea` 都要有）。
- **剪贴板**：富文本（多平台 mime 协商）、图片粘贴、Wayland primary selection。

## 七、文档与示例

- **`docs/` 太薄**：只有 `canvas.md`。需要：
  - `architecture.md`（数据流图，已经有口述版）
  - `state-management.md`（`State`/`Signal`/`TextController`/`AnimatedValue` 区别）
  - `theming.md`（token、Stateful、light/dark/system）
  - `windows.md`（多窗口、frameless、close policy）
  - `media.md`（来源、缓存、错误处理）
  - `notifications.md` / `dialogs.md` / `audio.md` / `video.md`（feature gate + 平台差异表）
  - `migration/`（每次 breaking 写迁移指南）
- **API doc**：所有公开 type 都要有 `///` 文档；CI 加 `RUSTDOCFLAGS="-D warnings"`。
- **`docs.rs` 配置**：在 `Cargo.toml` 加 `[package.metadata.docs.rs] all-features = true`，并处理 FFmpeg 不可用时的降级。
- **官方站点**：mdBook 或类似的 GitHub Pages，统一示例 + API 索引。
- **CHANGELOG.md / CONTRIBUTING.md / CODE_OF_CONDUCT.md / SECURITY.md**：四个标准文件目前都缺。

## 八、性能与资源

> 状态：基础设施已落地。`docs/performance.md` 汇总了 benchmark 基线、`ResourceBudget` 配置、冷启动与空闲帧规则、调优清单。后续在数据点变化时只需更新该文档。

- **基准基线**：`benches/` 现有 9 个 bench；`docs/performance.md` §1 给出代表性中位耗时和 0.x 阶段的 SLA 草案，PR 让任意 bench 退步 > 20% 需要在描述里说明原因。CI 的 `bench-publish` job（见第二章）启用 Bencher.dev 后会自动跟踪回归。
- **GPU 内存预算**：新增公开类型 [`tgui::application::ResourceBudget`]，覆盖 canvas / widget 阴影离屏纹理缓存、image 与 SVG 多分辨率缓存的 LRU 容量上限；`Application::resource_budget(...)` 注入，`ResourceBudget::compact()` 提供内存受限环境的紧凑组合。详见 `docs/performance.md` §2。
- **冷启动**：环境变量 `TGUI_PROFILE_STARTUP=1` 启动后，第一帧 `RenderStatus::Rendered` 会通过 `tgui-startup` tag 输出 `first_frame took ...ms` 日志（`src/log/profiler.rs::log_startup_phase` + `src/runtime/render_cycle.rs`）。桌面默认主题目标 < 200 ms。
- **空闲帧 CPU**：事件循环统一走 `ControlFlow::Wait` / `WaitUntil(deadline)`，无动画 / smooth scroll / caret blink / key repeat 时不会空转；调优入口与排查路径见 `docs/performance.md` §4。
- **大图片 / 大文档**：`MAX_IMAGE_DIMENSION = 2048` 会把过大的图片在解码前缩到 2048 长边；`Textarea` 走 `ropey` + `cosmic-text` viewport 增量 shape；具体边界在 `docs/performance.md` §5。
- **多窗口**：每窗一个 `Renderer`（独立 `Surface` + pipeline 集合），共享 `wgpu::Device` / `Queue`；frame pacing 由各 surface 的 `Queue::submit` 独立驱动。

## 九、安全

- **网络层硬化**：`reqwest` blocking + rustls ring 已经在用；需要补：超时、重定向上限、最大响应体（防 SSRF/DoS）、`MediaSource::Url` 的 scheme 白名单。
- **SVG 解析**：`resvg` 处理恶意 SVG（XXE、嵌入 raster bomb）的边界；`usvg-remote-resolvers` 远程引用要默认关闭或受限。
- **路径遍历**：`MediaSource::Local` 是否对调用方传入的相对路径做 canonicalize。
- **依赖审计**：`cargo audit` / `cargo deny advisories` 接 CI；`rustls 0.23 ring` 的 advisory 跟踪。
- **`SECURITY.md`**：响应窗口、PGP key、CVE 流程。

## 十、生态与可用性

- **更多 widget**：`Tabs`、`TreeView`、`Table`（虚拟滚动）、`Menu` / `ContextMenu`、`Tooltip`、`Toast`、`ProgressBar` 是企业级 GUI 的最小集；目前 `widgets` 模块只有基础控件。
- **拖放**：文件 drop、widget 间 drag-drop 的统一抽象。
- **窗口能力**：always-on-top、tray icon、global hotkey、单实例。
- **系统主题事件**：暗色模式切换时已经有过渡，但需要 example 验证；强调色（accent color）跟随系统。
- **打包**：示例工程加 `cargo-bundle` / `cargo-packager` 配置，生成 `.app` / `.msi` / `.AppImage`。
- **热重载 / DevTools**：开发期 widget tree inspector、动画时间线、性能 overlay；`bench-support` feature 可以扩成 `dev-tools` feature。

## 十一、维护性

- **`src/runtime/` 与 `src/ui/widget/core/` 的 invariant 文档**：CLAUDE.md 已警告"高风险区"，需要把每个子模块的不变量写成 `// invariant:` 注释或 `docs/runtime.md`。
- **公共 API 防回退测试**：`cargo public-api` 接 CI，breaking change 需要显式批准。
- **示例 CI**：`examples/*` 不在 workspace，需要单独脚本批量 `cargo check`，目前只能靠手动。
- **Issue / PR 模板**：`.github/ISSUE_TEMPLATE/`、`PULL_REQUEST_TEMPLATE.md`。
- **trace 体系**：用 `tracing` 替换部分 `log`；提供 feature gate 的 `tracing-subscriber` 集成示例。

## 十二、商业可用的细节

- **崩溃报告**：渲染线程 panic 时的 fallback UI，避免整窗黑屏；可选 `human-panic` 集成。
- **本地化运行时错误**：用户可见的字符串（默认对话框按钮、文件选择器标题）跟随系统语言。
- **打印 / 截屏 / 离屏渲染**：`Renderer` 渲染到 PNG 的 API（已有部分基础设施），便于 CI 视觉回归。
- **视觉回归测试**：`reftest` 风格，主题/widget/canvas 改动落地前自动跑。
- **签名与公证**：macOS notarization、Windows code signing 的示例脚本。

---

## 优先级建议（个人判断）

1. **现在就该做**：CI 矩阵 + clippy/fmt/audit、CHANGELOG/CONTRIBUTING/SECURITY、`unsafe` SAFETY 注释、`todo!()` 清理、API 文档覆盖率、`accesskit` 接入草案。
2. **0.2 之前**：winit 稳定版升级、macOS 通知后端、Linux 通知兼容性、错误类型对外固化、`cargo public-api` 守门。
3. **0.3 ~ 0.5**：a11y 屏幕阅读器实测、复杂脚本/IME 回归、Tabs/Tooltip/Menu/Table 等 widget、视觉回归测试、桌面打包样例。
4. **1.0 之前**：API 冻结、性能基线、安全审计、官方站点、多平台 release 工作流。
