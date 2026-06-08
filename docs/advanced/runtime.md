# 运行时与渲染

`tgui` 的运行时把 ViewModel 状态、组件树解析、布局、命中测试、输入、命令和 wgpu 渲染串成一条桌面 UI 管线。

## 主流程

1. ViewModel 构建 `Element<VM>` 组件树。
2. `WidgetTree` 解析组件树并用 `taffy` 计算布局。
3. 组件树生成 scene primitives、命中区域、滚动区域、IME 和 caret 信息。
4. `runtime` 处理窗口事件、输入状态、hover/focus/pressed、命令派发和缓存失效。
5. `Renderer` 把 scene primitives 提交到 `wgpu` pipeline。

## 渲染能力

当前渲染器覆盖：

- 矩形、圆角矩形、边框和阴影。
- 线性/径向渐变和 brush。
- mesh 绘制。
- `cosmic-text` 文本渲染。
- 图片和纹理绘制。
- Canvas primitive。
- 透明窗口 surface 和 backdrop blur。

## 输入与文本

文本输入相关逻辑集中在 `TextController`、共享 widget 基础设施和 runtime input 模块。修改输入、选择、IME、滚动或 caret 行为时，需要同时考虑 UTF-8 边界、选区、横向滚动和 invalidation。

## Overlay、Portal 与手势

浮层定位由 runtime overlay 引擎统一处理，`Tooltip`、`Popover`、菜单、picker、combobox 和 `Portal` 都会经过同一套 placement / flip / close / focus 路径。`Portal` 可以把内容投递到当前窗口或其它运行时管理窗口的 overlay layer。

手势识别通过 `GestureRecognizer` 附着到 `Element`，运行时会把长按、双击/双触、滑动、边缘滑动和双指缩放转成 `ValueCommand` 事件。详见[交互与 Portal](/features/interaction-portal)。

## 缓存与失效

状态写入、媒体加载完成、主题变化、布局相关属性变化和动画推进都会触发不同粒度的失效。共享核心模块会尽量复用 layout、scene 和资源缓存，避免不必要的整树重建。
