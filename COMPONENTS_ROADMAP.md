# tgui 组件成熟度路线图

> 目标：补齐"通用 GUI 库"应有的组件矩阵，使下游不需要为常用 UI 模式自行造轮子。本文保留**缺失项 + 已补齐项进度**；已有基础组件（Button / Text / Input / Textarea / Image / Slider / Canvas / Checkbox / Radio / Select / Switch / Flex / Grid / Stack / Audio / Video）的增强诉求请走 `PRODUCTION_READINESS.md`。
>
> 排序原则：**基础设施 → 被多个组件复用的容器 → 单点组件**。优先级越靠前，越是"不补就堵后续组件"的瓶颈。
>
> 每个组件统一描述：**作用 / 样式约定 / 桌面操作 / 键盘与触控补充**；基础设施描述：**作用 / 被谁依赖**。

---

## 优先级 P0 —— 基础设施（不补会让上层组件各自造轮子）

### 1. Overlay / Popup Anchoring 引擎
- **作用**：以"锚点 + 偏移 + 翻转策略"在屏幕坐标里定位浮层；监听窗口/滚动尺寸变化自动重定位；管理浮层 z-order 与 backdrop。
- **被依赖组件**：Tooltip、Menu / ContextMenu、Popover、Dropdown、Select（重构）、DatePicker、ColorPicker、Toast、Modal、Combobox、AutoComplete。
- **要点**：放在 `src/runtime/overlay/`，与 widget tree 解耦；提供 `OverlayLayer` API 让 widget 层只描述"我要从 anchor 弹出 X"，由 runtime 决定真实坐标和翻转方向；与 IME caret 矩形共享 caret rect 通道。
- **进度**：[功能完整]
  - ✅ 已迁移到 runtime-owned `src/runtime/overlay/`，widget 层通过兼容 re-export 使用 `Overlay` / `OverlayLayer` / `Anchor` / `PlacementOptions`；
  - ✅ 支持 `Anchor::Rect` / `Point` / `Key` / `Source`，并可从 collect 输出的 overlay anchor 解析真实窗口坐标；
  - ✅ `solve_placement` 支持 Top / Bottom / Left / Right、Start / Center / End、offset / cross_offset、match_anchor_width、viewport padding、FlipSide / ShiftOnly / FlipAndShift / Hide 等策略；
  - ✅ `OverlayLayer::{Tooltip, Popover, Menu, Modal, Toast}` 分层合并，finalize 后保持跨层 z-order 与层内 emit 顺序；
  - ✅ close handler 统一处理 outside click / Esc / return_focus_to，Popover、Menu、Modal、Drawer、Tooltip 均已复用；
  - ✅ overlay texture / SVG icon 管线已纳入统一 bucket，finalize 后进入 `scene.overlay_textures` 与 `overlay_commands`，renderer cache 会保留 overlay 纹理；
  - ✅ 单元测试覆盖 solver、portal collect、clip、hide、close handler、focus scope rebasing、z-order、overlay texture。
  - ℹ️ 复杂浮层的窄窗口 / 桌面降级布局策略仍由具体组件决定，不再阻塞 Overlay 引擎完整性。

### 2. Focus Management（焦点链 + 焦点陷阱）
- **作用**：定义 Tab 顺序（DOM-like 树序 + `tab_index` 覆盖）、可聚焦集合、`Esc`/`Enter`/`Space` 默认行为、模态浮层中的 focus trap、跨浮层焦点回归。
- **被依赖组件**：Modal、Drawer、Menu、Popover、Form、Tabs、DataGrid、Tree、所有需要键盘可用的录入类组件。
- **要点**：扩展 `src/runtime/input/` 现有 focus state；新增 `FocusScope`（可嵌套，模态作用域压栈）；与 a11y（PRODUCTION_READINESS §五）同源实现，避免重复。
- **进度**：[功能完整]
  - ✅ `tab_index` 已支持正数优先级、0/默认树序、负数跳过；
  - ✅ `FocusScopeOptions::{trap, auto_focus_first, active}` 已接入容器、ScrollView、Virtual、Modal、Drawer、Menu / Select overlay，关闭态 scope descendants 不进入焦点候选；
  - ✅ `FocusNavigationSnapshot` 统一服务 Tab 顺序、active trap 过滤、scope 激活 autofocus 与 a11y focus，同一套候选数据不再重复派生；
  - ✅ `auto_focus_first` 仅在 scope 初次激活或 topmost auto-focus scope 切换时触发，且当前焦点已在 scope 内时不抢焦点；
  - ✅ Modal / Drawer 默认 `auto_focus_first(true)`，公开 `.return_focus_to(widget_id)` 与 `.auto_focus_first(bool)` 声明式 builder；
  - ✅ pointer focus escape 会被 active trap 拦截，overlay close 后可按 `return_focus_to` 回焦；
  - ✅ `DefaultActivation` 已让 Button / Checkbox / Switch / Select 等响应 Enter / Space，Esc 走 overlay close；
  - ✅ AccessKit baseline 已默认接入：从 resolved layout + computed scene 生成 a11y tree，focus、trap filtering、Focus/Click/SetValue action 均复用 runtime 焦点和命令路径；
  - ✅ runtime / accessibility 测试覆盖 tab 顺序、focus trap、autofocus、默认激活、Esc / backdrop 关闭、焦点回归、a11y role/value/focus/trap/action。

