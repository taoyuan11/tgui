use std::cell::Cell;
use std::hint::black_box;
use std::time::{Duration, Instant};

use tgui::core::{ElementId, PropertyId, WidgetKey};
use tgui::test_support::WidgetHarness;
use tgui::widget::{BuildContext, View, WidgetNode};
use tgui::{Result, State, UpdateTxn};

const NODE_COUNTS: [usize; 3] = [10, 100, 1_000];
const IDLE_POLLS: usize = 10_000;
const REVISION_PROPERTY: PropertyId = PropertyId::new(1);

struct BenchmarkRoot;
struct BenchmarkLeaf;

struct BenchmarkView {
    node_count: usize,
    revision: State<u64>,
    reversed: Cell<bool>,
}

impl BenchmarkView {
    fn new(node_count: usize) -> Self {
        assert!(
            node_count >= 2,
            "a view needs its synthetic and declared roots"
        );
        Self {
            node_count,
            revision: State::new(0),
            reversed: Cell::new(false),
        }
    }

    fn keyed_node_count(&self) -> usize {
        // WidgetHarness::rebuild_view retains one synthetic View root in
        // addition to the root returned by this View.
        self.node_count - 2
    }
}

impl View for BenchmarkView {
    fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode> {
        let revision = context.read_state(&self.revision)?;
        let keyed_node_count = self.keyed_node_count();
        let reversed = self.reversed.get();
        let children = (0..keyed_node_count).map(|offset| {
            let index = if reversed {
                keyed_node_count - offset - 1
            } else {
                offset
            };
            WidgetNode::new::<BenchmarkLeaf>()
                .with_key(index as u64)
                // A transaction changes exactly one declaration property.
                .with_property(
                    REVISION_PROPERTY,
                    if index == 0 { revision } else { 0 },
                )
        });

        Ok(WidgetNode::new::<BenchmarkRoot>().with_children(children))
    }
}

#[derive(Debug)]
struct BenchmarkResult {
    node_count: usize,
    keyed_node_count: usize,
    initial_build: Duration,
    idle_polling: Duration,
    state_update: Duration,
    keyed_reorder: Duration,
    moved_nodes: usize,
    preserved_ids: usize,
}

fn declared_root(harness: &WidgetHarness) -> ElementId {
    let view_root = harness.root().expect("the benchmark view must be mounted");
    let children = harness.children(view_root);
    assert_eq!(children.len(), 1, "the synthetic View root has one child");
    children[0]
}

fn keyed_ids(
    harness: &WidgetHarness,
    parent: ElementId,
    keyed_node_count: usize,
) -> Vec<(WidgetKey, ElementId)> {
    (0..keyed_node_count)
        .map(|index| {
            let key = WidgetKey::numeric(index as u64);
            let id = harness
                .child_for_key(parent, &key)
                .unwrap_or_else(|| panic!("missing retained node for key {index}"));
            (key, id)
        })
        .collect()
}

fn benchmark(node_count: usize) -> Result<BenchmarkResult> {
    let view = BenchmarkView::new(node_count);
    let mut harness = WidgetHarness::with_capacity(node_count);

    let started = Instant::now();
    let initial_report = harness.rebuild_view(&view)?;
    let initial_build = started.elapsed();
    assert_eq!(initial_report.mounted, node_count);
    assert_eq!(harness.len(), node_count);

    // The initial request is consumed once. An inactive tree must not schedule
    // more work, even when the event loop polls it repeatedly.
    assert!(harness.take_scheduled_work());
    let started = Instant::now();
    let idle_work_items = (0..IDLE_POLLS)
        .filter(|_| black_box(harness.take_scheduled_work()))
        .count();
    let idle_polling = started.elapsed();
    assert_eq!(idle_work_items, 0, "idle polling scheduled unexpected work");

    let original_root = declared_root(&harness);
    let before = keyed_ids(&harness, original_root, view.keyed_node_count());

    // Include both the atomic state publication and the dependent declaration
    // rebuild in the end-to-end single-state-update measurement.
    let started = Instant::now();
    let mut transaction = UpdateTxn::<()>::new();
    view.revision.set(&mut transaction, 1)?;
    let receipt = transaction.commit(|commands| {
        assert!(commands.is_empty());
        Ok(())
    })?;
    let update_report = harness.rebuild_view(&view)?;
    let state_update = started.elapsed();
    assert_eq!(receipt.state_write_count, 1);
    assert_eq!(receipt.changed_state_count, 1);
    assert_eq!(receipt.invalidations().len(), 1);
    assert_eq!(update_report.updated, 1);
    assert_eq!(update_report.mounted, 0);
    assert_eq!(update_report.unmounted, 0);
    assert!(harness.take_scheduled_work());

    view.reversed.set(true);
    let started = Instant::now();
    let reorder_report = harness.rebuild_view(&view)?;
    let keyed_reorder = started.elapsed();
    assert_eq!(reorder_report.mounted, 0);
    assert_eq!(reorder_report.unmounted, 0);
    assert_eq!(reorder_report.replaced, 0);
    assert!(reorder_report.moved > 0);

    let reordered_root = declared_root(&harness);
    assert_eq!(original_root, reordered_root, "declared root ID changed");
    let after = keyed_ids(&harness, reordered_root, view.keyed_node_count());
    assert_eq!(before, after, "keyed reorder changed retained ElementIds");
    assert!(harness.take_scheduled_work());
    assert!(!harness.take_scheduled_work());

    Ok(BenchmarkResult {
        node_count,
        keyed_node_count: view.keyed_node_count(),
        initial_build,
        idle_polling,
        state_update,
        keyed_reorder,
        moved_nodes: reorder_report.moved,
        preserved_ids: after.len(),
    })
}

fn main() -> Result<()> {
    for node_count in NODE_COUNTS {
        let result = benchmark(node_count)?;
        black_box(&result);
        println!(
            "nodes={} keyed_nodes={} initial_build={:?} idle_polling={:?} idle_polls={} idle_work=0 state_update={:?} state_writes=1 keyed_reorder={:?} moved={} preserved_ids={}",
            result.node_count,
            result.keyed_node_count,
            result.initial_build,
            result.idle_polling,
            IDLE_POLLS,
            result.state_update,
            result.keyed_reorder,
            result.moved_nodes,
            result.preserved_ids,
        );
    }
    Ok(())
}
