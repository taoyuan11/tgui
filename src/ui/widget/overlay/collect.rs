//! Collect 阶段集成入口。
//!
//! 消费者 widget 在自己的 collect 函数末尾调用 `emit_overlay`，由它统一完成：
//! 1. 通过 solver 求出浮层在窗口坐标下的最终矩形；
//! 2. 把内容原语（局部坐标）平移到窗口坐标，写入 `ComputedScene::overlay_layers[layer]`；
//! 3. 注册 `OverlayCloseHandle` 到同一 bucket；
//! 4. collect 收尾时由 `ComputedScene::finalize_overlay_layers` 按层级顺序冲刷到
//!    `ScenePrimitives::overlay_*` / `overlay_hit_regions` / `overlay_close_handlers`，
//!    从而保证 Tooltip < Popover < Menu < Modal 的 z-order。
//!
//! 本轮内容模型限 `OverlayContent::Primitives`；Element 子树支持留待后续 PR。

use crate::ui::unit::Dp;
use crate::ui::widget::common::{ClipMask, ComputedScene, Point, RenderCommand, Rect};

use super::close::OverlayCloseHandle;
use super::overlay::{Overlay, OverlayContent, OverlayPrimitive};
use super::placement::SolvedPlacement;
use super::solver::solve_placement;

/// 把局部坐标矩形平移到窗口坐标。
fn translate_rect(rect: Rect, origin: Point) -> Rect {
    Rect::new(
        rect.x + origin.x.get(),
        rect.y + origin.y.get(),
        rect.width,
        rect.height,
    )
}

fn translate_clip_mask(mask: Option<ClipMask>, origin: Point) -> Option<ClipMask> {
    mask.map(|m| ClipMask {
        rect: translate_rect(m.rect, origin),
        corner_radius: m.corner_radius,
    })
}

/// 求解浮层位置并把内容写入 overlay scene。返回 `SolvedPlacement` 供调试 / 后续命中扩展。
///
/// `content_size` 是浮层内容的逻辑尺寸（Dp）。`content` 中的每条 primitive 的 rect/frame
/// 必须以**浮层左上角 (0, 0)** 为基准的局部坐标；本函数会按求解结果统一平移。
///
/// 写入 `computed.overlay_layers[layer]` 暂存桶；`finalize_overlay_layers` 在 collect 收尾
/// 时按层级顺序冲刷到平面列表。
///
/// 若 `solved.was_hidden`（FlipPolicy::Hide 且两侧均放不下），仅返回 solved，
/// 不写入任何 primitive、不注册 close handler。
pub(crate) fn emit_overlay<VM>(
    computed: &mut ComputedScene<VM>,
    viewport: Rect,
    overlay: Overlay<VM>,
    content_size: (Dp, Dp),
    content: OverlayContent,
) -> SolvedPlacement {
    let solved = solve_placement(overlay.anchor, content_size, viewport, &overlay.options);

    if solved.was_hidden {
        return solved;
    }

    let origin = Point::new(solved.rect.x, solved.rect.y);
    let overlay_clip = Some(solved.clip_rect);
    let bucket = &mut computed.overlay_layers[overlay.layer.index()];

    match content {
        OverlayContent::Primitives(prims) => {
            for prim in prims {
                match prim {
                    OverlayPrimitive::Shape(mut shape) => {
                        shape.rect = translate_rect(shape.rect, origin);
                        shape.clip_rect = overlay_clip;
                        shape.clip_mask = translate_clip_mask(shape.clip_mask, origin);
                        bucket.shapes.push(shape);
                        bucket.commands.push(RenderCommand::Shape(shape));
                    }
                    OverlayPrimitive::Text(mut text) => {
                        text.frame = translate_rect(text.frame, origin);
                        text.clip_rect = overlay_clip;
                        text.clip_mask = translate_clip_mask(text.clip_mask, origin);
                        if let Some(quad) = text.quad {
                            text.quad =
                                Some(quad.map(|p| Point::new(p.x + origin.x, p.y + origin.y)));
                        }
                        bucket.texts.push(text.clone());
                        bucket.commands.push(RenderCommand::Text(text));
                    }
                }
            }
        }
    }

    let has_close_hook =
        overlay.on_close.is_some() || overlay.close_on_outside_click || overlay.close_on_escape;
    if has_close_hook {
        bucket.close_handlers.push(OverlayCloseHandle {
            overlay_id: overlay.id,
            rect: solved.rect,
            layer: overlay.layer,
            on_close: overlay.on_close,
            close_value: false,
            return_focus_to: overlay.return_focus_to,
            close_on_outside_click: overlay.close_on_outside_click,
            close_on_escape: overlay.close_on_escape,
        });
    }

    solved
}
