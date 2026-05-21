//! `solve_placement` 纯函数单测。
//!
//! 覆盖：基础贴合、四向翻转、对齐、视口溢出、shift / clamp、hide、匹配宽度、点锚点、内容大于视口。

use crate::ui::unit::{dp, Dp};
use crate::ui::widget::common::{Point, Rect};
use crate::ui::widget::overlay::{
    solve_placement, Alignment, Anchor, FlipPolicy, Placement, PlacementOptions, Side,
};

fn viewport() -> Rect {
    Rect::new(dp(0.0), dp(0.0), dp(800.0), dp(600.0))
}

fn opts_default() -> PlacementOptions {
    PlacementOptions {
        placement: Placement::bottom(),
        offset: dp(8.0),
        cross_offset: Dp::ZERO,
        flip: FlipPolicy::FlipSide,
        viewport_padding: dp(0.0),
        clamp_to_viewport: true,
        match_anchor_width: false,
    }
}

fn approx_eq(a: Dp, b: f32) {
    assert!(
        (a.get() - b).abs() < 0.001,
        "expected {b}, got {}",
        a.get()
    );
}

#[test]
fn solves_below_when_fits() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(120.0), dp(60.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement.side, Side::Bottom);
    assert!(!solved.did_flip);
    assert!(!solved.was_hidden);
    // 居中对齐：anchor.x + (80 - 120)/2 = 100 - 20 = 80
    approx_eq(solved.rect.x, 80.0);
    // y = anchor.bottom + offset = 140 + 8 = 148
    approx_eq(solved.rect.y, 148.0);
    approx_eq(solved.rect.width, 120.0);
    approx_eq(solved.rect.height, 60.0);
}

