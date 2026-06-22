# tgui 生产可用化清单

> 基于当前仓库代码整理：公开 crate `tgui 0.2.0`，MSRV `1.85`，`wgpu 29`，稳定版 `winit 0.30.13`。本文不再把已经落地的能力列为待办，而是记录当前生产基线、剩余风险和 1.0 前需要收敛的事项。

## 状态总览

当前 `tgui` 已经适合原型、内部工具、小型桌面应用、可视化面板和需要强自定义绘制的桌面 GUI。核心链路包括 MVVM 启动、声明式 widget tree、`taffy` 布局、`wgpu` 渲染、主题/动画、文本输入、媒体加载、系统通知、原生对话框、AccessKit a11y、无边框窗口控制、多窗口、Canvas，以及可选音频/视频。

生产化的主要剩余风险集中在：跨平台实机矩阵、API 冻结、安全默认值、发布自动化、文档完整度、可访问性/IME 的真实设备验证，以及音视频/通知这类强平台能力的打包场景验证。

## 一、依赖与发布稳定性

已落地：

- `Cargo.toml` 已声明 `version = "0.2.0"`、workspace `rust-version = "1.85"`、`license = "MIT OR Apache-2.0"`。
- 双许可证文件已存在：`LICENSE-MIT`、`LICENSE-APACHE`，并补了 `NOTICE`。
- `[package.metadata.docs.rs] all-features = true` 已在根 crate 与 `tgui-runtime` 配置。
- 根 crate 继续作为公开 facade，主要实现位于 `crates/tgui-runtime/`，workspace 默认成员为根 crate，示例和 bench 也纳入 workspace。
- 已从 `winit-core/-win32/-wayland/-x11/-appkit 0.31.0-beta.2` 切回稳定版单体 `winit 0.30.13`，并在 runtime/platform 层保留兼容封装，降低下游 lockfile 和平台 API 波动。
- `publish.bat` 已改为默认要求干净工作区，并校验 `v<crate-version>` tag 指向当前 `HEAD`；只有显式设置 `PUBLISH_ALLOW_DIRTY=1` / `PUBLISH_ALLOW_UNTAGGED=1` 才跳过。

1.0 前阻塞：

- **公开 API 冻结**：`src/lib.rs` / `crates/tgui-runtime/src/lib.rs` 的 re-export 面已经很大，1.0 前需要系统 review `Application`、`WindowSpec`、widget builder、theme/style、media、dialog、notification、audio/video 的命名和泛型边界。
- **SemVer 文档化**：README 已说明 0.x / 1.0 节奏，发布检查应把 changelog / migration note 作为必要步骤。
- **winit 后续升级策略**：未来升级到 `winit 0.31` stable 时需要作为一次显式 minor 迁移，重点复测 IME、窗口透明/无边框、a11y adapter、事件转换和平台扩展方法。

## 二、CI 与质量基础设施

已落地：

- `.github/workflows/ci.yml` 包含 `fmt`、`doc`、`clippy`、`test`、`bench-compile`、`cargo-deny`，OS 覆盖 Linux / Windows / macOS。
- feature 矩阵覆盖默认配置和 `audio video bench-support` 组合，CI 中包含 FFmpeg 安装/缓存逻辑。
- `deny.toml` 已启用 licenses / advisories / bans / sources 检查，依赖源默认限制到 crates.io index。
- `.github/dependabot.yml` 已覆盖 cargo 和 GitHub Actions 周更新。
- coverage / Bencher.dev job 已有条件门控骨架，需要仓库 owner 配置变量和 secret。
- `benches/` 当前包含动画、音频、Canvas、事件、媒体、真实 widget pipeline、scene rendering、单属性 patch、state/signal、文本、视频缓冲、widget core layout 等 Criterion targets。

待补齐：

- clippy 目前只显式 deny 部分 lint 组；若要作为强 1.0 门禁，需要评估是否切到更完整的 `-D warnings` 或固定允许列表。
- coverage job 需要启用并设定实际门槛。高风险目录建议优先跟踪 `crates/tgui-runtime/src/runtime/`、`crates/tgui-runtime/src/ui/widget/core/`、文本输入、通知、媒体和渲染 primitive。
- 示例虽然是 workspace member，仍建议在 CI 里显式跑 `cargo check --workspace --all-targets` 或列出关键示例 smoke check，避免只测根 facade。
- 需要补 Issue / PR 模板，降低平台 bug、渲染 bug、IME/a11y bug 的信息缺口。

