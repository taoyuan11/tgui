# Canvas 文档

本文聚焦 `tgui::canvas` 当前公开能力、查询与调试 API、retained scene 使用建议，以及当前不支持或暂未承诺稳定的部分。

仓库内可配合阅读：

- [README.md](D:/Project/Rust/libs/tgui/README.md)
- [examples/README.md](D:/Project/Rust/libs/tgui/examples/README.md)
- [examples/canvas/src/main.rs](D:/Project/Rust/libs/tgui/examples/canvas/src/main.rs)

## 定位

当前 Canvas 有两层模型：

- `CanvasRecorder`：记录式绘制 API，适合声明式 UI、自定义图形、图表、示意图、卡片装饰层
- `CanvasScene`：保留式场景对象，适合编辑器、白板、流程图、设计器这类需要查询、遍历、命名和调试的场景

推荐方式：

- 纯展示型绘制优先用 `CanvasRecorder::build(...)`
- 需要在 ViewModel 中长期保留并修改场景时，直接持有 `CanvasScene`
- 需要查询 item、按名字定位、导出调试结构时，使用 `CanvasScene` 的 retained API

## 能力矩阵

| 类别 | 当前支持 | 备注 |
| --- | --- | --- |
| Path 绘制 | `move_to` / `line_to` / `quad_to` / `cubic_to` / `arc` / `arc_to` / `svg_path` | 支持 recorder 和 retained item |
| 快捷图元 | 矩形、圆角矩形、圆、椭圆、线段 | 通过 recorder 快捷方法 |
| 填充与描边 | 纯色、线性渐变、径向渐变、描边对齐、虚线、端点/连接样式 | |
| 文本 | 普通文本、富文本 span、换行、对齐、ellipsis | 当前文本 item 查询以内容摘要为主 |
| 图片 | 本地 / URL / bytes、`ContentFit`、圆角、源区域裁剪 | |
| 变换 | translate / scale / rotate / matrix | recorder 状态和 retained item 都支持 |
| 合成 | opacity、blend mode、clip、mask、effect stack、isolation | |
| 命中与交互 | item id、item 级 hover/click/wheel/drag、文本 hit payload | 运行时分发仍基于渲染结果 |
| Scene 查询 | `items()`、`items_mut()`、`find()`、`find_named()`、`visit()`、`remove()`、`query_point()` | retained 查询与主动 hit query |
| 命名 | `CanvasRecorder::next_item_name(...)`、`name_item(...)` | 适合编辑器语义层 |
| 调试导出 | `debug_info()`、`export_debug_text()`、`export_debug_json()` | 面向调试/工具集成 |
| 稳定导出 | `export_json()` | versioned JSON schema，适合工具链消费 |
| retained 编辑 | push / insert / clear / remove / mutate item tree | 基础保留式结构已具备 |

## `CanvasScene` 查询与 retained API

`CanvasScene` 现在不再只是渲染输入，也可以作为场景模型使用。

可用入口：

- `CanvasScene::from_items(...)`
- `scene.items()` / `scene.items_mut()`
- `scene.find(id)` / `scene.find_mut(id)`
- `scene.find_named(name)` / `scene.find_named_mut(name)`
- `scene.contains_id(id)` / `scene.contains_name(name)`
- `scene.visit(...)`
- `scene.remove(id)`
- `scene.bounds()`
- `scene.query_point(point)` / `scene.query_point_all(point)`
- `scene.query_point_with(&CanvasSceneQueryOptions::new(), point)`
- `scene.export_json()`

示例：

```rust
use tgui::canvas::*;
use tgui::core::{dp, Rect};

let mut scene = CanvasScene::from_items(vec![
    CanvasPath::new(1_u64, PathBuilder::new().rect(0.0, 0.0, 120.0, 80.0))
        .name_item("background")
        .fill(Color::WHITE.into())
        .into(),
    CanvasText::new(2_u64, Rect::new(12.0, 12.0, 80.0, 24.0), "Title")
        .name_item("title")
        .into(),
]);

if let Some(item) = scene.find_named("title") {
    println!("title id={}", item.id().get());
}

scene.visit(|entry| {
    println!("{:?} depth={} path={:?}", entry.item.kind(), entry.depth, entry.index_path);
});
```

## 命名与 item 语义

如果场景会被工具链、编辑器、设计器或白板逻辑读取，建议总是为关键 item 提供稳定名字。

两种方式：

