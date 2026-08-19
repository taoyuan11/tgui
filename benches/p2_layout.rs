use std::time::Instant;
use tgui::core::{DpiScale, Size};
use tgui::layout::{Dimension, FlexWrap, LayoutStyle};
use tgui::test_support::LayoutHarness;
use tgui::widget::WidgetNode;

struct Root;
struct Leaf;

fn declaration(count: usize, changed: Option<usize>) -> WidgetNode {
    assert!(count > 0, "the layout benchmark needs a root node");
    let mut root_style =
        LayoutStyle::default().with_size(Dimension::Length(1_000.0), Dimension::Length(1_000.0));
    root_style.flex_wrap = FlexWrap::Wrap;
    let children = (0..count - 1).map(|index| {
        let width = if changed == Some(index) { 12.0 } else { 10.0 };
        WidgetNode::new::<Leaf>()
            .with_key(index.to_string())
            .with_layout_style(
                LayoutStyle::default().with_size(Dimension::Length(width), Dimension::Length(10.0)),
            )
    });
    WidgetNode::new::<Root>()
        .with_layout_style(root_style)
        .with_children(children)
}

fn main() {
    for count in [10, 100, 1_000] {
        let mut harness = LayoutHarness::new();
        harness.mount(declaration(count, None)).unwrap();
        let initial_started = Instant::now();
        harness
            .layout(Size::new(1_000.0, 1_000.0), DpiScale::ONE)
            .unwrap();
        let initial = initial_started.elapsed();

        harness
            .reconcile(declaration(count, Some(count - 2)))
            .unwrap();
        let comparison_started = Instant::now();
        let comparison = harness
            .layout_and_compare(Size::new(1_000.0, 1_000.0), DpiScale::ONE)
            .unwrap();
        let comparison_time = comparison_started.elapsed();
        assert!(!comparison.incremental_report.full_rebuild);
        assert!(comparison.rebuilt_report.full_rebuild);
        println!(
            "nodes={count:>4} initial={initial:?} incremental_plus_full={comparison_time:?} revision={}",
            comparison.incremental.revision().get(),
        );
    }
}
