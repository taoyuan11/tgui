use std::time::Instant;

use tgui::core::ItemKey;
use tgui::virtualization::{VirtualList, VirtualListDataSource};
use tgui::widget::{BuildContext, WidgetNode};

struct Rows(usize);
struct Row;

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.0
    }
    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(index as u64)
    }
    fn build_item(
        &self,
        _index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Ok(WidgetNode::new::<Row>())
    }
}

fn main() {
    for count in [50_000, 100_000] {
        let started = Instant::now();
        let mut list = VirtualList::new(Rows(count), 20.0).unwrap();
        list.set_viewport(240.0, 0.0).unwrap();
        for offset in [0.0, 10_000.0, 500_000.0, 1_000_000.0] {
            list.set_scroll_offset(offset).unwrap();
            assert!(list.materialized_count() <= 30);
        }
        let metrics = list.metrics();
        assert!(metrics.peak_materialized_items <= 30);
        println!(
            "items={count} materialized={} peak={} elapsed={:?}",
            metrics.materialized_items,
            metrics.peak_materialized_items,
            started.elapsed()
        );
    }
}
