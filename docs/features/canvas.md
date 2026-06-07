# Canvas

`tgui::canvas` 提供记录式绘制和保留式场景两层能力，适合图表、编辑器、白板、流程图、自定义控件和复杂装饰层。

## 两层模型

- `CanvasRecorder`：记录式绘制 API，适合视图重建时重新生成图形。
- `CanvasScene`：保留式场景对象，适合长期持有、查询、命名、调试和增删改。

推荐选择：

- 纯展示型绘制优先用 recorder。
- 编辑器、白板、节点图和 inspector 优先持有 `CanvasScene`。
- 需要把命中结果映射到业务对象时，为关键 item 提供稳定 id 或 name。

## 当前能力

| 类别 | 能力 |
| --- | --- |
| Path | `move_to`、`line_to`、二次/三次贝塞尔、圆弧、SVG path |
| 快捷图元 | 矩形、圆角矩形、圆、椭圆、线段 |
| 填充与描边 | 纯色、线性渐变、径向渐变、虚线、端点和连接样式 |
| 文本 | 普通文本、富文本 span、换行、对齐和 ellipsis |
| 图片 | 本地、URL、bytes、`ContentFit`、圆角和源区域裁剪 |
| 变换 | translate、scale、rotate、matrix |
| 合成 | opacity、blend mode、clip、mask、effect stack、isolation |
| 交互 | item id、hover、click、wheel、drag 和文本命中 payload |
| 查询 | `find`、`find_named`、`visit`、`remove`、`query_point` |
| 导出 | versioned JSON、debug text、debug JSON |

## Retained scene

`CanvasScene` 可以作为更高阶文档模型的绘制树基础。它适合负责图元树、id/name、基础变更、命中查询和调试导出；业务节点、选择状态、历史记录、约束、吸附和协作协议通常由上层应用维护。

推荐结构：

1. ViewModel 持有业务 model。
2. 业务 model 生成或同步到 `CanvasScene`。
3. 命中事件通过 `item_id` 或 `name` 回映到业务对象。
4. 调试面板直接读取 `CanvasScene::debug_info()`。

## 限制

- debug JSON 面向调试，不承诺跨版本稳定。
- retained scene 是绘制树，不是完整编辑器文档模型。
- 当前没有内建选择框、变换手柄、图层树、撤销重做、空间索引或约束/吸附系统。
