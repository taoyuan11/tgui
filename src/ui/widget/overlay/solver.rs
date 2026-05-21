//! 浮层定位求解器。
//!
//! `solve_placement` 是纯函数：输入锚点 / 内容尺寸 / 视口 / 选项，输出最终矩形与翻转结果。
//! 不依赖 widget / scene / runtime，可独立单测。

use crate::ui::layout::Insets;
use crate::ui::unit::Dp;
use crate::ui::widget::common::Rect;

use super::anchor::Anchor;
use super::placement::{
    Alignment, FlipPolicy, Placement, PlacementOptions, Side, SolvedPlacement,
};

/// 求解浮层最终位置。
///
/// `content_size = (width, height)` 是浮层内容的 Dp 尺寸；当 `opts.match_anchor_width` 为 true
/// 且 placement 为 Top/Bottom 时，宽度会被锚点宽度覆盖。
pub fn solve_placement(
    anchor: Anchor,
    content_size: (Dp, Dp),
    viewport: Rect,
    opts: &PlacementOptions,
) -> SolvedPlacement {
    let anchor_rect = anchor.to_rect();

    let mut content_w = content_size.0;
    let content_h = content_size.1;
    if opts.match_anchor_width && opts.placement.side.is_vertical() {
        content_w = anchor_rect.width;
    }

    let viewport_inner = inset_rect_safe(viewport, opts.viewport_padding);

    let primary = place_on_side(
        anchor_rect,
        content_w,
        content_h,
        opts.placement,
        opts.offset,
        opts.cross_offset,
    );

    if fits_within(primary, viewport_inner) {
        return finalize(primary, opts.placement, false, false, viewport, anchor_rect);
    }

    let (after_policy, resolved_placement, did_flip) = match opts.flip {
        FlipPolicy::None => (primary, opts.placement, false),
        FlipPolicy::ShiftOnly => (shift_into(primary, viewport_inner), opts.placement, false),
        FlipPolicy::FlipSide => pick_better_side(
            anchor_rect,
            content_w,
            content_h,
            opts.placement,
            opts.offset,
            opts.cross_offset,
            viewport_inner,
            primary,
        ),
        FlipPolicy::FlipAndShift => {
            let (chosen, chosen_placement, did_flip) = pick_better_side(
                anchor_rect,
                content_w,
                content_h,
                opts.placement,
                opts.offset,
                opts.cross_offset,
                viewport_inner,
                primary,
            );
            (
                shift_cross_into(chosen, viewport_inner, chosen_placement.side),
                chosen_placement,
                did_flip,
            )
        }
        FlipPolicy::Hide => {
            let flipped_placement =
                Placement::new(opts.placement.side.opposite(), opts.placement.alignment);
            let flipped = place_on_side(
                anchor_rect,
                content_w,
                content_h,
                flipped_placement,
                opts.offset,
                opts.cross_offset,
            );
            if fits_within(flipped, viewport_inner) {
                return finalize(flipped, flipped_placement, true, false, viewport, anchor_rect);
            }
            let zero = Rect::new(Dp::ZERO, Dp::ZERO, Dp::ZERO, Dp::ZERO);
            return SolvedPlacement {
                rect: zero,
                resolved_placement: opts.placement,
                clip_rect: zero,
                did_flip: false,
                was_hidden: true,
                anchor_rect,
            };
        }
    };

    // 统一后处理：clamp_to_viewport 兜底保证内容在视口内（None/FlipSide/FlipAndShift 都遵从）。
    let final_rect = if opts.clamp_to_viewport && !fits_within(after_policy, viewport_inner) {
        shift_into(after_policy, viewport_inner)
    } else {
        after_policy
    };

    finalize(final_rect, resolved_placement, did_flip, false, viewport, anchor_rect)
}

fn place_on_side(
    anchor: Rect,
    content_w: Dp,
    content_h: Dp,
    placement: Placement,
    offset: Dp,
    cross_offset: Dp,
) -> Rect {
    match placement.side {
        Side::Top => Rect::new(
            align_cross_horizontal(anchor, content_w, placement.alignment, cross_offset),
            anchor.y - offset - content_h,
            content_w,
            content_h,
        ),
        Side::Bottom => Rect::new(
            align_cross_horizontal(anchor, content_w, placement.alignment, cross_offset),
            anchor.y + anchor.height + offset,
            content_w,
            content_h,
        ),
        Side::Left => Rect::new(
            anchor.x - offset - content_w,
            align_cross_vertical(anchor, content_h, placement.alignment, cross_offset),
            content_w,
            content_h,
        ),
        Side::Right => Rect::new(
            anchor.x + anchor.width + offset,
            align_cross_vertical(anchor, content_h, placement.alignment, cross_offset),
            content_w,
            content_h,
        ),
    }
}

fn align_cross_horizontal(anchor: Rect, content_w: Dp, alignment: Alignment, cross: Dp) -> Dp {
    match alignment {
        Alignment::Start => anchor.x + cross,
        Alignment::Center => anchor.x + (anchor.width - content_w) * 0.5 + cross,
        Alignment::End => anchor.x + anchor.width - content_w + cross,
    }
}