## 三、稳定性与正确性

已落地：

- `unsafe` 主要集中在平台/FFI/渲染边界，已能看到 `// SAFETY:` 注释覆盖 WIC、Windows Toast、窗口 parent handle、AccessKit macOS adapter、FFmpeg 音频解码等关键路径。
- 公开错误别名已存在：`tgui::core::Error` 和 `tgui::core::Result<T, E = TguiError>`。
- README 已补“线程模型与 Send / Sync”章节，说明 `ViewModel`、`Command`、`ValueCommand`、`root_view`、`Signal`、通知/对话框回调等边界。
- 媒体加载、异步 raster、通知 action、对话框等异步路径会通过 runtime invalidation / dispatcher 回到主线程。
- `State` / `Signal` / `TextController`、scene cache、runtime input、widget core、通知、媒体、动画、音频、视频 helper 均已有较多单元测试覆盖。

待补齐：

- 对仍保留的 `expect` / `panic` 做定期审计，区分测试代码、锁 poisoning、真实不变量和可恢复错误。
- 对 `crates/tgui-runtime/src/runtime/`、`crates/tgui-runtime/src/ui/widget/core/`、`rendering` 的关键 invariant 建议补 `docs/advanced/runtime.md` 细化或代码旁 `// invariant:` 注释。
- FFI 边界仍建议补平台 smoke checklist，尤其 Windows COM、macOS bundle-only API、Linux DBus/Wayland 组合。

## 四、平台覆盖

已落地：

- 桌面目标聚焦 Windows、macOS、Linux；README 明确移动端当前不支持。
- Windows 通知会准备 AppUserModelID 与开始菜单 shortcut；Linux 通知优先 `notify-rust`，失败后尝试 `notify-send`；macOS 普通通知支持 `.app` bundle 原生路径，裸二进制 fallback 到 `osascript`。
- `WindowControl` 已覆盖拖拽、拖拽调整大小、最小化、最大化、还原、关闭和最大化状态查询。
- 透明 / 无边框窗口、custom chrome、多窗口、dialog、clipboard、IME request 等已有平台抽象入口。

待实测：

- **Windows**：DPI awareness、HiDPI、多显示器、透明/无边框窗口、DWM 暗色标题栏、通知 shortcut 清理/升级、MSI/安装后通知身份。
- **macOS**：签名 `.app` bundle 下的 UserNotifications 权限、普通通知、VoiceOver、输入法、窗口透明/无边框、notarization。
- **Linux**：GNOME / KDE / wlroots，X11 / Wayland，fractional scaling，fcitx5/ibus，剪贴板，DBus 不可用时的通知降级。
- **WebAssembly**：当前不支持。README 应明确 wasm 暂不在 0.x 生产目标内，或单独开 roadmap。

## 五、可访问性

已落地：

- 已接入 AccessKit，并按平台使用 `accesskit_windows`、`accesskit_macos`、`accesskit_unix` adapter。
- a11y tree 从 resolved layout / computed scene 构造，覆盖 window/root、container、scroll view、text、image/icon、canvas、button、checkbox、radio、switch、select、slider、progress/spinner、text input/textarea、toast、modal/drawer、DataGrid/Table、Tree、Tabs、Splitter、audio/video 等基础 role。
- 焦点与 runtime focus 同源，AccessKit action 通过 channel 回到 runtime。
- reduced motion 已有应用级配置和窗口绑定：`Application::reduced_motion`、`bind_reduced_motion`、`WindowSpec::bind_reduced_motion`，并参与 style/animation collect。

待补齐：

- NVDA（Windows）、VoiceOver（macOS）、Orca（Linux）至少做 smoke test，并记录版本、桌面环境和已知问题。
- DataGrid / Tree / Menu / Tabs 等高级组件需要继续增强行列、层级、选区、快捷键、描述关系等语义。
- `Theme` token 需要标注 WCAG AA/AAA 对比度目标，并提供高对比度主题。
- reduced motion 当前由应用配置/绑定驱动，还没有看到自动读取系统 `prefers-reduced-motion` 的平台桥接。

## 六、文本、输入与国际化

已落地：

- `Input` / `Textarea` 共享 `TextController`、`TextChangeSet`、selection/caret、scroll、IME composition、clipboard copy/paste/cut、key repeat 等基础设施。
- IME event path 已处理 enable/disable、preedit、commit、composition caret、surrounding text request，并有 UTF-8 / emoji / composition / scroll 可见性相关测试。
- 文本布局基于 `cosmic-text`，`Textarea` 使用 `ropey`，并已有 caret/selection/scroll/viewport 增量测试。
- `Command`、`on_input`、焦点链、overlay close、Tab navigation、Home/End 等 runtime 输入路径已有测试。

