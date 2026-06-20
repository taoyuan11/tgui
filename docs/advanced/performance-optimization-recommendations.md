# 性能优化建议

本文面向应用作者和框架维护者，整理 `tgui` 应用中最常见的性能问题和处理顺序。原则是先定位瓶颈，再选择局部优化；不要为了“可能更快”而提前牺牲可读性。

## 应用作者建议

| 场景 | 建议 |
| --- | --- |
| 大量同质行 | 使用 `VirtualList`、`List` 的虚拟布局或 `DataGrid`，并提供稳定 key。 |
| 大量图形对象 | 使用 `CanvasScene` 管理绘制树，把业务模型和绘制 item 分开。 |
| 高频文本更新 | 避免把整页文本拼成一个巨大 `Signal::map`；把局部状态拆到对应组件。 |
| 网络图片多 | 复用 `MediaSource` / `MediaBytes`，给列表使用固定尺寸缩略图。 |
| SVG 图标多 | 统一目标尺寸，减少同一 SVG 的多尺寸栅格化。 |
| 阴影多 | 降低阴影数量或复用样式；必要时调大 shadow cache。 |
| 首屏慢 | 检查字体加载、网络资源、首屏图片、复杂 Canvas scene 和 pipeline 初始化。 |
| 滚动卡顿 | 使用固定行高虚拟列表，减少滚动区域内复杂 overlay、IME 或动态尺寸内容。 |

## 状态与 Signal

`Signal::map` 是惰性求值，但它会记录依赖。高频变化源如果被一个很大的派生 signal 读取，可能扩大 invalidation 半径。

建议：

- 把状态拆到自然的 UI 边界，例如每个表单字段、当前选中项、筛选条件。
- 只在需要展示的位置读取 signal，不要在根视图中提前 resolve 大量派生值。
- 列表行渲染函数中避免读取与该行无关的全局高频状态。
- 动画值只绑定到真正需要动画的属性。

## 布局

布局优化优先从结构开始：

- 大列表使用固定高度或可估算高度，减少每帧测量压力。
- 卡片、按钮、工具栏这类固定格式 UI 设置稳定尺寸或最小尺寸。
- 不要在滚动区域里嵌套过多自动尺寸、复杂 overlay 或动态文本测量。
- 使用 `Grid` 表达二维结构，使用 `Flex` 表达主轴排列，避免用多层 `Stack` 模拟常规布局。

## 渲染和视觉

GPU 渲染很适合大量矩形、文字和图片，但某些视觉效果仍有成本：

- 大面积 blur、复杂 shadow、透明叠层会增加离屏和合成压力。
- 许多不同尺寸的 SVG 会占用栅格缓存。
- 频繁变化的 Canvas scene 应尽量只更新变化的 item 或由业务层控制重建粒度。
- 大图使用与显示尺寸接近的资源，避免把超大原图塞进小缩略图。

## 媒体资源

图片和 SVG 使用 `ResourceBudget` 控制缓存容量：

```rust
Application::new()
    .resource_budget(ResourceBudget::compact())
    .with_view_model(AppVm::new)
    .root_view(AppVm::view)
    .run()
```

如果应用是图片密集型，可以在测试后调大 `image_raster_cache_entries` 或 `svg_raster_cache_entries`。如果应用是视觉效果密集型，可以关注 `canvas_shadow_cache_entries` 和 `widget_shadow_cache_entries`。

## 文本输入

文本输入性能和正确性都依赖 `TextController`、selection、IME 和滚动状态。建议：

- 真实输入框使用 `TextController`，不要把输入拆成普通字符串 signal 手动同步。
- 大文本区域避免每次按键都全量做昂贵解析；可在 ViewModel 中节流或延后处理。
- 涉及 UTF-8、IME、选择区间和横向滚动的修改必须补测试。

## 维护者建议

修改 `crates/tgui-runtime/src/runtime/`、`crates/tgui-runtime/src/ui/widget/core/` 或渲染 primitive 时：

- 先跑相关单元测试，再跑对应 benchmark。
- 快路径优化必须有“结果等价于全量重收集”的测试。
- 快路径前置不满足时必须回退，不能产生部分错误渲染。
- 触及 public API 时同步 facade、README、示例和 docs。

常用验证命令：

```sh
cargo test -p tgui-runtime --lib -- --test-threads=1
cargo check -p tgui
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo bench -p tgui-benchmarks --no-run --features bench-support
```

视频静态链接环境可用时再补：

```sh
cargo check -p tgui --features video-static
```

## 排查顺序

1. 复现：固定窗口大小、数据量、feature、系统电源模式。
2. 分类：判断是首屏、交互、滚动、输入、媒体加载还是动画。
3. 缩小：用示例或最小页面隔离问题组件。
4. 测量：运行相关 benchmark 或增加临时 profile 日志。
5. 修复：优先减少无效重建、无效测量和资源重复加载。
6. 验证：同时验证性能数字、视觉正确性和回退路径。
