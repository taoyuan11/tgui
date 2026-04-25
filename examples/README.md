# Examples

这个目录下的示例用于演示 `tgui` 当前各个功能模块的典型用法。  
大多数示例都是独立的小型 Cargo 工程，可以直接通过 `--manifest-path` 运行。

## 运行方式

桌面示例：

```bash
cargo run --manifest-path examples/basic_window/Cargo.toml
```

也可以把上面的路径替换成其他示例目录，例如：

```bash
cargo run --manifest-path examples/mvvm_counter/Cargo.toml
cargo run --manifest-path examples/layout/Cargo.toml
cargo run --manifest-path examples/widgets_showcase/Cargo.toml
cargo run --manifest-path examples/multi_page_showcase/Cargo.toml
```

## 推荐先看

如果是第一次接触这个仓库，建议按这个顺序看：

1. `basic_window`
2. `mvvm_counter`
3. `layout`
4. `widgets_showcase`
5. `animation_showcase`
6. `multi_window`
7. `multi_page_showcase`

## 桌面示例说明

### `basic_window`

最小完整窗口示例。演示 `Application`、窗口大小、主题设置和最基本的组件树结构。

### `mvvm_counter`

最基础的 MVVM 示例。演示：

- `ViewModelContext`
- `Observable`
- `Binding`
- `Command`
- `bind_title`
- `bind_clear_color`
- `on_input` 快捷键绑定

适合作为状态驱动 UI 的起点。

### `layout`

布局系统总览。集中展示：

- `Grid`
- `Flex`
- `Stack`

适合用来理解不同容器的布局职责与组合方式。

### `layout_theme_showcase`

把布局容器和自定义主题放在同一个界面里做展示。适合参考：

- 仪表盘式页面排布
- 自定义颜色与卡片样式
- 多层容器组合

### `theme`

主题模式切换示例。演示：

- `ThemeMode::Light`
- `ThemeMode::Dark`
- `ThemeMode::System`
- `bind_theme_mode`

也能看到运行时主题切换时的界面响应效果。

### `widgets_showcase`

常用组件综合示例。把这些能力放在一个页面里：

- `Text`
- `Button`
- `Input`
- `Stack`
- `Grid`
- `Flex`
- 鼠标移动事件

适合快速了解常见控件的搭配方式。

### `multi_page_showcase`

多页面、多文件示例。通过一个顶部页签把内容拆到不同源码文件中：

- `src/pages/basic.rs`：基础组件页
- `src/pages/media.rs`：媒体组件页
- `src/pages/canvas.rs`：Canvas 页

适合作为组织中型示例或文档型 demo 的结构参考。

### `input`

输入框与表单状态示例。演示：

- `Input`
- `ValueCommand`
- 多个 `Observable<String>`
- 输入内容实时反映到摘要区域

适合做表单、设置页、资料编辑页参考。

### `scroll`

滚动与溢出处理示例。演示：

- `Overflow::Scroll`
- 纵向滚动
- 横向/双向滚动
- `ScrollbarStyle`
- 滚动条颜色、厚度、圆角、内边距配置

### `canvas`

自定义绘制示例。演示：

- `Canvas`
- `PathBuilder`
- `CanvasPath`
- 线性渐变 / 径向渐变
- 描边、阴影
- 布尔运算路径
- 画布命中与点击事件

适合做图形编辑、可视化、流程图或定制图表表面。

### `image_example`

图片与 SVG 加载示例。展示：

- 网络图片
- SVG 资源
- 绑定驱动的图片切换

适合确认 `Image`、`MediaSource` 相关能力的使用方式。

### `dialogs`

原生对话框示例。演示：

- 同步文件选择
- 异步文件选择
- 同步消息框
- 异步消息框
- `Command::new_with_context`
- `ValueCommand::new`

适合参考运行时服务如何注入到命令处理逻辑中。

### `animation_showcase`

声明式动画示例。核心是 `Binding::animated(...)`，演示属性值变化时自动过渡，例如：

- 宽度
- 内边距
- 圆角
- 背景色
- 偏移
- 透明度
- 窗口清屏色

### `timeline_controller`

时间线动画控制器示例。演示：

- `AnimatedValue`
- `ViewModelContext::timeline()`
- `AnimationSpec`
- `Keyframes`
- `Playback`
- 播放、暂停、恢复、重启、反转、跳转进度

适合需要更强动画编排能力的场景。

### `multi_window`

多窗口示例。演示一个共享 ViewModel 如何同时驱动：

- 主窗口
- 检查器窗口
- 多个文档窗口

核心类型是 `WindowSpec`，适合做文档型应用、浮动工具面板或 inspector 模式界面。

### `video_surface`

视频播放示例。演示：

- `VideoController`
- `VideoSurface`
- `VideoSource`
- 播放 / 暂停 / 静音 / 跳转
- 绑定视频路径并动态加载

这个示例当前默认依赖 `tgui` 的 `video-static` feature。

## 平台示例

### `android_basic_window`

Android 入口示例，展示如何在 Android 环境下启动 `tgui` 应用。  
更详细说明见 [android_basic_window/README.md](D:/Project/Rust/libs/tgui/examples/android_basic_window/README.md)。

### `ohos_basic_window`

OpenHarmony / HarmonyOS 入口示例，展示如何导出并运行 `tgui` 的 OHOS 应用入口。

## 共享资源目录

### `static`

共享静态资源目录，目前包含示例字体资源，供部分示例复用。
