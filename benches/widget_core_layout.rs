use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};

#[cfg(feature = "bench-support")]
use std::time::Instant;

#[cfg(feature = "bench-support")]
use tgui::core::{dp, Color, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::theme::StateValue;
#[cfg(feature = "bench-support")]
use tgui::widgets::{
    Button, Card, Flex, ItemLayout, Pagination, Radio, SelectArrowBenchmarkContext, Spinner, Text,
    Tree, TreeNode, VirtualList, WidgetBenchmarkContext, WidgetTree,
};

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn build_flat_tree(rows: usize) -> WidgetTree<()> {
    let mut list = Flex::new(Axis::Vertical)
        .width(dp(900.0))
        .gap(dp(4.0))
        .padding(Insets::all(dp(8.0)));

    for row in 0..rows {
        list = list.child(
            Flex::new(Axis::Horizontal)
                .width(dp(860.0))
                .height(dp(36.0))
                .gap(dp(8.0))
                .padding(Insets::symmetric(dp(8.0), dp(4.0)))
                .child(Text::new(format!("Item {row:04}")))
                .child(Text::new(format!("status {}", row % 5)))
                .child(Button::new("Open").size(dp(80.0), dp(28.0))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(960.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(16.0)))
            .child(list),
    )
}

#[cfg(feature = "bench-support")]
fn build_nested_tree(depth: usize) -> WidgetTree<()> {
    let mut node = Flex::new(Axis::Vertical)
        .width(dp(120.0))
        .height(dp(32.0))
        .padding(Insets::all(dp(2.0)))
        .child(Button::new("Leaf").size(dp(96.0), dp(28.0)));

    for level in 0..depth {
        node = Flex::new(Axis::Vertical)
            .width(dp(160.0 + level as f32 * 12.0))
            .padding(Insets::all(dp(2.0)))
            .child(Text::new(format!("Level {level}")))
            .child(node);
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(640.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(node),
    )
}

#[cfg(feature = "bench-support")]
fn build_scroll_tree(rows: usize) -> WidgetTree<()> {
    let mut content = Flex::new(Axis::Vertical)
        .width(dp(1100.0))
        .gap(dp(3.0))
        .padding(Insets::all(dp(8.0)));

    for row in 0..rows {
        content = content.child(
            Flex::new(Axis::Horizontal)
                .width(dp(1060.0))
                .height(dp(30.0))
                .gap(dp(12.0))
                .padding(Insets::symmetric(dp(8.0), dp(3.0)))
                .child(Text::new(format!("row-{row:04}")))
                .child(Text::new(format!("owner {}", row % 17)))
                .child(Text::new(format!(
                    "The quick brown fox jumps over bucket {}",
                    row % 31
                ))),
        );
    }

    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1160.0))
            .height(dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(1120.0))
                    .height(dp(640.0))
                    .overflow_y(Overflow::Scroll)
                    .child(content),
            ),
    )
}

#[cfg(feature = "bench-support")]
fn build_virtual_list_tree(rows: usize, item_layout: ItemLayout) -> WidgetTree<()> {
    WidgetTree::new(
        VirtualList::new((0..rows).collect::<Vec<_>>(), |index, _| {
            Text::new(format!("row-{index}")).height(dp(40.0)).into()
        })
        .item_layout(item_layout)
        .size(dp(960.0), dp(720.0)),
    )
}

#[cfg(feature = "bench-support")]
fn build_dense_tree(rows: usize) -> WidgetTree<()> {
    let nodes = (0..rows)
        .map(|index| TreeNode::keyed(index, index))
        .collect::<Vec<_>>();
    WidgetTree::new(
        Tree::<usize, ()>::new(nodes, |context| {
            Text::new(format!("Node {:05}", context.item)).into()
        })
        .item_layout(ItemLayout::Fixed {
            item_extent: dp(32.0),
            spacing: dp(0.0),
            overscan: 2,
        })
        .size(dp(960.0), dp(720.0)),
    )
}

