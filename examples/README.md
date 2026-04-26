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
```

## 推荐先看

如果是第一次接触这个仓库，建议按这个顺序看：

1. `mvvm_counter`
2. `basic_window`
3. `animation_showcase`
4. `multi_window`

## 桌面示例说明

### `basic_window`

最小完整窗口示例。演示 `Application`、窗口大小、主题设置和最基本的组件树结构。
这个示例也使用命名空 ViewModel，保持与库的 MVVM-only 启动路径一致。

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

### `background_effects`

背景效果画廊示例。演示：

- 通用线性渐变背景
- 通用径向渐变背景
- backdrop blur 毛玻璃卡片
- 渐变与 blur 叠加
- 圆角裁剪与层叠玻璃面板

运行方式：

```bash
cargo run --manifest-path examples/background_effects/Cargo.toml
```

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

## 平台示例

### `android_basic_window`

Android 入口示例，展示如何在 Android 环境下启动 `tgui` 应用。  
更详细说明见 [android_basic_window/README.md](D:/Project/Rust/libs/tgui/examples/android_basic_window/README.md)。

### `ohos_basic_window`

OpenHarmony / HarmonyOS 入口示例，展示如何导出并运行 `tgui` 的 OHOS 应用入口。

## 共享资源目录

### `static`

共享静态资源目录，目前包含示例字体资源，供部分示例复用。
