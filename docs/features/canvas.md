# Canvas

`tgui::canvas` 提供记录式绘制和保留式场景两层能力，适合图表、编辑器、白板、流程图、自定义控件和复杂装饰层。Canvas 仍然是普通组件：它参与布局、命中测试、事件分发、样式和资源缓存。

## 两层模型

| 模型 | 适合场景 | 核心类型 |
| --- | --- | --- |
| 记录式绘制 | 每次视图重建时重新生成图形，例如小图表、装饰、只读图形。 | `CanvasRecorder`、`PathBuilder` |
| 保留式场景 | 长期持有图元树，需要查询、命名、增删改、调试导出。 | `CanvasScene`、`CanvasItem` |

推荐选择：

- 纯展示型绘制优先用 recorder。
- 编辑器、白板、节点图和 inspector 优先持有 `CanvasScene`。
- 需要把命中结果映射到业务对象时，为关键 item 提供稳定 id 或 name。

## 最小示例

```rust
let scene = CanvasRecorder::build(|canvas| {
    canvas
        .set_fill(Color::hexa(0x2563EBFF))
        .fill_round_rect(
            Rect::from_xywh(dp(16.0), dp(16.0), dp(180.0), dp(80.0)),
            dp(8.0),
        )
        .set_fill(Color::WHITE)
        .draw_text(
            Rect::from_xywh(dp(32.0), dp(40.0), dp(150.0), dp(32.0)),
            "Hello Canvas",
        );
});

Canvas::new(scene)
    .size(dp(240.0), dp(128.0))
```

`Canvas::new(...)` 接受 `CanvasScene`、`Value<CanvasScene>` 或 `Signal<CanvasScene>`。如果图形由状态驱动，可以用 `Signal::map` 生成 scene：

```rust
Canvas::new(self.progress.signal().map(|progress| {
    CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(Color::hexa(0xE2E8F0FF))
            .fill_rect(Rect::from_xywh(dp(0.0), dp(0.0), dp(240.0), dp(10.0)))
            .set_fill(Color::hexa(0x16A34AFF))
            .fill_rect(Rect::from_xywh(
                dp(0.0),
                dp(0.0),
                dp(240.0 * progress),
                dp(10.0),
            ));
    })
}))
.height(dp(10.0))
```

## Recorder API

`CanvasRecorder` 的使用方式接近 2D canvas：设置当前绘制状态，然后绘制路径、文本或图片。

常用状态 API：

| API | 说明 |
| --- | --- |
| `save()` / `restore()` | 保存和恢复绘制状态。 |
| `set_fill(...)` / `clear_fill()` | 设置或清除填充 brush。 |
| `set_stroke(CanvasStroke)` / `clear_stroke()` | 设置或清除描边。 |
| `set_shadow(...)` / `clear_shadow()` | 设置或清除阴影。 |
| `set_opacity(value)` | 设置后续 item 不透明度。 |
| `set_blend_mode(mode)` | 设置混合模式。 |
| `translate(...)` / `scale(...)` / `rotate(...)` / `transform(...)` | 设置变换。 |
| `clip()` / `mask()` | 使用当前路径裁剪或遮罩后续内容。 |
| `next_item_id(...)` / `next_item_name(...)` | 给下一个 item 设置稳定 id 或 name。 |

常用绘制 API：

| API | 说明 |
| --- | --- |
| `begin_path()`、`move_to()`、`line_to()`、`quad_to()`、`cubic_to()`、`arc()`、`close_path()` | 构建当前路径。 |
| `fill()`、`stroke()`、`fill_and_stroke()` | 绘制当前路径。 |
| `fill_rect()`、`stroke_rect()`、`fill_round_rect()`、`fill_circle()`、`draw_line()` | 快捷图元。 |
| `draw_text(frame, text)` | 绘制文本。 |
| `draw_rich_text(frame, spans)` | 绘制富文本 span。 |
| `draw_image(frame, source)` | 绘制图片。 |
| `draw_image_with_options(frame, source, options)` | 设置 `ContentFit`、圆角和源裁剪。 |

## PathBuilder

`PathBuilder` 适合复用复杂路径或做布尔运算。

```rust
let badge = PathBuilder::new()
    .rounded_rect(dp(0.0), dp(0.0), dp(120.0), dp(40.0), dp(20.0))
    .circle(dp(100.0), dp(20.0), dp(8.0));

CanvasPath::new(1u64, badge)
    .fill(Color::hexa(0xF97316FF))
    .stroke(CanvasStroke::new(dp(1.0), Color::hexa(0xC2410CFF)))
```

路径能力包括：

- `move_to`、`line_to`、二次/三次贝塞尔、圆弧。
- 矩形、圆角矩形、圆、椭圆。
- `svg_path(...)` 解析 SVG path 字符串。
- `union`、`intersect`、`difference`、`xor` 布尔运算。
- `even_odd()` / `non_zero()` 填充规则。

## Brush、描边和效果

填充可以是纯色、线性渐变或径向渐变：

