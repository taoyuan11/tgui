//! `solve_placement` 纯函数单测。
//!
//! 覆盖：基础贴合、四向翻转、对齐、视口溢出、shift / clamp、hide、匹配宽度、点锚点、内容大于视口。

use crate::runtime::overlay::{
    solve_placement, Alignment, Anchor, FlipPolicy, Placement, PlacementOptions, Side,
};
use crate::ui::unit::{dp, Dp};
use crate::ui::widget::{Point, Rect};

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
    assert!((a.get() - b).abs() < 0.001, "expected {b}, got {}", a.get());
}

#[test]
fn solves_below_when_fits() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(120.0), dp(60.0)),
        viewport(),
        &opts,
    );
    assert_eq!(solved.resolved_placement.side, Side::Bottom);
    assert!(!solved.did_flip);
    assert!(!solved.was_hidden);
    approx_eq(solved.rect.x, 80.0);
    approx_eq(solved.rect.y, 148.0);
    approx_eq(solved.rect.width, 120.0);
    approx_eq(solved.rect.height, 60.0);
}

#[test]
fn flips_to_top_when_below_overflows() {
    let anchor = Rect::new(dp(100.0), dp(540.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipSide,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(120.0), dp(80.0)),
        viewport(),
        &opts,
    );
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
    approx_eq(solved.rect.y, 452.0);
}

#[test]
fn flip_side_picks_better_when_neither_fully_fits() {
    let anchor = Rect::new(dp(100.0), dp(560.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipSide,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(80.0), dp(120.0)),
        viewport(),
        &opts,
    );
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
}

#[test]
fn shift_only_clamps_into_viewport_without_flipping() {
    let anchor = Rect::new(dp(750.0), dp(100.0), dp(40.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::ShiftOnly,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(200.0), dp(60.0)),
        viewport(),
        &opts,
    );
    assert_eq!(solved.resolved_placement.side, Side::Bottom);
    assert!(!solved.did_flip);
    approx_eq(solved.rect.x, 600.0);
}

#[test]
fn flip_and_shift_combines_axes() {
    let anchor = Rect::new(dp(770.0), dp(560.0), dp(20.0), dp(20.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::FlipAndShift,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(200.0), dp(120.0)),
        viewport(),
        &opts,
    );
    assert_eq!(solved.resolved_placement.side, Side::Top);
    assert!(solved.did_flip);
    approx_eq(solved.rect.x, 600.0);
}

#[test]
fn hide_policy_sets_was_hidden_when_neither_side_fits() {
    let small_viewport = Rect::new(dp(0.0), dp(0.0), dp(200.0), dp(100.0));
    let anchor = Rect::new(dp(80.0), dp(40.0), dp(40.0), dp(20.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::Hide,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(300.0), dp(150.0)),
        small_viewport,
        &opts,
    );
    assert!(solved.was_hidden);
}

#[test]
fn hide_policy_flips_when_only_other_side_fits() {
    let anchor = Rect::new(dp(100.0), dp(550.0), dp(80.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        flip: FlipPolicy::Hide,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(80.0), dp(60.0)),
        viewport(),
        &opts,
    );
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
        (Alignment::Center, 120.0),
        (Alignment::End, 140.0),
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
        (Alignment::Center, 120.0),
        (Alignment::End, 140.0),
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
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(60.0), dp(80.0)),
        viewport(),
        &opts,
    );
    approx_eq(solved.rect.width, 220.0);
    approx_eq(solved.rect.x, 100.0);
}

#[test]
fn match_anchor_width_ignored_for_horizontal_sides() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(220.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::right(),
        match_anchor_width: true,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(60.0), dp(80.0)),
        viewport(),
        &opts,
    );
    approx_eq(solved.rect.width, 60.0);
}

#[test]
fn point_anchor_zero_size_handled() {
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
    approx_eq(solved.rect.x, 160.0);
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
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(200.0), dp(200.0)),
        small_viewport,
        &opts,
    );
    approx_eq(solved.rect.x, 0.0);
    approx_eq(solved.rect.y, 0.0);
}

#[test]
fn viewport_padding_keeps_distance_from_edges() {
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let opts = PlacementOptions {
        placement: Placement::bottom(),
        viewport_padding: dp(16.0),
        flip: FlipPolicy::ShiftOnly,
        ..opts_default()
    };
    let solved = solve_placement(
        Anchor::Rect(anchor),
        (dp(40.0), dp(580.0)),
        viewport(),
        &opts,
    );
    approx_eq(solved.rect.y, 16.0);
}