待补齐：

- Windows IMM/TSF、macOS 输入法、fcitx5/ibus 的真实设备回归，尤其候选窗位置、commit/revoke、删除周边文本能力和长文本滚动；稳定版 `winit 0.30` 未暴露之前 beta API 中使用过的 `DeleteSurrounding` 事件，需要单独评估替代路径或等待上游稳定支持。
- RTL/双向文本、阿拉伯/希伯来、印度系连字、ZWJ 表情序列的 example 与回归测试。
- 字体 fallback 策略需要文档化；系统字体加载失败时是否需要嵌入最小字体子集仍需决策。
- 行编辑高级行为仍需审计：双击/三击选择、按词移动/选择、撤销/重做、富文本剪贴板、图片粘贴、Wayland primary selection。
- i18n 示例仍不足，可补 fluent/gettext 或应用侧集成指南。

## 七、文档与示例

已落地：

- `docs/` 已是 VitePress 文档站，而不再只有单个 canvas 文档。
- 当前文档包含 quick start、application、environment、MVVM、layout、theme、widgets、input controls、interaction/portal、window chrome、media、dialogs/notifications、canvas、performance、runtime、examples、migration 等页面。
- README 已覆盖项目状态、版本承诺、MSRV、feature、性能管线、workspace 结构、公开 API、线程模型、贡献注意事项和 license。
- 示例工程位于 workspace `examples/*`，覆盖 animation、background effects、basic window、canvas、demo、dialogs、drawer、frameless、virtual list、modal、multi window、MVVM counter、DataGrid/Table、textarea、timeline、toast、tree 等。

待补齐：

- 所有公开 type / 方法的 `///` 文档覆盖率需要系统检查。`cargo doc -D warnings` 能防坏链接，但不能自动保证文档完整。
- 需要补正式 `CONTRIBUTING.md`、`CODE_OF_CONDUCT.md`、`SECURITY.md`。
- 需要发布流程文档：版本 bump、changelog、migration、tag、package list、docs 发布、crate publish、GitHub release。
- 音频/视频文档应明确 FFmpeg 安装、动态/静态链接、平台差异和故障排查。

## 八、性能与资源

已落地：

- `docs/advanced/performance.md` 汇总了 benchmark、`ResourceBudget`、冷启动 profiling、调优方向和细粒度响应式渲染管线。
- `ResourceBudget` 已公开，覆盖 Canvas shadow、widget shadow、image raster、SVG raster 缓存容量，并提供 `ResourceBudget::compact()`。
- 细粒度响应式渲染快路径已默认内置：scene command splice、属性级依赖归因、GPU 顶点脏区间增量上传、纯滚动 CPU 子树重收集、纯滚动 GPU 平移。
- `TGUI_PROFILE_STARTUP=1` 可输出首帧阶段 profiling。
- 事件循环在无动画/滚动/caret/key repeat 时走等待路径，避免空闲帧忙等。

待补齐：

- 建立稳定 benchmark 基线和阈值，把 Bencher.dev job 启用到主分支。
- 补真实应用级场景：大表格、长文本、复杂 overlay、图片墙、Canvas retained scene、多窗口、多 DPI。
- GPU 内存和纹理缓存需要实机上限测试，并记录推荐 `ResourceBudget` 配置。
- 视觉回归测试仍缺，主题/widget/canvas/渲染管线改动建议接入 reftest 或截图 diff。

## 九、安全

已落地：

- 网络媒体使用 `reqwest` blocking + rustls/ring。
- SVG 外部引用解析已限制远程 scheme 为 `http` / `https`；嵌入 bytes 的 SVG 默认拒绝本地相对路径；本地 SVG 只在文件来源下允许相对本地资源。
- SVG data URL / nested image 解析失败会记录错误并使加载失败，避免静默吞掉外部资源问题。
- Windows Toast XML 和 macOS AppleScript fallback 都有转义逻辑。
- `cargo-deny` 已进入 CI，licenses/advisories/sources 有基础门禁。

待补齐：