### 3. Virtual Scrolling 框架
- **作用**：按可见视口仅实例化部分子节点；行高可定（固定 / 估算 / 测量）；支持横向、纵向、网格三种排布。
- **被依赖组件**：List、Table / DataGrid、Tree、长选项的 Select / Combobox、Calendar 月视图（年范围）。
- **要点**：新增 `src/ui/widget/virtual/`；先抽象 `ItemSource<T>` + `ItemLayout`（fixed/estimated/measured）；接入现有 `ScrollRegion`，但只让可见范围进入 widget tree 解析，否则 100k 行会击穿 layout。
- **进度**：[功能完整]
  - ✅ `src/ui/widget/virtual/` 已提供 `ItemSource<T>`、`ItemLayout::{Fixed, Estimated, Measured}`、`VirtualArrangement::{Linear, Grid}`、`VirtualDirection`、`VirtualViewport`、`VirtualList`；
  - ✅ `Vec<T>` / `Arc<[T]>` 默认实现 `ItemSource`，支持稳定 key、render closure 按可见 index 构建子树；
  - ✅ 接入 runtime `VirtualCacheState`，缓存 viewport hint、scroll offset、measured extents、key -> widget id 映射；
  - ✅ `VirtualList` 作为 `VirtualViewport` 的语义薄封装公开导出，默认垂直固定 40dp 行高、overscan 2，并保留 layout / visual / interaction / lifecycle builder；
  - ✅ nested scene / overlay collect 路径会注入 virtual runtime state，Select / Popover / Toast / Drawer 等浮层内的虚拟组件可复用同一滚动状态；
  - ✅ 长选项 `Select` 下拉菜单已迁移到 `VirtualList`，短列表视觉与命中行为保持兼容，长列表只解析 visible range + overscan；
  - ✅ measured extent 回写加入小阈值，避免亚像素测量差异导致连续 layout invalidation；
  - ✅ 单元与 runtime 测试覆盖依赖失效、横向/纵向/网格可见范围、估算/测量高度、总 extent 更新、Select overlay 虚拟滚动与 removed widget state cleanup。
  - ℹ️ 不阻塞框架完整性的长尾：Combobox / AutoComplete 尚未存在；P2 §17 的 List / VirtualList selection、多选、分组、键盘导航、empty/loading 等产品能力另行推进。

### 4. ScrollView（独立可滚动容器）
- **作用**：把目前散在 Input / Textarea 内部的滚动逻辑抽成通用容器；支持 overflow x/y 独立控制、滚动条样式、惯性滚动、键盘 PgUp/PgDn/Home/End。
- **被依赖组件**：List、Modal 内长内容、Drawer、Tabs panel、Accordion 内容区、Form、Table（与 VirtualList 协作）。
- **要点**：现有 `ScrollRegion` 已具备核心数据结构，需要把内部 widget core 的私有路径提升到公开 widget；明确"滚动事件冒泡到父级"的规则。
- **进度**：[基础完成]
  - ✅ `ScrollView` 已公开导出，默认垂直滚动并可配置 `overflow` / `overflow_x` / `overflow_y`、scrollbar 显示与样式；
  - ✅ 支持 `ScrollViewController` 绑定 widget、同步 offset、`scroll_to` 请求与 immediate / smooth 模式；
  - ✅ wheel scroll 已支持嵌套 ScrollView 冒泡：内层不能继续滚动时父级接管；
  - ✅ widget core / runtime 测试覆盖滚动条渲染、clip、controller 绑定与请求、嵌套滚动冒泡。
  - ⏳ 待补：键盘 PgUp / PgDn / Home / End 语义仍需产品化；触控惯性滚动策略仍未单独抽象。

### 5. Portal / Layer Stack
- **作用**：允许 widget 在树中声明、却渲染到顶层（脱离父级 clip / transform）。是浮层、Toast 队列、Modal backdrop 的底层机制。
- **被依赖组件**：Tooltip、Menu、Popover、Modal、Drawer、Toast、Snackbar。
- **要点**：与 §1 配套，但职责不同——§1 算坐标，Portal 管 widget 树重定位 + 渲染顺序。建议在 scene patch 阶段引入 layer 概念，避免回退到全树重建。
- **进度**：[基础完成]
  - ✅ `PortalEntry` 已进入 runtime overlay，collect 阶段先登记 portal，finalize 阶段再求解、平移、合并到 overlay scene；
  - ✅ `OverlayContent` 支持 primitives / hits / batch / nested `ComputedScene`，可承载 Popover 这类任意子树浮层；
  - ✅ portal 内 hit region、focus scope、overlay anchors、nested portal entries 会按 origin 重定位并保持 clip；
  - ✅ scene patch cleanup 已覆盖 Select portal 重定位与关闭后清理，避免 stale overlay handler。
  - ⏳ 待补：Portal 仍是内部基础设施，没有独立 public widget API；跨窗口 portal / 多窗口层栈尚未抽象。

