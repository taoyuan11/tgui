//! Phase 0 单属性更新基准（度量护栏）。
//!
//! 构造 n=1k/10k 节点的深树，最深叶子挂在一个 `opacity` 受 `State` 驱动的容器下，
//! 对比两条路径的端到端耗时：
//!
//! - `single_leaf_patch` —— 改深层叶子视觉属性时走的子树 patch（重收集该叶子 chunk +
//!   沿祖先链 `recompose_scene_chunk` 向上合成到根）。这是运行时 `scene_subtree_patch`
//!   的核心成本，也是 Phase 1 命令区间 splice 要压平的目标。
//! - `full_recollect` —— 滚动/动画当前每帧的整树重收集（对照上界）。
//!
//! 跑：`cargo bench --features bench-support --bench single_property_update`

use std::hint::black_box;
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::core::dp;
use tgui::layout::{Axis, Insets};
use tgui::mvvm::{State, ViewModelContext};
use tgui::widgets::{Element, Flex, Stack, Text, WidgetBenchmarkContext, WidgetTree};

/// 构造一棵「主体宽扁 + 一条深链」的树：
/// - 大部分节点（`filler`）摊出目标节点规模，使祖先链上的兄弟数量真实；
/// - 一条 `depth` 层的链通到最深叶子，叶子文本固定，但链顶容器的 `opacity` 受
///   `State<f32>` 驱动 —— 改 opacity 即触发「单个深层叶子视觉属性变化」。
fn build_deep_leaf_tree(node_count: usize, depth: usize) -> (WidgetTree<()>, State<f32>) {
    let ctx = ViewModelContext::for_benchmarks();
    let opacity = ctx.state(1.0_f32);

    // 深链：root chain container (opacity-bound) → … → 最深叶子。
    fn nest(remaining: usize) -> Element<()> {
        if remaining == 0 {
            return Stack::new()
                .size(dp(24.0), dp(24.0))
                .child(Text::new("deep leaf"))
                .into();
        }
        Stack::new()
            .padding(Insets::all(dp(2.0)))
            .child(nest(remaining - 1))
            .into()
    }

    let chain: Element<()> = Stack::new()
        .opacity(opacity.signal())
        .child(nest(depth))
        .into();

    // 宽扁填充体：把节点规模摊到 node_count（每行一个简单卡片 ≈ 2 节点）。
    let filler_rows = node_count.saturating_sub(depth + 2) / 2;
    let mut root = Flex::new(Axis::Vertical)
        .width(dp(1280.0))
        .padding(Insets::all(dp(8.0)))
        .gap(dp(4.0))
        .child(chain);
    for row in 0..filler_rows {
        root = root.child(
            Stack::new()
                .width(dp(1200.0))
                .child(Text::new(format!("filler row {row}"))),
        );
    }

    (WidgetTree::new(root), opacity)
}

fn bench_single_property_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_property_update");

    for node_count in [1_000_usize, 10_000_usize] {
        let (tree, opacity) = build_deep_leaf_tree(node_count, 16);

        // single_leaf_patch：改深层叶子所在子树 → 子树 patch + 祖先链向上合成到根。
        group.bench_with_input(
            BenchmarkId::new("single_leaf_patch", node_count),
            &node_count,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                // 预热布局 + 首次 scene 收集。
                let _ = bench.run_layout_and_scene(&tree, Instant::now());
                let mut tick = 0u32;
                b.iter(|| {
                    // 每次改一点 opacity，确保确有属性变化（值本身不影响 patch 成本）。
                    tick = tick.wrapping_add(1);
                    opacity.set(0.5 + (tick % 2) as f32 * 0.5);
                    let patched = bench.patch_single_deep_leaf_scene(&tree, Instant::now());
                    black_box(patched)
                });
            },
        );

        // full_recollect：滚动/动画每帧的整树重收集（对照上界）。
        group.bench_with_input(
            BenchmarkId::new("full_recollect", node_count),
            &node_count,
            |b, _| {
                let mut bench = WidgetBenchmarkContext::default();
                let _ = bench.recollect_scene_only(&tree, Instant::now());
                b.iter(|| {
                    let stats = bench.recollect_scene_only(&tree, Instant::now());
                    black_box((stats.shape_count, stats.text_count))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(single_property_update_benches, bench_single_property_update);
criterion_main!(single_property_update_benches);