- `MediaSource::Url` 顶层加载仍需明确 scheme 白名单、超时、重定向上限、最大响应体大小和可选 host allowlist，避免 SSRF/DoS 风险。
- 远程 SVG 引用同样需要响应体大小、超时和重定向限制。
- `MediaSource::Path` 是否 canonicalize、是否限制 sandbox 根目录应交给应用还是框架，需要文档化。
- `deny.toml` 当前忽略了一个 advisory，需记录原因、影响范围和复查日期。
- 补 `SECURITY.md`：报告渠道、响应窗口、支持版本、CVE 流程。

## 十、生态与可用性

已落地：

- widget 覆盖已经超过基础控件，公开导出包括 Tabs/TabView、Tree、Table/DataGrid、Menu/ContextMenu/MenuBar、Tooltip、Popover、ToastHost、ProgressBar、Spinner、List/VirtualList、Calendar、DatePicker、TimePicker、ColorPicker、Upload、Pagination、Breadcrumb、Accordion、Combobox/AutoComplete、RichText、Carousel、Rating、Splitter/ResizablePanels、Badge、Avatar、Skeleton、Card、Icon 等。
- overlay/portal 基础设施、焦点 trap、回焦、菜单/Select/Popover/Tooltip 定位已经成为共享 runtime 能力。
- 文件 drop、gesture、drag/edge swipe/pinch/long press 等交互类型已公开。

待补齐：

- 系统级能力仍少：tray icon、global hotkey、single instance、always-on-top、taskbar progress、jump list、文件关联。
- 打包示例仍缺：`.app`、`.msi`、`.AppImage` / Flatpak / deb/rpm，以及音视频依赖打包。
- DevTools / inspector / 性能 overlay / animation timeline 仍可作为独立 `dev-tools` feature 规划。
- 示例需要更贴近真实业务：设置页、数据表 CRUD、媒体库、日志查看器、后台任务进度、通知/托盘组合。

## 十一、维护性

已落地：

- `AGENTS.md` / `CLAUDE.md` 已标注高风险区域和推荐阅读顺序。
- 运行时、widget core、渲染 primitive、文本输入、通知、媒体、音视频已有较多模块化测试。
- 内部 crate 边界已经拆出 `tgui-core`、`tgui-platform`、`tgui-log`、`tgui-mvvm`、`tgui-media`、`tgui-ui`、`tgui-rendering`，根 crate 保持 facade。

待补齐：

- 把细粒度响应式渲染管线的重要 invariant 从 `FINE_GRAINED_ROADMAP.md` 同步到更稳定的开发文档或代码注释。
- 公共 API、widget 行为、theme token、layout 行为需要更清晰的变更分类：breaking / compatible / visual-compatible。
- `tracing` 集成可作为 opt-in 示例，不一定替换现有日志，但要明确 runtime phase、scene patch、GPU upload、media load 的 observability 入口。

## 十二、商业可用细节

待补齐：

- 崩溃报告与 panic hook：渲染/runtime panic 时的日志、用户提示和可恢复策略。
- 本地化运行时字符串：默认对话框按钮、错误提示、文件选择标题、通知文案。
- 离屏渲染 / 截图 API：用于 CI 视觉回归和 bug report。
- macOS signing/notarization、Windows code signing、Linux desktop file/icon/notification identity 的脚本和文档。
- 长期支持策略：支持哪些 Rust 版本、桌面系统版本、GPU 后端、FFmpeg 版本。

## 优先级建议

现在就该做：

1. 做 0.2 API 面 review。
2. 补 `CONTRIBUTING.md` / `SECURITY.md` / Issue & PR 模板。
3. 为 `MediaSource::Url` 和远程 SVG 资源补网络安全默认值。
4. 跑一轮 Windows / macOS / Linux 的通知、IME、a11y、透明窗口、HiDPI smoke test 并记录结果。
5. 记录稳定版 `winit 0.30.13` 兼容封装与未来 `0.31` stable 升级的迁移注意事项。

0.3 前：

1. 启用 coverage / Bencher.dev 主分支趋势。
2. 补音频/视频打包和 FFmpeg 故障排查文档。
3. 补 RTL/复杂脚本/高级文本编辑测试。
4. 补真实业务示例和视觉回归基础设施。

1.0 前：

1. API 冻结、SemVer / migration / changelog 流程稳定，并完成 `winit 0.30` 生产实机验证。
2. a11y 屏幕阅读器实测、IME 实机矩阵、平台通知/对话框/窗口能力验证完成。
3. 安全响应流程、依赖审计策略、发布签名/公证/打包文档完成。
4. 性能基线、资源预算指南和高风险渲染快路径回归测试长期稳定。