### 6. Gesture 抽象（触控与复杂指针交互）
- **作用**：把当前事件层的"按下/抬起/移动"升级为高阶手势——长按、双击、滑动方向判定、滑动关闭、双指缩放、边缘滑动，并在桌面触控设备和鼠标场景中提供统一语义。
- **被依赖组件**：Drawer（边缘滑动）、Modal（向下滑动关闭）、Tabs（左右滑切换）、List item（左滑显示动作）、Calendar（左右滑切月）、Carousel、ContextMenu（长按触发）。
- **要点**：放在 `src/runtime/input/gesture.rs`；与已有 hover/pressed 状态机互不冲突；提供 `GestureRecognizer` 让 widget 订阅。
- **进度**：[基础完成]
  - ✅ `GestureRecognizer` 已公开导出，可订阅 long press、double tap、swipe、edge swipe、pinch；
  - ✅ 事件类型覆盖 source / phase / position / delta / finger id / scale / edge / direction；
  - ✅ runtime `src/runtime/input/gesture.rs` 已接入 mouse / touch session、axis lock、threshold、cancel/end/update 派发、双指 pinch session；
  - ✅ ContextMenu 已复用 long press；runtime 测试覆盖长按、双击、水平/垂直滑动、边缘滑动、pinch start/update/end/cancel，以及 click 抑制。
  - ⏳ 待补：Drawer 边缘滑动打开、Modal 下滑关闭、Tabs 左右滑切换等组件级产品行为仍需逐个接入；手势参数 token 化尚未完成。

### 7. Form 抽象（值聚合 + 校验 + 错误传播）
- **作用**：统一录入类组件的值绑定、校验规则、错误展示、提交/重置；不强加 schema，仅约定"字段 ↔ State ↔ Validator ↔ 错误信息"四元组。
- **被依赖组件**：Input、Textarea、Select、Checkbox、Radio、Switch、Slider、NumberInput、DatePicker、ColorPicker、Upload。
- **要点**：纯 ViewModel 层抽象，不引入新 widget；可放 `src/foundation/form/`。
- **进度**：[基础完成]
  - ✅ `src/foundation/form/` 已提供 `Form`、`FormField<T>`、`TextFormField`、`ValidationErrors`、`FormSnapshot`；
  - ✅ 支持字段注册、重复字段名检测、字段级 validator、全表 validate / submit / snapshot / reset / clear_errors；
  - ✅ `FormField<T>::bind_change()` 可直接接 Checkbox / Radio / Switch / Slider 等 `on_change`，`TextFormField::controller()` 可接 Input / Textarea；
  - ✅ 表单级 `errors()` / `is_valid()` 与字段级 `errors()` / `first_error()` / `is_valid()` 均为 Signal；
  - ✅ 单元测试覆盖聚合校验、快照取值、reset、clear_errors、文本字段与重复注册。
  - ⏳ 待补：录入组件的错误态视觉约定尚未统一绑定到 Form；异步 validator / submit lifecycle 仍未抽象。

---

## 优先级 P1 —— 高频组件（基础设施齐备后第一波铺开）

### 8. Tooltip
- **作用**：悬停/聚焦时显示简短文本说明。
- **样式**：默认浅色/暗色双 token；圆角小、阴影弱、最大宽度限制后自动换行；带三角形指针指向锚点。
- **桌面操作**：鼠标进入 anchor 延迟（默认 ~500ms）显示、离开立即隐藏；键盘 focus 也触发；`Esc` 隐藏。
- **键盘与触控补充**：键盘 focus 触发；触控长按 anchor 显示，点击其它区域立即隐藏。
- **依赖**：P0 §1、§5。
- **进度**：[基础完成]
  - ✅ `Tooltip` 已公开导出，作为任意 widget 的 `Element::tooltip(...)` 修饰符使用；
  - ✅ 支持纯文本 `Value<String>`、默认 500ms hover delay、自定义 `placement` / `flip_policy` / `delay` / `TooltipStyle`；
  - ✅ collect 阶段通过 Overlay 引擎渲染背景、文本与三角指针，使用 `OverlayLayer::Tooltip`；
  - ✅ runtime 已支持 hover 延迟、离开 fade-out、focus 触发、Esc 隐藏、下一次 wakeup deadline；
  - ✅ runtime 测试覆盖 hover 显示/隐藏、默认 delay、延迟唤醒、focus 显示、Esc 关闭、无 descriptor 不渲染。
  - ⏳ 待补：触控长按 tooltip 行为尚未单独接入；Tooltip 内容仍是纯文本，暂不支持任意 Element 子树。

