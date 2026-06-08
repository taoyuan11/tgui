# P3 体验组件

P3 组件是一组更偏产品体验和业务界面的组合组件。它们复用现有布局、主题、状态、命令和 overlay 基础设施，适合快速搭建密集型桌面工具界面。

## 组件分组

| 分组 | 组件 |
| --- | --- |
| 标识与状态 | `Badge`、`Avatar`、`AvatarGroup`、`Icon`、`Skeleton` |
| 内容容器 | `Card`、`RichText` |
| 导航 | `Breadcrumb`、`Pagination` |
| 展开与布局 | `Collapse`、`Accordion`、`ResizablePanels` / `Splitter` |
| 输入与浏览 | `Rating`、`Carousel`、`Combobox` / `AutoComplete` |

## Badge / Avatar / Icon / Skeleton

```rust
Flex::horizontal().gap(dp(12.0)).child(el![
    Badge::count(128u32)
        .max(99)
        .attach(Icon::builtin(BuiltinIcon::Info)),
    Badge::text("NEW").tone(BadgeTone::Success),
    AvatarGroup::new(vec![
        Avatar::name("Ada Lovelace"),
        Avatar::name("Mika Chen"),
        Avatar::initials("NP"),
    ])
    .max_visible(2),
    Skeleton::lines(3),
])
```

- `Badge` 支持 dot、文本和计数，计数可以通过 `max(...)` 折叠为 `99+` 这类标签。
- `Badge::attach(...)` 可以把徽标挂到任意组件上。
- `Avatar` 支持图片、姓名推导 initials 和手写 initials。
- `AvatarGroup` 用于头像组，`max_visible(...)` 控制可见数量。
- `Icon` 支持内置 Material 名称、字符 glyph 和 SVG bytes。
- `Skeleton` 支持 line、lines、rect 和 circle 占位形态。

## Card / RichText

```rust
Card::new()
    .header(Text::new("Release candidate"))
    .body(RichText::markdown("**Ready** for desktop QA."))
    .footer(Badge::text("P3").tone(BadgeTone::Primary))
```

`Card` 提供 header、body、footer 和普通 child 插槽。`RichText::markdown(...)` 支持 Markdown 块内容、强调、行内 code、链接、列表、代码块和图片；链接点击通过 `RichTextLinkClick` 回到 ViewModel。

```rust
RichText::markdown("### Markdown sample\n- **Bold** text\n- [Link](https://example.com)")
    .on_link_click(ValueCommand::new(|app: &mut App, link: RichTextLinkClick| {
        app.status.set(format!("open {}", link.href));
    }))
```

## Breadcrumb / Pagination

```rust
Breadcrumb::new(vec![
    BreadcrumbItem::new("Workspace").on_click(Command::new(App::go_workspace)),
    BreadcrumbItem::new("Components"),
    BreadcrumbItem::new("P3 Widgets"),
])

Pagination::new(app.page.signal(), 12usize)
    .page_size(app.page_size.signal())
    .page_size_options(vec![10, 20, 50])
    .on_change(ValueCommand::new(|app: &mut App, change: PaginationChange| {
        app.page.set(change.page);
        app.page_size.set(change.page_size);
    }))
```

`Breadcrumb` 适合层级路径导航，`Pagination` 适合受控分页。分页变化事件同时带回 `page` 和 `page_size`。

## Collapse / Accordion

```rust
Collapse::new("Runtime notes", Text::new("内容区域"))
    .expanded(app.collapse_open.signal())
    .on_change(ValueCommand::new(|app: &mut App, open| {
        app.collapse_open.set(open);
    }))

Accordion::new(items, app.accordion_key.signal())
    .on_change(ValueCommand::new(|app: &mut App, key| {
        app.accordion_key.set(key);
    }))
```

`Collapse` 管理单个区域的展开状态。`Accordion` 管理多个 `AccordionItem`，同一时间只展开一个 key。

## ResizablePanels / Splitter

```rust
ResizablePanels::new(
    vec![Pane::new(left).min(0.2), Pane::new(right)],
    app.splitter_sizes.signal(),
)
.axis(SplitterAxis::Horizontal)
.on_resize(ValueCommand::new(|app: &mut App, resize: SplitterResize| {
    app.splitter_sizes.set(resize.sizes);
}))
```

`ResizablePanels` 也以 `Splitter` 类型别名导出。当前交互模型以受控 sizes 为中心：点击分隔条按 `step(...)` 调整相邻面板，双击恢复均分。

## Rating / Carousel

```rust
Rating::new(app.rating.signal())
    .half()
    .on_change(ValueCommand::new(|app: &mut App, change: RatingChange| {
        app.rating.set(change.value);
    }))

Carousel::new(slides, app.carousel_index.signal())
    .auto_play(Duration::from_secs(4))
    .on_change(ValueCommand::new(|app: &mut App, index| {
        app.carousel_index.set(index);
    }))
```

`Rating` 支持最大星数、半星和自定义步长，也可以设为只读。`Carousel` 使用受控 index，并可配置自动播放间隔。

## Combobox / AutoComplete

```rust
let options = vec![
    ComboboxOption::new("badge", "Badge"),
    ComboboxOption::new("avatar", "Avatar"),
    ComboboxOption::new("rich-text", "RichText"),
];

Combobox::new(app.search_text.clone(), options)
    .open(app.combo_open.signal())
    .selected_key(app.selected_component.signal())
    .placeholder("Search component")
    .allow_custom(false)
    .on_open_change(ValueCommand::new(|app: &mut App, open| {
        app.combo_open.set(open);
    }))
    .on_change(ValueCommand::new(|app: &mut App, change: ComboboxChange| {
        app.selected_component.set(change.selected_key);
    }))
```

`AutoComplete` 是 `Combobox` 的类型别名。默认过滤基于本地 options；需要自定义匹配规则时使用 `filter(...)`。

## 示例位置

综合演示位于 `examples/demo/src/pages/p3.rs`。运行：

```sh
cargo run --manifest-path examples/demo/Cargo.toml
```