#[cfg(feature = "bench-support")]
fn build_card_shadow_tree(cards: usize, elevated: bool) -> WidgetTree<()> {
    let mut content = Flex::new(Axis::Vertical)
        .width(dp(960.0))
        .gap(dp(12.0))
        .padding(Insets::all(dp(24.0)));

    for index in 0..cards {
        let height = 88.0 + index as f32;
        let mut card = Card::new()
            .header(Text::new(format!("Card {index:03}")))
            .body(Text::new("A compact surface with predictable geometry."))
            .size(dp(880.0), dp(height));
        card = card.style(move |style, context| {
            style.shadow = if elevated {
                context.theme.elevation.sm.clone()
            } else {
                context.theme.elevation.none.clone()
            };
        });
        content = content.child(card);
    }

    WidgetTree::new(content)
}

#[cfg(feature = "bench-support")]
fn card_shadow_viewport(cards: usize) -> Rect {
    let card_heights = cards as f32 * 88.0 + (cards.saturating_sub(1) * cards) as f32 / 2.0;
    let gaps = cards.saturating_sub(1) as f32 * 12.0;
    Rect::new(0.0, 0.0, 1280.0, card_heights + gaps + 48.0)
}

#[cfg(feature = "bench-support")]
fn build_spinner_track_tree(spinners: usize, show_track: bool) -> WidgetTree<()> {
    const COLUMNS: usize = 20;
    let mut body = Flex::new(Axis::Vertical).gap(dp(4.0));
    for row_start in (0..spinners).step_by(COLUMNS) {
        let mut row = Flex::new(Axis::Horizontal).height(dp(24.0)).gap(dp(4.0));
        for _ in row_start..(row_start + COLUMNS).min(spinners) {
            row = row.child(Spinner::new().track(show_track).size(dp(20.0), dp(20.0)));
        }
        body = body.child(row);
    }
    WidgetTree::new(body)
}

#[cfg(feature = "bench-support")]
fn spinner_track_viewport(spinners: usize) -> Rect {
    const COLUMNS: usize = 20;
    let rows = spinners.div_ceil(COLUMNS);
    Rect::new(0.0, 0.0, 480.0, rows as f32 * 28.0)
}

#[cfg(feature = "bench-support")]
fn build_pagination_options_tree(
    paginations: usize,
    with_page_size_options: bool,
) -> WidgetTree<()> {
    let mut body = Flex::new(Axis::Vertical).width(dp(1240.0)).gap(dp(4.0));
    for _ in 0..paginations {
        let pagination = Pagination::new(6usize, 12usize).page_size(25usize);
        body = body.child(if with_page_size_options {
            pagination
        } else {
            pagination.page_size_options(Vec::new())
        });
    }
    WidgetTree::new(body)
}

#[cfg(feature = "bench-support")]
fn pagination_options_viewport(paginations: usize) -> Rect {
    Rect::new(0.0, 0.0, 1280.0, paginations as f32 * 44.0)
}

#[cfg(feature = "bench-support")]
fn build_radio_surface_tree(radios: usize, filled: bool) -> WidgetTree<()> {
    const COLUMNS: usize = 40;
    let mut body = Flex::new(Axis::Vertical).width(dp(1000.0)).gap(dp(1.0));
    for row_start in (0..radios).step_by(COLUMNS) {
        let mut row = Flex::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(4.0));
        for _ in row_start..(row_start + COLUMNS).min(radios) {
            let radio = Radio::new(true).size(dp(20.0), dp(20.0));
            row = row.child(if filled {
                radio.style(|style, _| {
                    let fill = Color::rgba(226, 232, 240, 255);
                    style.background = StateValue::new(fill.into());
                    style.background_checked = StateValue::new(fill.into());
                })
            } else {
                radio
            });
        }
        body = body.child(row);
    }
    WidgetTree::new(body)
}