### 9. Menu / ContextMenu / MenuBar
- **作用**：层级化的操作命令列表；支持图标、快捷键提示、子菜单、分隔线、勾选项、禁用项。
- **样式**：项 padding、悬停背景、选中态、子菜单箭头、快捷键右对齐；遵循平台习惯（macOS 圆角较大，Windows 直角较硬）。
- **桌面操作**：右键 / 主菜单触发；方向键导航、`Enter` 触发、`Esc` 关闭、`→` 进入子菜单、`←` 返回；首字母快速跳转；快捷键全局可触发。
- **键盘与触控补充**：长按触发 ContextMenu；窄窗口可由应用自行组合为按钮 + Drawer；子菜单保持键盘可达。
- **依赖**：P0 §1、§2、§5、§6。
- **进度**：[功能完整]
  - ✅ Menu / ContextMenu / MenuBar 公开 builder API + 主题样式 token（`MenuStyle` / `MenuBarStyle`）；
  - ✅ Menu 下拉浮层 collect 渲染：label / separator / disabled / checked ✓ / 快捷键提示文本（右对齐）/ submenu ▸ 箭头 / `MenuIcon::glyph` 字符图标 / 点击触发 on_select / 外部点击 / Esc 关闭 / focus trap / return_focus_to；
  - ✅ ContextMenu 自动接 `GestureRecognizer::on_long_press`（鼠标右键 + 触屏长按）；
  - ✅ MenuBar 以 `Flex<Button+Menu>` 形式落地，共享 `MenuBarGroupId`；
  - ✅ runtime 键盘导航：Up/Down 在当前层 cycle 跳过 separator/disabled、Enter/Space 触发叶子项 + 关菜单、Esc 关菜单、字母 type-ahead 在当前层匹配跳转；
  - ✅ MenuBar Left/Right 切换：菜单打开时在同 `MenuBarGroupId` 内 cycle active 条目；
  - ✅ submenu 嵌套：collect 阶段父项 hovered（鼠标或键盘 cursor）时递归 emit 子菜单 overlay；键盘 cursor 表示为 `Vec<usize>` 路径，Right 入栈进入 submenu / Left 弹栈退出，与 MenuBar 切换自然衔接；
  - ✅ 全局 `KeyChord` 派发：扫 cached resolved 树里所有 menu / context_menu 含 submenu 递归的 shortcut chord，命中即执行 on_select 并吞键（无需 widget 打开）；`format_chord` 把 chord 渲染成 "Ctrl+N" 风格的 hint 文本；
  - ✅ `MenuIcon::glyph(char)` / `MenuIcon::svg(...)`：在 item label 左侧、checked 列右侧加固定宽度图标列，glyph 走文本渲染，SVG 走 overlay texture 管线；
  - ✅ `menu_tests` + `runtime::tests::menu_tests`：覆盖 descriptor / 渲染 / hover / 键盘 / 全局快捷键 / submenu 嵌套渲染 + 键盘 cursor 进出 / type-ahead / glyph 与 SVG 图标。
  - ⏳ 长尾（独立 PR、不影响功能完整性）：
    - ContextMenu / MenuBar 自动 active 状态接管——需要在 `CollectContext` 加 `internal_context_menu_anchor` / `internal_menubar_active` 字段贯穿调用链，在 gesture 派发里直接写入 runtime state，绕开用户 State 绑定。当前 API 通过 `.on_show(cmd)` / `MenuBar::new(active_signal).on_active_change(cmd)` 与 `State<bool>`/`State<Point>`/`State<Option<usize>>` 绑定，工作良好且可控。


### 10. Modal / Dialog（应用内）
- **作用**：阻塞式对话框；与 `tgui::dialog`（系统原生对话框）区分——这是 app 内绘制的版本，可放任意内容。
- **样式**：居中容器 + 半透明 backdrop；标题区、内容区、动作区三段；进入用 fade + scale，退出反向；最大宽度/高度可配。
- **桌面操作**：`Esc` 关闭（可禁用）；`Enter` 触发主动作；Tab 在内部循环（focus trap）；点击 backdrop 关闭（可禁用）。
- **键盘与触控补充**：触控点击 backdrop 关闭；可选向下滑动关闭（参考 P0 §6）；`Esc` 作为统一关闭入口。
- **依赖**：P0 §1、§2、§5。
- **进度**：[基本完成]
  - ✅ `Modal` / `ModalAction` / `ModalStyle` 公开 builder API + 主题样式 token；
  - ✅ Modal in-tree 子树渲染（任意 widget 内容支持）：semi-transparent backdrop + 居中 card（title / content / actions 三段）；
  - ✅ Card 自动启用 `FocusScopeOptions::{trap(true), auto_focus_first(true)}`：打开后聚焦主按钮 / 首个控件，Tab 在 modal 内循环；
  - ✅ 主按钮（`ModalAction::primary`）`tab_index=1`，正 `tab_index` 优先于默认树序，配合 Button 自带 `DefaultActivation::EnterAndSpace` 可直接 Enter 触发；
  - ✅ Esc 关闭：collect 阶段额外 emit 空内容 sentinel overlay 到 `OverlayLayer::Modal`，piggyback runtime overlay close 机制；可通过 `.close_on_escape(false)` 禁用；
  - ✅ 点击 backdrop 关闭：backdrop Stack 自带 `on_click` → on_open_change(false)；可通过 `.close_on_backdrop_click(false)` 禁用；
  - ✅ `.return_focus_to(widget_id)` 与默认 autofocus 已接入 runtime，关闭后可声明式回焦触发控件；
  - ✅ Fade 动画：backdrop + card 的 `opacity` 由 `open: Signal<bool>` 派生 + `.animated(Transition::ease_in_out(160ms))` 自动过渡；
  - ✅ `WidgetProperty::ModalVisibility` 注册到动画引擎，复用 tooltip 同源 `AnimationKey::Widget` 通道；
  - ✅ 单元测试覆盖（widget core + runtime）：descriptor 挂载、open/close 渲染对比、focus trap、autofocus、Esc / backdrop 关闭、声明式回焦、close_on_escape=false 抑制；
  - ✅ `examples/modal_demo/` 独立示例：alert / confirm / 自定义内容（带 Input）三种用法。
  - ⏳ 待补（独立 PR 价值低、可按需补）：scale 动画（VisualStyle 暂无 scale 字段，需要框架基础设施扩展）、触控"向下滑动关闭"（依赖 P0 §6 Gesture）。


