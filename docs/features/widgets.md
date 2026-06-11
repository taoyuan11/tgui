# 组件

组件通过 builder 组合成 `Element<VM>`。常见属性覆盖尺寸、布局、视觉样式、交互事件和生命周期事件。

## 基础组件

- 文本与媒体：`Text`、`RichText`、`Image`、`Icon`、`Canvas`、`Carousel`。
- 输入控件：`Input`、`Textarea`、`Checkbox`、`Radio` / `RadioGroup`、`Select`、`Combobox` / `AutoComplete`、`Slider`、`Switch`、`Rating`、`Calendar`、`DatePicker`、`TimePicker`、`NumberInput`、`ColorPicker`、`Upload`。
- 反馈控件：`ProgressBar`、`Spinner`、`Badge`、`Skeleton`、toast / snackbar。
- 结构与导航控件：`Tabs` / `TabView`、`Breadcrumb`、`Pagination`、`Card`、`Collapse` / `Accordion`、`Splitter` / `ResizablePanels`、`List`、`DataGrid` / `Table`、`Tree`。
- 浮层控件：`Tooltip`、`Popover`、`Menu`、`ContextMenu`、`MenuBar`、`Modal`、`Drawer`。
- 交互与 Portal：`GestureRecognizer`、`Portal`、`LayerStack`、`PortalAnchor`、`OverlayPlacement`。
- 头像与标识：`Avatar` / `AvatarGroup`。
- 音视频：`Audio`、`Video`、`VideoSurface`，需要启用对应 feature。

增强表单控件见[表单增强控件](/features/input-controls)，高级交互见[交互与 Portal](/features/interaction-portal)。

## 常见 builder 能力

- 尺寸：`size`、`width`、`height`、`min_*`、`max_*`、`aspect_ratio`。
- 布局：`margin`、`padding`、`grow`、`shrink`、`basis`、grid row/column。
- 视觉：`background`、`background_brush`、`background_image`、`background_blur`、`border`、`border_radius`、`opacity`。
- 交互：`on_click`、`on_double_click`、`on_focus`、`on_blur`、`on_mouse_enter`、`on_mouse_leave`、`on_mouse_move`。
- 生命周期：`on_mount`、`on_unmount`、`on_update`。

## 浮层

运行时提供统一的 overlay anchoring 引擎。`Tooltip`、`Popover`、`Select`、菜单组件、`DatePicker`、`TimePicker`、`ColorPicker`、`Combobox` 和 `Portal` 共享锚点定位、自动翻转、脱离父级裁剪、关闭与回焦能力。

## 数据密集界面

大量同质数据优先考虑 `VirtualList`、`DataGrid`、`Table` 或 `Tree`。如果界面更像编辑器、白板或图表，通常可以把主体绘制放到 `Canvas`，用 ViewModel 管理业务模型。
