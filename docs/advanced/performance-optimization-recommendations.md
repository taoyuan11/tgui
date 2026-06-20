# 性能优化建议

优先从真实应用场景出发测量，再选择优化路径。`tgui` 的细粒度响应式渲染管线已经包含
多条快路径，改动前应先确认瓶颈是否真的在 runtime、scene patch 或 GPU upload。

## 测量入口

```sh
TGUI_PROFILE_STARTUP=1 cargo run --manifest-path examples/basic_window/Cargo.toml
cargo bench -p tgui-benchmarks --features bench-support
```

建议同时记录窗口尺寸、scale factor、主题、GPU、OS、feature set 和是否启用音视频。

## 应用侧建议

- 复用 `MediaSource::bytes(MediaBytes::from_shared(...))`，避免重复复制大资源。
- 为图片墙、SVG 密集页面和 Canvas shadow 调整 `ResourceBudget`。
- 大数据列表优先使用 `VirtualList`，表格场景优先使用 DataGrid 的受控排序/选择。
- 长文本输入避免在每次 keypress 中重建无关大组件树。
- 动画尽量使用声明式属性过渡或 timeline controller，避免空闲帧手动请求重绘。

## 框架侧变更建议

- runtime、widget core、scene primitive、renderer 改动需要补等价性测试。
- 快路径不满足前置条件时必须回退到子树 patch 或整帧重收集。
- 视觉变更先补稳定 scene/debug snapshot；像素级回归等待离屏渲染 API 完成后接入。