### 11. Popover
- **作用**：相对锚点的非阻塞浮层，用于二级表单、详情、操作组等"比 Tooltip 重、比 Modal 轻"的场景。
- **样式**：带阴影的圆角面板，可选指针；内容由调用方决定。
- **桌面操作**：点击 anchor 触发；点击外部关闭；`Esc` 关闭；可设置 hover 触发模式。
- **键盘与触控补充**：点击 anchor 触发；窄窗口下可降级为底部 sheet 或居中 popover。
- **依赖**：P0 §1、§2、§5。
- **进度**：[基础完成]
  - ✅ `Popover`、`PopoverStyle`、`PopoverTriggerMode` 已公开导出（`prelude` / `widgets`）；
  - ✅ Popover 作为任意 widget 的可选修饰（`Element::popover`）而非独立 `WidgetKind`，内容支持任意子树；
  - ✅ 通过 overlay / portal 机制锚定到 trigger（`Anchor::Key` + `OverlayLayer::Popover`），带阴影圆角面板，支持 `placement` / `flip_policy` / `match_anchor_width`；
  - ✅ 桌面端点击 anchor 触发（toggle 注入 trigger 的 `on_click`）、`close_on_outside_click`、`close_on_escape`（均默认开启）、`return_focus_to` 还原焦点；
  - ✅ `PopoverTriggerMode::{Click, Hover, ClickAndHoverPreview}` 三种触发模式，runtime 解析 active hover popover，cursor 移入浮层 rect 后保持可见；
  - ✅ 浮层内交互元素可正常获得焦点 / 输入 / 点击（覆盖 Input caret、内部点击不关闭）；
  - ✅ `examples/demo` 已新增 Popover 展示卡片，覆盖 click 固定打开 + hover 预览，浮层内含 Input / Switch / Checkbox / Button。
  - ✅ 单元测试覆盖 builder 挂载 descriptor、open 态 overlay 收集、click+hover 包装、外部点击 / `Esc` 关闭、hover 预览可见性、浮层内焦点与 caret。
  - ⏳ 待补（独立 PR）：可选指针 / 箭头渲染（`PopoverStyle::pointer_size` / `pointer_inset` 字段已存在但尚未接入渲染，目前为占位）；窄窗口下的底部 sheet 或居中 popover 降级策略。

### 12. Toast / Snackbar
- **作用**：临时通知（区别于系统级 `tgui::notification`），用于 app 内成功/错误/警告/信息提示。
- **样式**：四种语义色（success / error / warning / info）+ 图标；自动消失（4-6s）；可堆叠成队列；可带"撤销"等操作按钮。
- **桌面操作**：右上 / 右下角；鼠标悬停时暂停计时；点 × 关闭。
- **键盘与触控补充**：点击关闭；可选上滑关闭。
- **依赖**：P0 §1、§5；可选 §6。
- **进度**：[基础完成]
  - ✅ `ToastHost`、`ToastQueue`、`Toast`、`ToastAction`、`ToastKind`、`ToastPlacement`、`ToastStyle` 已公开导出；
  - ✅ 通过 overlay / portal 机制在顶层渲染 app 内 toast 队列，支持 success / error / warning / info 四种语义样式；
  - ✅ 默认自动消失（5s）、`.duration(...)` 自定义时长、`.persistent(true)` 持久提示、关闭按钮、action 按钮；
  - ✅ 桌面端 hover 暂停 / 恢复倒计时，支持点击关闭；
  - ✅ runtime 接入 toast deadline 唤醒，到点后自动触发 scene invalidate 并在下一轮 collect 清理过期项；
  - ✅ `examples/demo` 已新增 Toast / Snackbar 展示卡片，覆盖 4 种语义提示、撤销 action、持久提示和最近操作状态文本。

### 13. ProgressBar / Spinner
- **作用**：表达任务进度。线性进度条 + 环形 spinner，确定（0-1）和不确定两态。
- **样式**：高度 / 直径 / 轨道色 / 进度色 / 圆角；不确定态用循环动画；可选百分比文字。
- **操作**：纯展示。`Signal<f32>` 驱动进度值；`prefers-reduced-motion` 关闭循环动画。
- **依赖**：无新基础设施。
- **进度**：[基础完成]
  - ✅ `ProgressBar`、`Spinner`、`ProgressBarStyle`、`SpinnerStyle` 已公开导出，并加入 `prelude` / `widgets`；
  - ✅ `ProgressBar` 支持确定态数值、非确定态滑动高亮段、可选文本标签与样式覆盖；
  - ✅ `Spinner` 支持尺寸、厚度、轨道显示开关与颜色样式覆盖，复用现有 mesh primitive 提交链路；
  - ✅ 应用级 `reduced_motion` 默认值与 `bind_reduced_motion(...)` 绑定链路已接入 runtime，窗口级 binding 优先于应用默认值；
  - ✅ reduced-motion 开启时，`ProgressBar` 非确定态退化为静态居中高亮段，`Spinner` 退化为静态弧段；
  - ✅ `examples/demo` 已新增 ProgressBar / Spinner 展示卡片，覆盖确定态、不确定态、自定义 spinner 与 reduced-motion 开关演示。

