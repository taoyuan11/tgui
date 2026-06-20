# 发布流程

本文记录 `tgui` 0.x 发布时的最小检查清单。发布前应保持工作区干净，除非明确记录
了允许 dirty 发布的原因。

## 版本准备

1. 确认 `Cargo.toml` 和 workspace package version 一致。
2. 更新 README、docs、examples 和 migration note。
3. 检查 `PRODUCTION_READINESS.md` 是否需要同步状态。
4. 为 public API 变化更新 `public-api/*.txt`，并在 PR 中说明兼容性分类。

## 本地门禁

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo doc --no-deps --all-features
cargo test -p tgui-runtime --lib -- --test-threads=1
```

Feature 检查：

```sh
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check -p tgui --features video-static
```

如果本机缺少 FFmpeg 或 libclang，记录环境限制，并确认 CI 的全 feature job 通过。

## Package 检查

```sh
cargo package -p tgui --allow-dirty --list
cargo package -p tgui-runtime --allow-dirty --list
```

确认 crate 包含 README、license、NOTICE 和源码，不包含示例 target、docs 站点构建产物
或本地临时文件。

## Tag 与发布

1. 合并 release PR。
2. 打 `v<crate-version>` tag，确认 tag 指向发布 commit。
3. 等 CI 通过。
4. 运行发布脚本或手动 `cargo publish -p ...`，按依赖顺序发布内部 crate 与 facade。
5. 创建 GitHub release，附 changelog、迁移说明、已知平台限制和验证矩阵。

`publish.bat` 默认要求干净工作区和匹配 tag。只有在明确记录原因时才设置
`PUBLISH_ALLOW_DIRTY=1` 或 `PUBLISH_ALLOW_UNTAGGED=1`。
