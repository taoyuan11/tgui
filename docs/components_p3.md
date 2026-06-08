# P3 组件

P3 组件已按独立模块接入 `tgui::widgets` 和 `tgui::prelude`，样式类型接入 `ComponentThemes` 和 `StyleSheet`。

## 组件清单

- `Badge`：红点、文本和计数角标，支持语义色和挂载到任意元素。
- `Avatar` / `AvatarGroup`：图片、首字母、姓名回退和堆叠头像组。
- `Skeleton`：矩形、圆形、单行和多行骨架占位。
- `Collapse` / `Accordion`：受控展开状态和互斥展开组。
- `Splitter` / `ResizablePanels`：水平/垂直多 pane 分栏，受控尺寸、min/max、点击微调和双击重置。
- `Breadcrumb`：路径导航、分隔符和中间溢出折叠。
- `Pagination`：上一页/下一页、页码窗口、ellipsis 和 page size 回调。
- `Card`：header/body/footer 槽位、圆角、边框、阴影和整体点击。
- `Rating`：星级展示/录入，支持半星步长和只读模式。
- `Icon`：内置小图标集、命名图标、glyph 和 SVG 字节源。
- `RichText`：Markdown 段落、标题、列表、粗体、行内代码、代码块、链接和图片。
- `Carousel`：受控 slide index、左右按钮、indicator 和 hover-pause autoplay。
- `AutoComplete` / `Combobox`：本地 options 过滤，复用 `Input`、`Popover` 和 `VirtualList`，支持 Esc / 方向键 / Enter 选择。

## 示例

综合示例 `examples/demo` 新增 `P3 Widgets` 页面：

```bash
cargo run --manifest-path examples/demo/Cargo.toml
```