### 14. Tabs / TabView
- **作用**：在一组 panel 之间切换。
- **样式**：标签条（top / bottom / left / right）+ 内容区；当前 tab 高亮 + 下划线/背景；溢出时可滚动或折叠成 "more"。
- **桌面操作**：点击切换；`←/→` 在标签间导航、`Home/End` 跳首尾、`Enter`/`Space` 激活；可拖拽重排（可选）。
- **键盘与触控补充**：点击切换；可选左右滑动 panel 切换（P0 §6）；标签条横滚。
- **依赖**：P0 §2、§4；可选 §6。
- **进度**：[基础完成]
  - ✅ `Tabs` / `TabView` / `TabItem` / `TabPlacement` / `TabsStyle` 已公开导出，并加入 `prelude` / `widgets`；
  - ✅ 支持 top / bottom / left / right 标签条布局，panel 按当前 selected key 动态切换；
  - ✅ tab trigger 复用 Button + ScrollView 组合实现，标签条溢出可滚动，禁用 tab 不进入 tab trigger 命中与焦点导航；
  - ✅ runtime 键盘导航：方向键在同组 tab trigger 中循环移动并跳过禁用项，`Home` / `End` 跳首尾，`Enter` / `Space` 激活；
  - ✅ `examples/demo` 已新增 Tabs / TabView 展示卡片，覆盖受控切换、不同 panel 内容与禁用 tab；
  - ✅ 单元测试覆盖渲染、禁用命中、样式默认值、点击派发、方向键、`Home` / `End`。
  - ⏳ 待补（独立 PR）：more 折叠菜单、拖拽重排、触控左右滑动切换（依赖 P0 §6 Gesture 的产品化手势策略）。

### 15. Drawer / Sidebar
- **作用**：从屏幕边缘滑出的容器，用于导航、过滤、详情。
- **样式**：四个方向之一；遮罩可选；推内容 / 覆盖内容两种模式。
- **桌面操作**：按钮触发；`Esc` 关闭；focus trap；点遮罩关闭。
- **键盘与触控补充**：可选边缘滑动打开（P0 §6）；反向滑动关闭。
- **依赖**：P0 §1、§2、§5、§6。
- **进度**：[基础完成]
  - ✅ `Drawer` / `DrawerPlacement` / `DrawerStyle` 已公开导出，并加入 `prelude` / `widgets`；
  - ✅ 支持 Left / Right / Top / Bottom 四方向，内容区接受任意 widget 子树；
  - ✅ overlay 模式已落地：全屏 Stack + backdrop + panel，backdrop fade、panel slide + fade，打开时启用 focus trap；
  - ✅ 支持 `on_open_change`、`close_on_escape`、`close_on_backdrop_click`、主题样式覆盖；
  - ✅ collect 阶段通过 sentinel overlay 处理 Esc / close handler / backdrop hit，关闭状态不拦截底层 hit，也不注册 focus trap；
  - ✅ `examples/drawer_demo/` 独立示例覆盖四方向 Drawer；widget core / runtime 测试覆盖 hit 穿透、focus trap、backdrop 点击关闭。
  - ⏳ 待补：Push 模式暂未实现；`return_focus_to` builder 尚未公开；触控边缘滑动打开 / 反向滑动关闭尚未接入组件行为。

### 16. Divider
- **作用**：水平/垂直分隔线，带可选标签。
- **样式**：颜色 token 化；粗细、虚线/实线、内边距。
- **操作**：纯展示。
- **依赖**：无。
- **进度**：[功能完整]
  - ✅ `Divider` / `DividerOrientation` / `DividerStyle` 已公开导出，并加入 `prelude` / `widgets`；
  - ✅ 支持 horizontal / vertical、thickness、dashed、color、end_inset、水平标签、style resolver；
  - ✅ 继承通用 layout builder API，可设置尺寸、margin、padding、grid row/column、absolute inset 等；
  - ✅ 渲染测试覆盖默认水平线、显式水平/垂直、虚线、标签；
  - ✅ `examples/demo` 已新增 Divider 展示卡片，覆盖普通、带标签、虚线、内缩、垂直分隔。

---

## 优先级 P2 —— 数据展示与录入扩展

### 17. List / VirtualList
- **作用**：通用列表，支持选中、多选、分组、滑动操作。
- **样式**：item 高度（固定/动态）、分组头、空状态、加载占位（配合 Skeleton）。
- **桌面操作**：方向键导航、`Shift+↑/↓` 范围选、`Ctrl+Click` 多选、`Enter` 触发主动作；右键 ContextMenu。
- **键盘与触控补充**：单击选中；可选左滑显示删除/归档等动作（P0 §6）；长按进入多选模式。
- **依赖**：P0 §2、§3、§4、§6；可选 §9 配 ContextMenu。