- recorder：`next_item_name("selection-outline")`
- retained item：`CanvasPath::new(...).name_item("selection-outline")`

建议命名给这些对象：

- 交互热点
- 选中框 / 控制柄 / 锚点
- 主要节点、边、标签
- 调试时需要定位的层

## 调试与导出

`CanvasScene::debug_info()` 返回结构化调试对象，包含：

- 场景统计信息
- 树形节点结构
- item id / name / kind / depth / 可见性 / hit-test / opacity
- 布局 bounds 与摘要信息

额外导出：

- `export_json()`：稳定的 versioned scene JSON 导出
- `export_debug_text()`：便于日志和人工阅读
- `export_debug_json()`：便于编辑器、测试、外部工具抓取

说明：

- `export_json()` 是公开的 versioned schema，当前格式 id 为 `tgui.canvas.scene`，版本号为 `1`
- `export_debug_text()` / `export_debug_json()` 仍然是调试格式，不保证跨版本稳定
- 如果未来要做更完整的文档模型或协作协议，应在 `export_json()` 的版本化基础上继续演进，而不是依赖 debug JSON

## 主动 hit query

如果你在做编辑器、白板、节点图或 inspector，通常不适合完全依赖 runtime 事件链。这时可以直接对 `CanvasScene` 做主动查询：

- `query_point(point)`：返回当前点命中的最上层 item
- `query_point_all(point)`：返回该点命中的全部 item，顺序从上到下
- `query_point_with(...)` / `query_point_all_with(...)`：显式提供字体与缩放查询上下文，适合编辑器或高频查询

返回值 `CanvasSceneHit` 当前包含：

- `item_id` / `name` / `kind`
- `depth` / `index_path`
- `scene_position` / `local_position`
- `bounds`

说明：

- 这套主动查询会尽量对齐现有 Canvas runtime 命中语义
- 默认 `query_point(...)` / `query_point_all(...)` 是便捷入口；如果你的应用注册了自定义字体、修改了默认字体，或需要和实际 DPI / font scale 严格对齐，应该复用 `CanvasSceneQueryOptions`
- 同一轮 `query_point_all_with(...)` 查询会复用文本命中布局缓存，适合 hover inspector、拖拽、编辑器选区这类更高频的主动 hit query
- 当前更偏向 item 级命中，不等价于完整编辑器选择系统
- 文本命中 payload 已支持返回 `text_hit`，但是否与运行时完全一致仍取决于你是否传入与应用一致的字体与缩放上下文

## retained model 建议

目前已经可以把 `CanvasScene` 当成更高阶 retained scene 的基础层，但它还不是完整设计器数据模型。

适合它负责的内容：

- 绘制树
- 图元 id / name / 分组
- scene 查询与基础变更
- 导出调试树

建议上层应用自己维护的内容：

- 业务语义节点
- 选择状态、历史记录、撤销重做
- 约束系统、吸附系统、图层面板数据
- 正式的存档/协作协议

推荐架构：

1. ViewModel 持有业务 model
2. 业务 model 生成或同步到 `CanvasScene`
3. 命中事件通过 `item_id` / `name` 回映到业务对象
4. 调试或 inspector 直接读取 `CanvasScene::debug_info()`

## 当前限制

当前已经支持很多绘制与交互能力，但仍有一些边界需要明确：

- `export_debug_json()` 当前用于调试，不保证跨版本稳定
- 还没有内建选择框、变换手柄、图层树、撤销重做
- retained scene 目前是绘制树，不是完整编辑器文档模型
- 对 scene 的查询当前包含 `id` / `name` / 遍历 / 主动点查询，但仍不含 CSS-like selector 或空间索引
- 还没有内建空间索引、约束/吸附、选择框、撤销重做等完整编辑器上层能力

## 什么时候用 recorder，什么时候直接持有 scene

优先用 recorder：

- 视图重建时重新生成绘制
- 没有复杂场景编辑需求
- item 只需要稳定 id 用于事件

优先直接持有 `CanvasScene`：

- 需要 inspector、调试树、导出、命名
- 需要编辑器式增删改
- 需要把绘制树和业务对象建立长期映射

## 相关 API

- `Canvas`
- `CanvasRecorder`
- `CanvasScene`
- `CanvasItem`
- `CanvasPath`
- `CanvasText`
- `CanvasImage`
- `CanvasGroup`
- `CanvasSceneVisit`
- `CanvasSceneDebugInfo`
