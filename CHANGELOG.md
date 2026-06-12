# Changelog

本文件记录 `tgui` 的版本发布历史。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)（在 0.x 阶段，破坏性变更可能出现在任意 minor 版本）。

## [Unreleased]

### Added

- 细粒度响应式增量渲染管线，提供一组可独立开关、分级降级的失效/渲染快路径（详见 `docs/advanced/performance.md`）：
  - `fine-grained-splice`（**默认开启**）：叶子/子树 scene-only 改动时把新场景 chunk 原地拼接进根扁平场景与各祖先 chunk 的稳定区间，跳过祖先链向上重合成；命令数量或结构变化即干净回退到原 recompose 路径。
  - `property-deps`（默认关闭）：属性级依赖归因，把 Signal 读取归因到具体视觉属性；未识别属性安全退化为整 widget 失效，不改变当前失效粒度。
  - `incremental-upload`（默认关闭）：逐帧顶点池按字节 diff 只上传变化区间（triple-buffer 下安全）；CPU 逻辑已单测，GPU 视觉验证待真机完成后再决定默认值。
  - `transform-only-scroll`（默认关闭）：纯滚动帧只重收集滚动子树而非整树，与全量重收集逐项等价。
  - 每条快路径都带「与全量重收集逐项等价」+「回退路径」两类单测；任一前置条件不满足均回退，绝不产生错误渲染。

### Changed

- 协议从 `MIT` 切换到 `MIT OR Apache-2.0`，与 Rust 生态默认实践对齐。新增 `LICENSE-APACHE` 与 `NOTICE`，原 `LICENSE` 重命名为 `LICENSE-MIT`。
- 在 `Cargo.toml` 中声明 `rust-version = "1.85"`（MSRV）。
- 在 `Cargo.toml` 中加入 `[package.metadata.docs.rs]`，docs.rs 默认启用全部 feature，便于生成完整 API 文档。
- `Cargo.toml` 的 `exclude` 新增 `docs/*`（独立 vitepress 文档站点）与 `FINE_GRAINED_ROADMAP.md`，避免文档工程与内部路线图进入发布的 crate。
- `publish.bat` 默认要求工作区干净，并按 `Cargo.toml` 中的版本进行 git tag 校验，避免误发；显式 opt-in `PUBLISH_ALLOW_DIRTY=1` 时才允许脏工作区。

## [0.1.8] - 之前发布

公开 API 和能力请见 README.md。详细 commit 历史见 git log。

---

## 版本承诺与升级节奏

### 0.x 阶段

- 仍然处于公开 API 调整窗口期。`src/lib.rs` 中导出的类型、`Application` 链式 API、widget builder 的方法签名都可能在 minor（`0.x.0`）版本中破坏。
- patch 版本（`0.x.y`）只做兼容性修复、文档补充和不会破坏现有调用方的行为微调。
- 每个 minor 版本会在本文件中列出 **Breaking Changes** 段落，并在 `docs/migration/` 中提供迁移说明（如有）。

### 1.0 路线

- 1.0 之前会做一次系统的公开 API review（重点是 `src/lib.rs` 的 re-export、`Command` / `ValueCommand` / `CommandContext` 签名、widget builder 的方法集），冻结后通过 `cargo public-api` 守门。
- 1.0 起严格遵循 SemVer，破坏性变更只在 major（`x.0.0`）版本出现。

### MSRV 策略

- 当前 MSRV：`1.85`。
- MSRV 提升被视为 minor-level 变更（在 0.x 期视为 minor，1.0 之后视为 minor）。提升前会在 CHANGELOG 中显式列出。

### wgpu 升级策略

`tgui` 紧跟 `wgpu` 主线版本，但每次升级都会显式记录：

- 当前依赖：`wgpu = "29"`。
- 一次只跟一个主版本：`wgpu` 出新主版本后，会在下一个 `tgui` minor 中升级。
- 升级 PR 会在本文件 `Changed` 段落列出版本范围迁移点（着色器、Surface 创建、Buffer mapping 等常见兼容性变更）。
- 不在 patch 版本中升级 `wgpu` 主版本。

### winit 升级策略

- 当前依赖：`winit-core/-win32/-wayland/-x11/-appkit = "0.31.0-beta.2"`。
- 在 `winit 0.31` 进入 stable 之前会保持 beta 系列，所有平台后端版本一同升级。stable 化时会作为一次显式 minor 升级，并在迁移说明中列出 lockfile 影响。

[Unreleased]: https://github.com/nandebishitaoyuan/tgui/compare/v0.1.8...HEAD
[0.1.8]: https://github.com/nandebishitaoyuan/tgui/releases/tag/v0.1.8