### 18. Table / DataGrid
- **作用**：多列数据展示；支持排序、列宽拖拽、列固定、行选择、分组、单元格编辑。
- **样式**：行高、斑马纹、悬停行、表头粘性、列分隔线；紧凑/普通/宽松三档密度。
- **桌面操作**：列头点击排序、Shift 多列排序；列头拖拽改列宽 / 拖换列序；行点击/方向键选择；单元格双击编辑；右键 ContextMenu；横向滚动 + 列固定。
- **键盘与触控补充**：横向滚动；少列时可由应用退化为卡片列表；行长按多选；列固定保留。
- **依赖**：P0 §2、§3、§4、§6；P1 §9。

### 19. Tree
- **作用**：层级数据展示，可展开/折叠、复选、拖拽。
- **样式**：缩进线、展开箭头、复选框（三态：选中/未选/半选）、当前节点高亮。
- **桌面操作**：`←/→` 收/展、`↑/↓` 上下移动、`Enter` 触发主动作、`Space` 切换复选；右键 ContextMenu；拖拽节点改父级。
- **键盘与触控补充**：点击箭头收展、点击节点触发；长按拖拽。
- **依赖**：P0 §2、§3、§4、§6；P1 §9。

### 20. DatePicker / TimePicker / Calendar
- **作用**：日期/时间录入；Calendar 可独立用于日历视图。
- **样式**：触发按钮（Input 风格）+ 弹出 Calendar；月/年切换头；今天高亮；范围选高亮区间；时区/区域设置可配。
- **桌面操作**：键盘录入 + 弹出选；方向键移动焦点日；`PgUp/PgDn` 切月、`Shift+PgUp/PgDn` 切年；`Enter` 选定；`Esc` 关闭。
- **键盘与触控补充**：点选日期；范围选两次点击；可选左右滑切月。
- **依赖**：P0 §1、§2、§5；P1 §11；P0 §6（移动滑切月）；P0 §7（与 Form 联动）。

### 21. NumberInput
- **作用**：数字录入，带步进按钮、最小/最大、精度、单位后缀。
- **样式**：左右或上下两个步进按钮；只读数字字符；错误态边框。
- **桌面操作**：`↑/↓` 步进、`Shift+↑/↓` 大步、`PgUp/PgDn` 跳页；滚轮调值（可禁用）；粘贴自动清洗非数字。
- **键盘与触控补充**：点步进按钮；长按连续步进。
- **依赖**：P0 §7。

### 22. ColorPicker
- **作用**：颜色选择，支持 HSV/HSL/RGB/HEX、透明度、最近用色、预设调色板。
- **样式**：方形 saturation/value 区 + 色相滑条 + alpha 滑条 + 文本输入 + swatches。
- **桌面操作**：拖拽方形/滑条；文本框输入 HEX 或 RGB；方向键微调；`Enter` 提交。
- **键盘与触控补充**：拖拽手势；预设色块大尺寸点击。
- **依赖**：P0 §1、§5、§7。

### 23. Upload
- **作用**：文件选择与拖放上传；支持多文件、类型过滤、进度展示、错误处理。
- **样式**：拖放区（带 hover 高亮）+ 文件列表 + 每项进度 + 删除按钮。
- **桌面操作**：点击触发系统文件选择器（走 `tgui::dialog`）；拖拽文件进入触发；剪贴板粘贴图片可选。
- **键盘与触控补充**：点击调起系统文件选择器；无拖放环境下保留按钮入口。
- **依赖**：P0 §7；P1 §13（进度展示）。

---

## 优先级 P3 —— 视觉与体验完善

### 24. Badge
- **作用**：数字角标 / 红点，挂在 Avatar、按钮、Tab 等上。
- **样式**：圆形红点 / 数字胶囊；最大数限制（"99+"）；语义色。
- **操作**：纯展示。
- **依赖**：无。

### 25. Avatar
- **作用**：用户头像，支持图片、首字母回退、形状、状态点。
- **样式**：圆形/方形/圆角；多种尺寸；右下角可挂 Badge。
- **操作**：可选点击；group 形式可显示堆叠（"+N"）。
- **依赖**：无；可选叠 §24。

### 26. Skeleton
- **作用**：内容加载时的占位骨架。
- **样式**：灰色矩形/圆/线条；闪烁动画（`prefers-reduced-motion` 时关闭）。
- **操作**：纯展示。
- **依赖**：无。

### 27. Collapse / Accordion
- **作用**：可折叠内容区，Accordion 是互斥的 Collapse 组。
- **样式**：标题行 + 箭头 + 内容区；展开/折叠高度动画。
- **桌面操作**：点标题切换；键盘 `Enter`/`Space` 切换；方向键在多个 header 间移动。
- **键盘与触控补充**：点标题切换。
- **依赖**：P0 §4（内容区可能需要滚动）。

### 28. Splitter / Resizable
- **作用**：可拖拽分栏，水平/垂直、双窗格/多窗格。
- **样式**：分隔条 hover/active 高亮；可选最小/最大尺寸；双击重置。
- **桌面操作**：拖拽分隔条；键盘 `←/→` 或 `↑/↓` 微调（focus 在分隔条上）。
- **键盘与触控补充**：拖拽手势；窄窗口可由应用隐藏或折叠成单栏 + Tab。
- **依赖**：P0 §6（移动）。

