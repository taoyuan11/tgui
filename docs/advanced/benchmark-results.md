# 基准测试结果

仓库的 Criterion benchmark 位于 `benches/`。CI 已提供 Bencher.dev 上报入口；仓库 owner
配置 `ENABLE_BENCH=true` 和 `BENCHER_API_TOKEN` 后，`master` 分支 push 会发布趋势。

## 本地运行

```sh
cargo bench -p tgui-benchmarks --no-run --features bench-support
cargo bench -p tgui-benchmarks --features bench-support
```

音视频相关 bench 需要 FFmpeg 环境：

```sh
cargo bench -p tgui-benchmarks --all-features
```

## 当前覆盖

- state/signal 与单属性 patch。
- widget core layout 和真实 widget pipeline。
- scene rendering 与 Canvas retained scene。
- text processing、media source、audio output、video buffering。
- animation 与事件处理。

## 记录规则

发布前应保存 Bencher.dev 趋势链接或本地 Criterion summary。性能变化超过预期时，在 PR
中说明原因、影响场景和是否需要跟进。
