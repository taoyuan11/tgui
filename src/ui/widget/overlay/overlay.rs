//! `Overlay` 浮层声明 builder。
//!
//! 消费者（如 Tooltip / Menu / Popover）在自己的 collect 函数末尾构造 `Overlay`，
//! 通过 `super::collect::emit_overlay` 一次性完成"求解 → push 渲染原语 → 注册命中 → 注册关闭"。
//!
//! 本轮内容模型限于 `OverlayContent::Primitives`（已是窗口尺寸级的渲染原语）。
//! 未来扩 `OverlayContent::Element(Element<VM>)` 时只增加枚举变体，不破坏现有 API。

use crate::foundation::view_model::Command;
use crate::ui::widget::common::{RenderPrimitive, TextPrimitive, WidgetId};

use super::anchor::Anchor;
use super::placement::{
    FlipPolicy, OverlayId, OverlayLayer, Placement, PlacementOptions,
};

/// 浮层声明。包含锚点、求解选项、关闭回调与归属层。
///
/// `<VM>` 是 ViewModel 类型；`on_close` 等命令运行在该类型上。
pub(crate) struct Overlay<VM> {
    pub(crate) id: OverlayId,
    pub(crate) anchor: Anchor,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) on_close: Option<Command<VM>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
}

impl<VM> Overlay<VM> {
    pub(crate) fn new(id: OverlayId, anchor: impl Into<Anchor>) -> Self {
        Self {
            id,
            anchor: anchor.into(),
            options: PlacementOptions::default(),
            layer: OverlayLayer::default(),
            on_close: None,
            return_focus_to: None,
            close_on_outside_click: false,
            close_on_escape: false,
        }
    }

    pub(crate) fn placement(mut self, placement: Placement) -> Self {
        self.options.placement = placement;
        self
    }

    pub(crate) fn offset(mut self, offset: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.offset = offset.into();
        self
    }

    pub(crate) fn cross_offset(mut self, cross_offset: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.cross_offset = cross_offset.into();
        self
    }

    pub(crate) fn flip_policy(mut self, flip: FlipPolicy) -> Self {
        self.options.flip = flip;
        self
    }

    pub(crate) fn viewport_padding(mut self, padding: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.viewport_padding = padding.into();
        self
    }

    pub(crate) fn match_anchor_width(mut self, on: bool) -> Self {
        self.options.match_anchor_width = on;
        self
    }

    pub(crate) fn layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }

    pub(crate) fn on_close(mut self, command: Command<VM>) -> Self {
        self.on_close = Some(command);
        self
    }

    pub(crate) fn return_focus_to(mut self, widget_id: WidgetId) -> Self {
        self.return_focus_to = Some(widget_id);
        self
    }

    pub(crate) fn close_on_outside_click(mut self, on: bool) -> Self {
        self.close_on_outside_click = on;
        self
    }

    pub(crate) fn close_on_escape(mut self, on: bool) -> Self {
        self.close_on_escape = on;
        self
    }
}

/// 浮层内容。
///
/// 本轮仅支持 `Primitives`：已经构造好的 `OverlayPrimitive` 列表，rect/frame 以**浮层左上角为 (0, 0) 的局部坐标系**。
/// `emit_overlay` 在求解出最终位置后会平移到窗口坐标。
pub(crate) enum OverlayContent {
    Primitives(Vec<OverlayPrimitive>),
}

/// 浮层内单个渲染原语（局部坐标）。
#[derive(Clone)]
pub(crate) enum OverlayPrimitive {
    Shape(RenderPrimitive),
    Text(TextPrimitive),
}