### 29. Breadcrumb
- **作用**：层级导航路径。
- **样式**：项 + 分隔符（`/` 或 `>`）；溢出时折叠中间为 "…"。
- **桌面操作**：点击项跳转；溢出菜单点击展开（依赖 §9）。
- **键盘与触控补充**：同桌面；横向滚动。
- **依赖**：P1 §9（溢出菜单）。

### 30. Pagination
- **作用**：分页控件。
- **样式**：上一页 / 下一页 / 页码 / 每页条数 / 跳转输入；溢出折叠为 "…"。
- **桌面操作**：点击页码；`←/→` 翻页；输入页码跳转。
- **键盘与触控补充**：窄窗口可精简为上一页/下一页 + 当前页文本，或由应用改用无限滚动。
- **依赖**：P2 §21（跳转输入用 NumberInput）。

### 31. Card
- **作用**：约定式的容器样式（圆角 + 阴影 + padding），降低拼接成本。
- **样式**：默认 / hover / pressed 三态；可选 header / body / footer 槽位。
- **操作**：与 Container 一致；可整体点击触发。
- **依赖**：无（Container 上的样式封装）。

### 32. Rating
- **作用**：星级评分录入/展示。
- **样式**：N 颗星，可半星；hover 预览；只读模式纯展示。
- **桌面操作**：鼠标移动预览、点击确定；`←/→` 调值、`Enter` 确认。
- **键盘与触控补充**：点击或拖动；半星支持需在 item 内细分点击区。
- **依赖**：P0 §7。

### 33. Icon 体系
- **作用**：统一矢量图标管线，避免每个组件单独走 SVG `Image`。
- **样式**：尺寸 token、`currentColor` 语义、可堆叠。
- **操作**：纯展示。
- **依赖**：与现有 `media/svg` 共享栅格化；建议提供常用图标集（或允许外挂图标包）。

### 34. RichText
- **作用**：段落级富文本渲染（粗体 / 斜体 / 链接 / 行内代码 / 行内图片），区别于纯样式串的 `Text`。
- **样式**：继承主题排版；链接可点击；选择可跨段。
- **桌面操作**：选择文本（与 Input 选择基础设施同源）；点击链接；右键 ContextMenu 复制。
- **键盘与触控补充**：长按选择；点击链接。
- **依赖**：P1 §9（右键复制菜单）；与 `src/ui/widget/core` 的文本选择基础设施共用。

### 35. Carousel
- **作用**：横向轮播图 / 卡片组。
- **样式**：indicator 圆点 / 进度条；左右箭头；自动播放可选。
- **桌面操作**：箭头点击 / `←/→`；鼠标悬停暂停自动播放。
- **键盘与触控补充**：左右滑动切换（P0 §6）；点击 indicator 跳页。
- **依赖**：P0 §6。

### 36. AutoComplete / Combobox
- **作用**：带过滤建议的输入框；Combobox 允许自由输入，Select 不允许。
- **样式**：Input + 浮层建议列表；高亮匹配子串；空结果占位。
- **桌面操作**：输入实时过滤；`↑/↓` 在建议中导航、`Enter` 选定、`Esc` 关闭；`Tab` 接受高亮项（可选）。
- **键盘与触控补充**：点击选定；窄窗口下建议限制浮层高度并保持输入框可见。
- **依赖**：P0 §1、§2、§3、§5；P0 §7。

---

## 下一步实施建议

1. **批 1（P0 收尾）**：Overlay / Portal / Focus / Virtual / ScrollView / Gesture / Form 都已进入基础完成状态，其中 Overlay / Focus / Virtual 已功能完整；下一步优先补剩余 public API 长尾与窄窗口策略。
2. **批 2（P1 收尾）**：Tooltip / Menu / Modal / Popover / Toast / ProgressBar / Spinner / Tabs / Drawer / Divider 已铺开，建议集中补 Drawer push 模式、触控手势接入、Popover 指针渲染、Modal scale 动画、Menu SVG 图标。
3. **批 3（P2 开工）**：数据类组件成本最高，但基础设施已可支撑；建议先做 List / VirtualList，把 selection、keyboard nav、empty/loading 状态跑通，再推进 Table / DataGrid 与 Tree。
4. **批 4（P2 录入扩展）**：NumberInput、DatePicker、ColorPicker、Upload 依赖 Form + Overlay + Popover + Gesture，适合在 List 基础交互稳定后并行推进。
5. **批 5（P3）**：视觉完善类，按用户呼声排队；Badge / Avatar / Skeleton / Card / Breadcrumb 可以先作为低风险组件快速补齐。

每批结束都要：
- 更新 `src/lib.rs` re-export 与 `README.md` 组件清单
- 在 `docs/` 增对应章节（Modal / Forms / VirtualList / Table 等单独成文）
- 同步 `PRODUCTION_READINESS.md` 中 a11y / 键盘 / IME 的覆盖矩阵

新组件统一遵循现有惯例：链式 builder、`impl Into<Value<T>>` 接受静态值或 `Signal`、`*Style` token 入 `src/ui/widget/style/`、状态走 `WidgetStateMap`、命中走 `HitGeometry`、事件走 `InteractionHandlers` / `LifecycleEventHandlers`。不要为新组件另起事件 / 布局 / 样式系统。