#[test]
fn flips_to_top_when_below_overflows() {
    // anchor 贴近视口底部，下方空间不足，应翻到上方。
    let anchor = Rect::new(dp(100.0), dp(540.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipSide,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(120.0), dp(80.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
    // y = anchor.y - offset - content.h = 540 - 8 - 80 = 452
    approx_eq(solved.rect.y, 452.0);
}

#[test]
fn flip_side_picks_better_when_neither_fully_fits() {
    // anchor 在底部稍微贴底，下方空间不足，但上方空间也勉强够；取拟合度更高的一侧。
    let anchor = Rect::new(dp(100.0), dp(560.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipSide,
        ..opts_default()
    };
    // 内容高 120：下方仅 600 - 600 = 0 可用；上方有 560 可用 → 必翻
    let solved = solve_placement(Anchor::Rect(anchor), (dp(80.0), dp(120.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
}

#[test]
fn shift_only_clamps_into_viewport_without_flipping() {
    // anchor 紧贴右边，下方放得下，但内容会向右溢出；ShiftOnly 应平移而非翻转。
    let anchor = Rect::new(dp(750.0), dp(100.0), dp(40.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::ShiftOnly,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(200.0), dp(60.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement.side, Side::Bottom);
    assert!(!solved.did_flip);
    // 右贴边：x = viewport.right - content.w = 800 - 200 = 600
    approx_eq(solved.rect.x, 600.0);
}

#[test]
fn flip_and_shift_combines_axes() {
    // 锚点贴右下角：垂直要翻到上方，水平要左移避免溢出。
    let anchor = Rect::new(dp(770.0), dp(560.0), dp(20.0), dp(20.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipAndShift,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(200.0), dp(120.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
    // 水平移位：x = 800 - 200 = 600
    approx_eq(solved.rect.x, 600.0);
}

#[test]
fn hide_policy_sets_was_hidden_when_neither_side_fits() {
    // 视口太小，内容两侧都放不下。
    let small_viewport = Rect::new(dp(0.0), dp(0.0), dp(200.0), dp(100.0));
    let anchor = Rect::new(dp(80.0), dp(40.0), dp(40.0), dp(20.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::Hide,
        ..opts_default()
    };
    let solved =
        solve_placement(Anchor::Rect(anchor), (dp(300.0), dp(150.0)), small_viewport, &opts);
    assert!(solved.was_hidden);
}

#[test]
fn hide_policy_flips_when_only_other_side_fits() {
    // 下方放不下，但翻到上方能完整放下。
    let anchor = Rect::new(dp(100.0), dp(550.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::Hide,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(80.0), dp(60.0)), viewport(), &opts);
    assert!(!solved.was_hidden);
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
}

#[test]
fn alignment_on_bottom_side() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(80.0), dp(40.0));
    let content = (dp(40.0), dp(40.0));

    for (alignment, expected_x) in [
        (Alignment::Start, 100.0),
        (Alignment::Center, 100.0 + (80.0 - 40.0) / 2.0),
        (Alignment::End, 100.0 + 80.0 - 40.0),
    ] {
        let opts = PlacementOptions {
            placement: Placement::bottom().align(alignment),
            ..opts_default()
        };
        let solved = solve_placement(Anchor::Rect(anchor), content, viewport(), &opts);
        approx_eq(solved.rect.x, expected_x);
    }
}

#[test]
fn alignment_on_right_side() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(80.0));
    let content = (dp(40.0), dp(40.0));

    for (alignment, expected_y) in [
        (Alignment::Start, 100.0),
        (Alignment::Center, 100.0 + (80.0 - 40.0) / 2.0),
        (Alignment::End, 100.0 + 80.0 - 40.0),
    ] {
        let opts = PlacementOptions {
            placement: Placement::right().align(alignment),
            ..opts_default()
        };
        let solved = solve_placement(Anchor::Rect(anchor), content, viewport(), &opts);
        approx_eq(solved.rect.y, expected_y);
    }
}

#[test]
fn match_anchor_width_overrides_content_width() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(220.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        match_anchor_width: true,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(60.0), dp(80.0)), viewport(), &opts);
    approx_eq(solved.rect.width, 220.0);
    approx_eq(solved.rect.x, 100.0); // 宽度被锚点覆盖后，居中等价于贴左
}

#[test]
fn match_anchor_width_ignored_for_horizontal_sides() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(220.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::right(),
        match_anchor_width: true,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(60.0), dp(80.0)), viewport(), &opts);
    approx_eq(solved.rect.width, 60.0); // Left/Right 不受 match_anchor_width 影响
}

#[test]
fn point_anchor_zero_size_handled() {
    // Point 锚点 -> 零尺寸 anchor_rect；浮层应贴在点附近。
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        offset: dp(0.0),
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Point(Point::new(dp(200.0), dp(200.0))),
        (dp(80.0), dp(60.0)),
        viewport(),
        &opts,
    );
    // 锚点宽度 0，居中对齐：x = 200 + (0 - 80) / 2 = 160
    approx_eq(solved.rect.x, 160.0);
    // y = anchor.y + 0 + 0 = 200
    approx_eq(solved.rect.y, 200.0);
}

#[test]
fn content_larger_than_viewport_clamps_to_origin() {
    let small_viewport = Rect::new(dp(0.0), dp(0.0), dp(100.0), dp(100.0));
    let anchor = Rect::new(dp(50.0), dp(50.0), dp(10.0), dp(10.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipSide,
        clamp_to_viewport: true,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(200.0), dp(200.0)), small_viewport, &opts);
    // 内容超出视口，shift_into 会把它贴到 (0, 0)（因为 x = 100 - 200 = -100 然后被 max 到 0）。
    approx_eq(solved.rect.x, 0.0);
    approx_eq(solved.rect.y, 0.0);
}

#[test]
fn viewport_padding_keeps_distance_from_edges() {
    // 视口 padding=16，内容很高溢出底部时，shift_into 应让顶部贴 padding 边界（y=16）。
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        viewport_padding: dp(16.0),
        flip: FlipPolicy::ShiftOnly,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(40.0), dp(580.0)), viewport(), &opts);
    // 内容高 580 > 内视口高 568：shift_into 先把底部对齐到 inner_bottom，
    // 再把顶部贴 inner_top=16（因为顶部仍越界）。
    approx_eq(solved.rect.y, 16.0);
}

#[test]
fn negative_cross_offset_shifts_content_backwards() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom().align(Alignment::Start),
        cross_offset: dp(-30.0),
        clamp_to_viewport: false,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(40.0), dp(40.0)), viewport(), &opts);
    approx_eq(solved.rect.x, 70.0); // 100 + (-30) = 70
}

#[test]
fn clip_rect_intersects_with_viewport() {
    // 浮层左上角溢出 -10/-10，clip_rect 应被裁掉负的部分。
    let anchor = Rect::new(dp(0.0), dp(0.0), dp(10.0), dp(10.0));
    let opts = PlacementOptions {
        placement: Placement::top(),
        clamp_to_viewport: false,
        flip: FlipPolicy::None,
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(100.0), dp(60.0)), viewport(), &opts);
    // 内容 rect 至少部分溢出视口；clip_rect 应只覆盖 viewport ∩ rect 的部分。
    assert!(solved.clip_rect.x >= viewport().x);
    assert!(solved.clip_rect.y >= viewport().y);
}

#[test]
fn fits_without_change_returns_input_placement() {
    let anchor = Rect::new(dp(400.0), dp(300.0), dp(40.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::top().align(Alignment::End),
        ..opts_default()
    };
    let solved = solve_placement(Anchor::Rect(anchor), (dp(80.0), dp(40.0)), viewport(), &opts);
    assert_eq!(solved.resolved_placement, Placement::top().align(Alignment::End));
    assert!(!solved.did_flip);
    assert!(!solved.was_hidden);
}