#[cfg(feature = "bench-support")]
fn radio_surface_viewport(radios: usize) -> Rect {
    const COLUMNS: usize = 40;
    let rows = radios.div_ceil(COLUMNS);
    Rect::new(0.0, 0.0, 1000.0, rows as f32 * 23.0)
}

#[cfg(feature = "bench-support")]
fn bench_element_tree_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_tree_build");

    for rows in [10_usize, 50, 100, 200] {
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, &rows| {
            b.iter(|| {
                let tree = build_flat_tree(black_box(rows));
                black_box(tree);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_flat_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_flat_full_layout");

    for rows in [10_usize, 50, 100, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_nested_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_nested_full_layout");

    for depth in [2_usize, 4, 8, 12, 16] {
        let tree = build_nested_tree(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_full_layout_and_scene(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_full_layout_and_scene");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            b.iter(|| {
                ctx.invalidate_all();
                let stats = ctx.run_layout_and_scene(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scene_recollect_cached_layout");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_cached_scene_hit_path");

    for rows in [50_usize, 200, 500] {
        let tree = build_flat_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let hit_len = ctx.cached_hit_path_len(
                    black_box(&tree),
                    black_box(Point::new(640.0, 360.0)),
                    Instant::now(),
                );
                black_box(hit_len);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scroll_container_scene_recollect(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scroll_scene_recollect_cached_layout");

    for rows in [50_usize, 200, 500, 1_000] {
        let tree = build_scroll_tree(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scroll_content_bounds_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scroll_content_bounds_cache");

    for rows in [500_usize, 1_000] {
        let tree = build_scroll_tree(rows);

        group.bench_with_input(BenchmarkId::new("retained", rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });

        group.bench_with_input(BenchmarkId::new("forced_rescan", rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                ctx.clear_cached_content_bounds();
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scroll_child_culling(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_scroll_child_culling");

    for rows in [1_000_usize, 10_000] {
        let tree = build_scroll_tree(rows);

        group.bench_with_input(BenchmarkId::new("indexed", rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout_and_scene(&tree, Instant::now());
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });

        group.bench_with_input(BenchmarkId::new("full_scan", rows), &rows, |b, _| {
            let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
            let _ = ctx.run_layout(&tree, Instant::now());
            ctx.disable_cached_child_culling();
            b.iter(|| {
                let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                black_box(stats);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_virtual_window_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_virtual_window_planning");

    for rows in [1_000_usize, 10_000, 100_000] {
        for (name, item_layout) in [
            (
                "fixed",
                ItemLayout::Fixed {
                    item_extent: dp(40.0),
                    spacing: dp(4.0),
                    overscan: 2,
                },
            ),
            (
                "estimated",
                ItemLayout::Estimated {
                    estimate: dp(40.0),
                    spacing: dp(4.0),
                    overscan: 2,
                },
            ),
            (
                "measured",
                ItemLayout::Measured {
                    estimate: dp(40.0),
                    spacing: dp(4.0),
                    overscan: 2,
                },
            ),
        ] {
            let tree = build_virtual_list_tree(rows, item_layout);
            group.bench_with_input(
                BenchmarkId::new(format!("{name}/bounded"), rows),
                &rows,
                |b, _| {
                    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                    b.iter(|| {
                        ctx.invalidate_all();
                        black_box(ctx.run_layout(black_box(&tree), Instant::now()));
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("{name}/legacy_full_scan"), rows),
                &rows,
                |b, _| {
                    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                    b.iter(|| {
                        black_box(ctx.run_layout_with_legacy_virtual_window_plan(
                            black_box(&tree),
                            Instant::now(),
                        ));
                    });
                },
            );
        }
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_tree_row_source(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_tree_row_source");

    for rows in [1_000_usize, 10_000] {
        let tree = build_dense_tree(rows);
        let mut retained = WidgetBenchmarkContext::new().with_viewport(viewport());
        let _ = retained.run_layout(&tree, Instant::now());
        group.bench_with_input(
            BenchmarkId::new("retained_snapshot", rows),
            &rows,
            |b, _| {
                b.iter(|| {
                    retained.invalidate_all();
                    black_box(retained.run_layout(black_box(&tree), Instant::now()));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("legacy_per_query_flatten", rows),
            &rows,
            |b, _| {
                let mut legacy = WidgetBenchmarkContext::new().with_viewport(viewport());
                b.iter(|| {
                    black_box(
                        legacy.run_layout_with_legacy_tree_row_source(
                            black_box(&tree),
                            Instant::now(),
                        ),
                    );
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_single_row_update_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_single_row_update_paths");

    for rows in [500_usize, 1000] {
        let tree = build_flat_tree(rows);

        group.bench_with_input(
            BenchmarkId::new("full_layout_and_scene", rows),
            &rows,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                b.iter(|| {
                    ctx.invalidate_all();
                    let stats = ctx.run_layout_and_scene(black_box(&tree), Instant::now());
                    black_box(stats);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("layout_root_patch_and_scene", rows),
            &rows,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let patched = ctx.patch_parent_of_deepest_leaf_layout_and_scene(
                        black_box(&tree),
                        Instant::now(),
                    );
                    black_box(patched);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scene_only_recollect", rows),
            &rows,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport());
                let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                b.iter(|| {
                    let stats = ctx.recollect_scene_only(black_box(&tree), Instant::now());
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_card_shadow(c: &mut Criterion) {
    let mut group = c.benchmark_group("card_shadow");
    group.sample_size(20);

    for cards in [24_usize, 100] {
        for (variant, elevated) in [("border_only", false), ("elevated", true)] {
            let tree = build_card_shadow_tree(cards, elevated);
            let card_viewport = card_shadow_viewport(cards);

            let mut validation = WidgetBenchmarkContext::new().with_viewport(card_viewport);
            let _ = validation.run_layout_and_scene(&tree, Instant::now());
            let textures = validation
                .cached_texture_scene_stats()
                .expect("card benchmark should retain a cached scene");
            assert_eq!(
                textures.unique_texture_ids,
                if elevated { cards } else { 0 },
                "each elevated card must exercise a unique shadow texture",
            );

            group.bench_with_input(
                BenchmarkId::new(format!("cold_{variant}"), cards),
                &cards,
                |b, _| {
                    b.iter_batched_ref(
                        || WidgetBenchmarkContext::new().with_viewport(card_viewport),
                        |ctx| {
                            black_box(ctx.run_layout_and_scene(black_box(&tree), Instant::now()));
                        },
                        BatchSize::SmallInput,
                    );
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("recollect_{variant}"), cards),
                &cards,
                |b, _| {
                    let mut ctx = WidgetBenchmarkContext::new().with_viewport(card_viewport);
                    let _ = ctx.run_layout_and_scene(&tree, Instant::now());
                    b.iter(|| {
                        black_box(ctx.recollect_scene_only(black_box(&tree), Instant::now()));
                    });
                },
            );
        }
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_spinner_track(c: &mut Criterion) {
    let mut group = c.benchmark_group("spinner_track_scene_recollect");
    group.sample_size(30);

    for spinners in [100_usize, 1_000] {
        let viewport = spinner_track_viewport(spinners);
        let indicator_only = build_spinner_track_tree(spinners, false);
        let with_track = build_spinner_track_tree(spinners, true);

        let mut indicator_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
        let indicator_stats =
            indicator_validation.run_layout_and_scene(&indicator_only, Instant::now());
        let mut track_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
        let track_stats = track_validation.run_layout_and_scene(&with_track, Instant::now());
        assert_eq!(indicator_stats.mesh_count, spinners);
        assert_eq!(track_stats.mesh_count, spinners * 2);
        assert!(indicator_stats.mesh_vertex_count < track_stats.mesh_vertex_count);
        eprintln!(
            "spinner_track_scene: spinners={spinners} indicator_meshes={} track_meshes={} indicator_vertices={} track_vertices={} vertex_reduction_pct={:.1}",
            indicator_stats.mesh_count,
            track_stats.mesh_count,
            indicator_stats.mesh_vertex_count,
            track_stats.mesh_vertex_count,
            100.0
                * (1.0
                    - indicator_stats.mesh_vertex_count as f64
                        / track_stats.mesh_vertex_count as f64),
        );

        for (variant, tree) in [
            ("indicator_only", &indicator_only),
            ("track_and_indicator", &with_track),
        ] {
            group.bench_with_input(BenchmarkId::new(variant, spinners), &spinners, |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport);
                let _ = ctx.run_layout_and_scene(tree, Instant::now());
                b.iter(|| {
                    black_box(ctx.recollect_scene_only(black_box(tree), Instant::now()));
                });
            });
        }
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_pagination_default_options(c: &mut Criterion) {
    let mut group = c.benchmark_group("pagination_default_options");
    group.sample_size(30);

    for paginations in [24_usize, 100] {
        let viewport = pagination_options_viewport(paginations);
        let with_options = build_pagination_options_tree(paginations, true);
        let minimal = build_pagination_options_tree(paginations, false);

        let mut with_options_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
        let with_options_stats =
            with_options_validation.run_layout_and_scene(&with_options, Instant::now());
        let mut minimal_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
        let minimal_stats = minimal_validation.run_layout_and_scene(&minimal, Instant::now());
        assert_eq!(
            with_options_stats.text_count - minimal_stats.text_count,
            paginations * 4,
        );
        assert_eq!(
            with_options_stats.hit_region_count - minimal_stats.hit_region_count,
            paginations * 4,
        );
        eprintln!(
            "pagination_default_options: paginations={paginations} with_options={with_options_stats:?} minimal={minimal_stats:?}",
        );

        for (variant, tree) in [("with_options", &with_options), ("minimal", &minimal)] {
            group.bench_with_input(
                BenchmarkId::new(format!("cold_{variant}"), paginations),
                &paginations,
                |b, _| {
                    b.iter_batched_ref(
                        || WidgetBenchmarkContext::new().with_viewport(viewport),
                        |ctx| {
                            black_box(ctx.run_layout_and_scene(black_box(tree), Instant::now()));
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("recollect_{variant}"), paginations),
                &paginations,
                |b, _| {
                    let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport);
                    let _ = ctx.run_layout_and_scene(tree, Instant::now());
                    b.iter(|| {
                        black_box(ctx.recollect_scene_only(black_box(tree), Instant::now()));
                    });
                },
            );
        }
    }

    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_radio_surface(c: &mut Criterion) {
    const RADIOS: usize = 1000;
    let viewport = radio_surface_viewport(RADIOS);
    let outline_only = build_radio_surface_tree(RADIOS, false);
    let filled = build_radio_surface_tree(RADIOS, true);

    let mut outline_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
    let outline_stats = outline_validation.run_layout_and_scene(&outline_only, Instant::now());
    let mut filled_validation = WidgetBenchmarkContext::new().with_viewport(viewport);
    let filled_stats = filled_validation.run_layout_and_scene(&filled, Instant::now());
    assert_eq!(outline_stats.shape_count, RADIOS);
    assert_eq!(filled_stats.shape_count, RADIOS * 2);
    assert_eq!(outline_stats.overlay_shape_count, RADIOS);
    assert_eq!(filled_stats.overlay_shape_count, RADIOS);
    assert_eq!(
        outline_stats.hit_region_count,
        filled_stats.hit_region_count
    );
    eprintln!(
        "radio_surface_scene: radios={RADIOS} outline_shapes={} filled_shapes={} outline_indicators={} filled_indicators={} shape_reduction_pct={:.1}",
        outline_stats.shape_count,
        filled_stats.shape_count,
        outline_stats.overlay_shape_count,
        filled_stats.overlay_shape_count,
        100.0 * (1.0 - outline_stats.shape_count as f64 / filled_stats.shape_count as f64),
    );

    let mut group = c.benchmark_group("radio_surface");
    group.sample_size(20);
    for (variant, tree) in [("outline_only", &outline_only), ("filled", &filled)] {
        group.bench_with_input(
            BenchmarkId::new(format!("cold_{variant}"), RADIOS),
            &RADIOS,
            |b, _| {
                b.iter_batched_ref(
                    || WidgetBenchmarkContext::new().with_viewport(viewport),
                    |ctx| {
                        black_box(ctx.run_layout_and_scene(black_box(tree), Instant::now()));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new(format!("recollect_{variant}"), RADIOS),
            &RADIOS,
            |b, _| {
                let mut ctx = WidgetBenchmarkContext::new().with_viewport(viewport);
                let _ = ctx.run_layout_and_scene(tree, Instant::now());
                b.iter(|| {
                    black_box(ctx.recollect_scene_only(black_box(tree), Instant::now()));
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_select_arrow_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("select_arrow_state_animation");
    for selects in [1_usize, 24] {
        group.bench_with_input(
            BenchmarkId::new("legacy_stateful_svg_tint", selects),
            &selects,
            |b, &selects| {
                b.iter_batched(
                    || SelectArrowBenchmarkContext::new(selects, true),
                    |mut context| black_box(context.run_pressed_animation()),
                    BatchSize::SmallInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("constant_neutral_svg_tint", selects),
            &selects,
            |b, &selects| {
                b.iter_batched(
                    || SelectArrowBenchmarkContext::new(selects, false),
                    |mut context| black_box(context.run_pressed_animation()),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

#[cfg(not(feature = "bench-support"))]
fn bench_select_arrow_state(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_element_tree_build(_c: &mut Criterion) {
    eprintln!("Skipping widget_core_layout benchmarks: bench-support feature not enabled");
}

#[cfg(not(feature = "bench-support"))]
fn bench_flat_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_nested_layout(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_full_layout_and_scene(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scene_recollect(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_hit_test(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_container_scene_recollect(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_content_bounds_cache(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_scroll_child_culling(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_single_row_update_paths(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_virtual_window_planning(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_tree_row_source(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_card_shadow(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_spinner_track(_c: &mut Criterion) {}

#[cfg(not(feature = "bench-support"))]
fn bench_pagination_default_options(_c: &mut Criterion) {}

#[cfg(feature = "bench-support")]
fn bench_widget_core_layout(c: &mut Criterion) {
    bench_radio_surface(c);
    if std::env::var_os("TGUI_RADIO_SURFACE_ONLY").is_some() {
        return;
    }
    bench_element_tree_build(c);
    bench_flat_layout(c);
    bench_nested_layout(c);
    bench_full_layout_and_scene(c);
    bench_scene_recollect(c);
    bench_hit_test(c);
    bench_scroll_container_scene_recollect(c);
    bench_scroll_content_bounds_cache(c);
    bench_scroll_child_culling(c);
    bench_virtual_window_planning(c);
    bench_tree_row_source(c);
    bench_single_row_update_paths(c);
    bench_card_shadow(c);
    bench_spinner_track(c);
    bench_pagination_default_options(c);
    bench_select_arrow_state(c);
}

#[cfg(not(feature = "bench-support"))]
fn bench_widget_core_layout(c: &mut Criterion) {
    bench_element_tree_build(c);
    bench_flat_layout(c);
    bench_nested_layout(c);
    bench_full_layout_and_scene(c);
    bench_scene_recollect(c);
    bench_hit_test(c);
    bench_scroll_container_scene_recollect(c);
    bench_scroll_content_bounds_cache(c);
    bench_scroll_child_culling(c);
    bench_virtual_window_planning(c);
    bench_tree_row_source(c);
    bench_single_row_update_paths(c);
    bench_card_shadow(c);
    bench_spinner_track(c);
    bench_pagination_default_options(c);
    bench_select_arrow_state(c);
}

criterion_group!(benches, bench_widget_core_layout);
criterion_main!(benches);
