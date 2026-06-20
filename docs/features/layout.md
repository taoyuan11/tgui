# 布局系统

`tgui` 使用 `taffy` 计算组件布局，并在公开 API 中提供更贴近 UI 描述的容器和尺寸类型。应用侧通常不需要直接接触 taffy，只需要组合 `Flex`、`Grid`、`Stack`、`ScrollView` 和虚拟化组件。

## 选择容器

| 容器 | 适合场景 | 关键 API |
| --- | --- | --- |
| `Flex` | 工具栏、表单、侧边栏、列表项、按钮组 | `horizontal`、`vertical`、`direction`、`wrap`、`gap`、`align`、`justify` |
| `Grid` | 仪表盘、两列表单、主从区域、固定列 + 弹性列 | `Grid::columns`、`Grid::rows`、`set_columns`、`set_rows` |
| `Stack` | 层叠、居中、背景层、浮动按钮、卡片表面 | `center`、`child`、`padding`、视觉样式 |
| `ScrollView` | 普通可滚动内容 | `overflow_x`、`overflow_y`、`show_scrollbar`、`controller` |
| `VirtualList` | 大量同质行 | `item_layout`、`overscan`、`direction`、`arrangement` |
| `List` / `Tree` / `DataGrid` | 带选择、展开、排序等行为的数据区域 | 组件自带虚拟化和行布局配置 |

## 尺寸类型

| 类型或函数 | 说明 | 示例 |
| --- | --- | --- |
| `dp(...)` / `Dp` | 设备无关像素，用于尺寸、间距、圆角 | `dp(16.0)` |
| `sp(...)` / `Sp` | 字体尺寸 | `sp(14.0)` |
| `Length::AUTO` | 由内容和父布局决定 | `width(Length::AUTO)` |
| `pct(...)` | 百分比长度，按 `0..100` 语义传入 | `width(pct(100.0))` |
| `fr(...)` | Grid 轨道分数单位 | `Grid::columns([fr(1.0), fr(2.0)])` |
| `Insets` | 四边边距或内边距 | `Insets::all(dp(12.0))` |
| `Track` | Grid 行列轨道，支持 `Auto`、`Dp`、百分比和 `fr` | `[dp(240.0).into(), fr(1.0)]` |

`Length` 可由 `Dp`、数字和 `pct(...)` 转换而来。Grid 的 `Track` 可由 `Dp` 和 `fr(...)` 构造。

## Flex

`Flex` 是最常用的容器。默认按主轴排列子节点；子节点可以通过 `grow` / `shrink` 占用或释放空间。

```rust
Flex::vertical()
    .width(dp(420.0))
    .padding(Insets::all(dp(16.0)))
    .gap(dp(12.0))
    .child(Text::new("账户设置"))
    .child(
        Flex::horizontal()
            .gap(dp(8.0))
            .align(Align::Center)
            .child(Text::new("名称").width(dp(80.0)))
            .child(Input::new(self.name.clone()).grow(1.0)),
    )
    .child(
        Flex::horizontal().gap(dp(8.0)).justify(Justify::End).child(el![
            Button::new("取消").secondary(),
            Button::new("保存").primary().on_click(Command::new(AppVm::save)),
        ]),
    )
```

常用模式：

| 需求 | 写法 |
| --- | --- |
| 水平工具栏 | `Flex::horizontal().gap(dp(8.0)).align(Align::Center)` |
| 垂直表单 | `Flex::vertical().gap(dp(12.0)).align(Align::Stretch)` |
| 自动换行 chips | `Flex::horizontal().wrap(Wrap::Wrap).gap(dp(8.0))` |
| 子节点撑满剩余空间 | 子节点 `.grow(1.0)` |
| 右对齐按钮 | 父容器 `.justify(Justify::End)` |

## Grid

`Grid` 适合同时需要行列关系的界面。轨道可以是固定 `Dp`、百分比、自动或 `fr`。

```rust
Grid::columns([dp(220.0).into(), fr(1.0)])
    .set_rows([Track::Auto, fr(1.0)])
    .gap(dp(16.0))
    .padding(Insets::all(dp(16.0)))
    .child(
        Stack::new()
            .column(1)
            .row(1)
            .child(Text::new("侧边栏")),
    )
    .child(
        Stack::new()
            .column(2)
            .row(1)
            .grow(1.0)
            .child(Text::new("主内容")),
    )
```

