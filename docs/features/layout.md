# 布局系统

`tgui` 使用 `taffy` 计算组件布局，并在公开 API 中提供更贴近 UI 描述的容器和尺寸类型。

## 常用容器

- `Flex`：按主轴排列子元素，适合工具栏、表单、侧边栏和列表项。
- `Grid`：按行列轨道布局，适合仪表盘、表格型面板和复杂区域划分。
- `Stack`：层叠布局，适合覆盖层、背景层和简单组合。
- `ScrollView`：滚动容器。
- `VirtualViewport` / `VirtualList`：用于大量同质数据的虚拟化展示。

## 尺寸与间距

常用基础类型：

- `dp()` / `Dp`：设备无关像素。
- `sp()` / `Sp`：字体尺寸。
- `Length`：自动、固定、百分比等尺寸表达。
- `Insets`：边距和内边距。
- `Align` / `Justify`：交叉轴和主轴对齐。
- `Axis` / `Wrap` / `Overflow`：布局方向、换行和溢出策略。

## 约定

组件 builder 通常提供统一的链式布局 API，包括 `width`、`height`、`min_*`、`max_*`、`margin`、`padding`、`grow`、`shrink`、`basis`、`align_self`、`justify_self`、grid row/column 和绝对定位属性。

布局相关属性大多接受静态值或 `Signal<T>`，因此窗口状态、主题状态和业务状态都可以驱动布局变化。