fn align_cross_vertical(anchor: Rect, content_h: Dp, alignment: Alignment, cross: Dp) -> Dp {
    match alignment {
        Alignment::Start => anchor.y + cross,
        Alignment::Center => anchor.y + (anchor.height - content_h) * 0.5 + cross,
        Alignment::End => anchor.y + anchor.height - content_h + cross,
    }
}

fn fits_within(content: Rect, viewport: Rect) -> bool {
    content.x >= viewport.x
        && content.y >= viewport.y
        && content.x + content.width <= viewport.x + viewport.width
        && content.y + content.height <= viewport.y + viewport.height
}

/// 拟合度评分：可见面积 / 内容面积，范围 [0, 1]。
fn fit_score(content: Rect, viewport: Rect) -> f32 {
    let inter_x = content.x.max(viewport.x);
    let inter_y = content.y.max(viewport.y);
    let inter_right = (content.x + content.width).min(viewport.x + viewport.width);
    let inter_bottom = (content.y + content.height).min(viewport.y + viewport.height);
    let inter_w = (inter_right - inter_x).max(Dp::ZERO).get();
    let inter_h = (inter_bottom - inter_y).max(Dp::ZERO).get();
    let inter_area = inter_w * inter_h;
    let total = content.width.get() * content.height.get();
    if total <= 0.0 {
        0.0
    } else {
        (inter_area / total).clamp(0.0, 1.0)
    }
}

/// 在视口内移动 content 矩形（不改变尺寸）；若 content 比 viewport 大，则贴左/上对齐。
fn shift_into(content: Rect, viewport: Rect) -> Rect {
    let mut x = content.x;
    let mut y = content.y;
    if x + content.width > viewport.x + viewport.width {
        x = viewport.x + viewport.width - content.width;
    }
    if x < viewport.x {
        x = viewport.x;
    }
    if y + content.height > viewport.y + viewport.height {
        y = viewport.y + viewport.height - content.height;
    }
    if y < viewport.y {
        y = viewport.y;
    }
    Rect::new(x, y, content.width, content.height)
}

/// 仅沿交叉轴移位（保持主轴不动）。垂直 side（Top/Bottom）的交叉轴是 x；水平 side 的交叉轴是 y。
fn shift_cross_into(content: Rect, viewport: Rect, side: Side) -> Rect {
    if side.is_vertical() {
        let mut x = content.x;
        if x + content.width > viewport.x + viewport.width {
            x = viewport.x + viewport.width - content.width;
        }
        if x < viewport.x {
            x = viewport.x;
        }
        Rect::new(x, content.y, content.width, content.height)
    } else {
        let mut y = content.y;
        if y + content.height > viewport.y + viewport.height {
            y = viewport.y + viewport.height - content.height;
        }
        if y < viewport.y {
            y = viewport.y;
        }
        Rect::new(content.x, y, content.width, content.height)
    }
}

fn pick_better_side(
    anchor: Rect,
    content_w: Dp,
    content_h: Dp,
    placement: Placement,
    offset: Dp,
    cross_offset: Dp,
    viewport_inner: Rect,
    primary: Rect,
) -> (Rect, Placement, bool) {
    let flipped_placement = Placement::new(placement.side.opposite(), placement.alignment);
    let flipped = place_on_side(
        anchor,
        content_w,
        content_h,
        flipped_placement,
        offset,
        cross_offset,
    );
    let primary_fit = fit_score(primary, viewport_inner);
    let flipped_fit = fit_score(flipped, viewport_inner);
    if flipped_fit > primary_fit {
        (flipped, flipped_placement, true)
    } else {
        (primary, placement, false)
    }
}

fn inset_rect_safe(viewport: Rect, padding: Dp) -> Rect {
    if padding <= Dp::ZERO {
        viewport
    } else {
        // 当视口太小放不下 padding 时退化为原视口，避免负宽高。
        let inset = Insets::all(padding);
        let w = viewport.width - inset.left - inset.right;
        let h = viewport.height - inset.top - inset.bottom;
        if w <= Dp::ZERO || h <= Dp::ZERO {
            viewport
        } else {
            Rect::new(viewport.x + inset.left, viewport.y + inset.top, w, h)
        }
    }
}

fn finalize(
    rect: Rect,
    resolved_placement: Placement,
    did_flip: bool,
    was_hidden: bool,
    viewport: Rect,
    anchor_rect: Rect,
) -> SolvedPlacement {
    let clip_rect = clip_to_viewport(rect, viewport);
    SolvedPlacement {
        rect,
        resolved_placement,
        clip_rect,
        did_flip,
        was_hidden,
        anchor_rect,
    }
}

fn clip_to_viewport(content: Rect, viewport: Rect) -> Rect {
    let x = content.x.max(viewport.x);
    let y = content.y.max(viewport.y);
    let right = (content.x + content.width).min(viewport.x + viewport.width);
    let bottom = (content.y + content.height).min(viewport.y + viewport.height);
    let w = (right - x).max(Dp::ZERO);
    let h = (bottom - y).max(Dp::ZERO);
    Rect::new(x, y, w, h)
}