如果只需要“左侧固定宽度，右侧自适应”的区域，也可以用 `Flex::horizontal()`；当界面需要多行、多列、跨行跨列时再选 `Grid`。

## Stack

`Stack` 会把子元素放在同一个区域中，适合背景层、居中内容和覆盖按钮。

```rust
Stack::new()
    .size(pct(100.0), pct(100.0))
    .center()
    .style(|style, _ctx| {
        style.surface.background = Some(Color::hexa(0x0F172AFF).into());
    })
    .child(
        Card::new()
            .width(dp(360.0))
            .body(Text::new("居中内容")),
    )
```

绝对定位适合小范围覆盖，不建议用来替代整体布局：

```rust
Stack::new()
    .child(Image::from_path("assets/photo.png").size(pct(100.0), pct(100.0)))
    .child(
        Badge::text("NEW")
            .position_absolute()
            .right(dp(12.0))
            .top(dp(12.0)),
    )
```

## ScrollView

普通滚动内容使用 `ScrollView`。它是一个布局容器，可以包裹任意子树。

```rust
ScrollView::new()
    .height(dp(360.0))
    .overflow_y(Overflow::Scroll)
    .show_scrollbar(true)
    .child(
        Flex::vertical()
            .gap(dp(8.0))
            .child(self.rows.get().into_iter().map(|row| Text::new(row)).collect::<Vec<_>>()),
    )
```

需要从 ViewModel 主动滚动时，创建并绑定 `ScrollViewController`：

```rust
struct AppVm {
    scroll: ScrollViewController,
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            scroll: ctx.scroll_view_controller(),
        }
    }
}

ScrollView::new()
    .controller(self.scroll.clone())
    .child(content)
```

## 虚拟化布局

当列表数量很大时，不要把所有行都构建成普通子节点。使用 `VirtualList`、`List`、`Tree` 或 `DataGrid`。

固定行高性能最好：

```rust
VirtualList::new_with_context(self.rows.clone(), |ctx: ListItemContext<String>| {
    Text::new(ctx.item).into()
})
.height(dp(400.0))
.item_layout(ItemLayout::Fixed {
    item_extent: dp(32.0),
    spacing: dp(2.0),
    overscan: 4,
})
```

行高会变化时使用测量布局：

```rust
List::new(self.contacts.get(), contact_row)
    .height(dp(420.0))
    .item_layout(ItemLayout::Measured {
        estimate: dp(64.0),
        spacing: dp(4.0),
        overscan: 3,
    })
```

`overscan` 表示视口外额外保留多少行。值越大，滚动过程中重建更少，但会增加内存和收集成本。

## 常见布局 API

大多数组件和容器都支持这些链式方法：

| API | 说明 |
| --- | --- |
| `size(width, height)` | 同时设置宽高。 |
| `width(...)` / `height(...)` | 设置单个方向尺寸。 |
| `min_width` / `min_height` / `max_width` / `max_height` | 设置尺寸约束。 |
| `margin(...)` | 设置外边距。 |
| `padding(...)` | 容器内边距。 |
| `grow(...)` / `shrink(...)` / `basis(...)` | Flex 子节点尺寸策略。 |
| `align_self(...)` / `justify_self(...)` | 覆盖单个子节点对齐。 |
| `position_absolute()` + `left/top/right/bottom/inset` | 在父容器内绝对定位。 |
| `overflow(...)` / `overflow_x` / `overflow_y` | 裁剪或滚动。 |
| `focusable` / `tab_index` / `focus_scope` | 键盘焦点行为。 |

## 实用建议

- 首选 `Flex` 表达一维关系，首选 `Grid` 表达二维关系。
- 为滚动区域设置明确高度，否则内容可能按无限高度展开。
- 对大型数据使用 `VirtualList`、`List`、`Tree` 或 `DataGrid`，不要手写几千个普通 child。
- 给可选择、可展开、可拖拽的数据行提供稳定 `WidgetKey`。
- 用 `pct(100.0)` 表示填满父级，用 `.grow(1.0)` 表示在 Flex 中吃掉剩余空间。
- 绝对定位适合徽标、浮动按钮和局部装饰，不适合整个页面排版。