```rust
let gradient = CanvasLinearGradient::new(
    Point::new(dp(0.0), dp(0.0)),
    Point::new(dp(240.0), dp(0.0)),
    vec![
        CanvasGradientStop::new(0.0, Color::hexa(0x2563EBFF)),
        CanvasGradientStop::new(1.0, Color::hexa(0x22C55EFF)),
    ],
);

canvas
    .set_fill(gradient)
    .fill_rect(Rect::from_xywh(dp(0.0), dp(0.0), dp(240.0), dp(80.0)));
```

描边由 `CanvasStroke` 描述：

```rust
CanvasStroke::new(dp(2.0), Color::hexa(0x334155FF))
    .dash([dp(6.0), dp(4.0)])
    .line_cap(CanvasStrokeCap::Round)
    .line_join(CanvasStrokeJoin::Round)
```

常用效果和合成能力包括 shadow、inner shadow、opacity、blend mode、color filter、clip、mask 和 isolation。大量阴影会占用离屏纹理缓存，必要时调整 `ResourceBudget`。

## Retained scene

`CanvasScene` 可以作为更高阶文档模型的绘制树基础。它适合负责图元树、id/name、基础变更、命中查询和调试导出；业务节点、选择状态、历史记录、约束、吸附和协作协议通常由上层应用维护。

```rust
let mut scene = CanvasScene::empty();
scene.push(
    CanvasPath::new(
        100u64,
        PathBuilder::new().rect(dp(20.0), dp(20.0), dp(160.0), dp(90.0)),
    )
    .name_item("node/background")
    .fill(Color::hexa(0xFFFFFFFF))
    .stroke(CanvasStroke::new(dp(1.0), Color::hexa(0xCBD5E1FF))),
);
scene.push(CanvasText::new(
    101u64,
    Rect::from_xywh(dp(36.0), dp(40.0), dp(128.0), dp(32.0)),
    "Node A",
));
```

常用 `CanvasScene` API：

| API | 说明 |
| --- | --- |
| `empty()` / `from_items(...)` | 创建场景。 |
| `push(...)` / `insert(...)` / `remove(id)` / `clear()` | 修改顶层 items。 |
| `find(id)` / `find_mut(id)` | 按 id 查询。 |
| `find_named(name)` / `find_named_mut(name)` | 按 name 查询。 |
| `visit(...)` | 遍历场景树。 |
| `bounds()` | 计算场景边界。 |
| `query_point(...)` / `query_point_all(...)` | 命中查询。 |
| `debug_info()` / `export_debug_text()` / `export_debug_json()` | 调试输出。 |

推荐结构：

1. ViewModel 持有业务 model。
2. 业务 model 生成或同步到 `CanvasScene`。
3. 命中事件通过 `item_id` 或 `name` 回映到业务对象。
4. 调试面板直接读取 `CanvasScene::debug_info()`。

## Canvas 事件

`Canvas` 同时支持组件级事件和 item 级事件。item 级事件会携带命中的 item id、scene 坐标和文本命中信息。

```rust
Canvas::new(self.scene.signal())
    .size(dp(640.0), dp(420.0))
    .on_item_click(ValueCommand::new(|vm: &mut DiagramVm, event: CanvasMouseEvent| {
        vm.selected_item.set(event.item_id);
    }))
    .on_item_drag(ValueCommand::new(|vm: &mut DiagramVm, event: CanvasDragEvent| {
        vm.drag_item(event.item_id, event.delta);
    }))
    .on_item_wheel(ValueCommand::new(|vm: &mut DiagramVm, event: CanvasWheelEvent| {
        vm.zoom_at(event.position, event.delta);
    }))
```

常用事件 API：

| API | Payload |
| --- | --- |
| `on_item_click` / `on_item_double_click` | `CanvasMouseEvent` |
| `on_item_mouse_down` / `on_item_mouse_up` | `CanvasMouseEvent` |
| `on_item_mouse_enter` / `on_item_mouse_leave` / `on_item_mouse_move` | `CanvasMouseEvent` |
| `on_item_wheel` | `CanvasWheelEvent` |
| `on_item_drag_start` / `on_item_drag` / `on_item_drag_end` | `CanvasDragEvent` |

如果某个 item 只用于视觉背景，设置 `hit_test(false)`，可以减少命中结果噪音。

## 图片和文本

Canvas 图片使用同一套媒体系统：

```rust
CanvasImage::new(
    200u64,
    Rect::from_xywh(dp(0.0), dp(0.0), dp(320.0), dp(180.0)),
    MediaSource::path("assets/cover.svg"),
)
.options(
    CanvasImageOptions::new()
        .fit(ContentFit::Cover)
        .corner_radius(dp(8.0)),
)
```

文本支持普通文本和 `CanvasTextSpan` 富文本。复杂文档排版仍建议用普通 `Text` / `RichText` 组件，Canvas 文本更适合图形内部标签、坐标轴、节点标题等场景。

## 限制

- debug JSON 面向调试，不承诺跨版本稳定。
- retained scene 是绘制树，不是完整编辑器文档模型。
- 当前没有内建选择框、变换手柄、图层树、撤销重做、空间索引或约束/吸附系统。
- 大型编辑器需要上层自己维护业务模型、历史记录和命令系统，再把可视部分同步到 Canvas。
