use std::cell::Cell;
use std::rc::Rc;
use tgui::Result;
use tgui::core::{ItemKey, WidgetKey};
use tgui::event::NamedKey;
use tgui::virtualization::{
    MeasurementCompletion, ScrollAlignment, SelectionMode, VirtualList, VirtualListDataSource,
};
use tgui::widget::{BuildContext, WidgetNode};

struct Row;

struct Rows {
    keys: Vec<u64>,
    builds: Rc<Cell<usize>>,
    destroys: Rc<Cell<usize>>,
}

impl Rows {
    fn sequential(len: usize) -> Self {
        Self {
            keys: (0..len as u64).collect(),
            builds: Rc::new(Cell::new(0)),
            destroys: Rc::new(Cell::new(0)),
        }
    }
}

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.keys.len()
    }

    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(self.keys[index])
    }

    fn build_item(
        &self,
        _index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> Result<WidgetNode> {
        self.builds.set(self.builds.get() + 1);
        // VirtualList must replace a builder-supplied positional identity.
        Ok(WidgetNode::new::<Row>().with_key("wrong-builder-key"))
    }

    fn item_destroyed(&self, _index: usize, _key: &ItemKey) {
        self.destroys.set(self.destroys.get() + 1);
    }
}

#[test]
fn fifty_and_one_hundred_thousand_items_keep_materialization_bounded() {
    for len in [50_000, 100_000] {
        let rows = Rows::sequential(len);
        let builds = rows.builds.clone();
        let mut list = VirtualList::new(rows, 20.0).unwrap();
        list.set_overscan(40.0).unwrap();

        list.set_viewport(200.0, 0.0).unwrap();
        assert!(list.materialized_count() <= 12);
        list.set_scroll_offset(730_000.0).unwrap();
        assert!(list.materialized_count() <= 14);
        list.scroll_to_key(&ItemKey::numeric((len - 1) as u64), ScrollAlignment::End)
            .unwrap();

        let metrics = list.metrics();
        assert_eq!(metrics.total_items, len);
        assert!(metrics.materialized_items <= 12);
        assert!(metrics.peak_materialized_items <= 14);
        assert!(
            builds.get() <= 40,
            "built {} rows for {len} items",
            builds.get()
        );
        for item in list.materialized_items() {
            assert_eq!(
                item.node().key(),
                Some(&WidgetKey::numeric(item.key().as_numeric().unwrap()))
            );
        }
    }
}

#[test]
fn fenwick_height_updates_preserve_the_first_visible_item_anchor() {
    let mut list = VirtualList::new(Rows::sequential(2_000), 20.0).unwrap();
    list.set_overscan(0.0).unwrap();
    list.set_viewport(100.0, 1_007.0).unwrap();
    let anchored = list
        .key_at(list.index_at_offset(list.scroll_offset()).unwrap())
        .unwrap()
        .clone();
    assert_eq!(anchored, ItemKey::numeric(50));

    let update = list
        .report_item_height(&ItemKey::numeric(10), 65.0)
        .unwrap();
    assert_eq!(update.scroll_adjustment, 45.0);
    assert_eq!(list.scroll_offset(), 1_052.0);
    assert_eq!(list.index_at_offset(list.scroll_offset()), Some(50));
    assert_eq!(
        list.scroll_offset() - list.offset_of_index(50).unwrap(),
        7.0
    );
    assert_eq!(list.offset_of_index(11), Some(265.0));
}

#[test]
fn state_focus_and_selection_follow_item_keys_across_scroll_and_reorder() {
    let rows = Rows::sequential(200);
    let destroys = rows.destroys.clone();
    let mut list = VirtualList::new(rows, 20.0).unwrap();
    list.set_overscan(0.0).unwrap();
    list.set_viewport(100.0, 0.0).unwrap();
    list.set_selection_mode(SelectionMode::Multiple).unwrap();

    let key = ItemKey::numeric(3);
    *list.state_for(&key, || 7_u32).unwrap() = 41;
    list.select(&key, true).unwrap();
    list.focus_key(&key).unwrap();
    list.set_scroll_offset(2_000.0).unwrap();

    assert_eq!(list.item_state::<u32>(&key), Some(&41));
    assert_eq!(list.focused_key(), Some(&key));
    assert!(list.is_selected(&key));
    assert!(list.materialized_item(&key).is_none());
    assert!(destroys.get() > 0);

    list.update_source(|rows| rows.keys.reverse()).unwrap();
    assert_eq!(list.index_of(&key), Some(196));
    assert_eq!(list.item_state::<u32>(&key), Some(&41));
    assert_eq!(list.focused_key(), Some(&key));
    list.focus_key(&key).unwrap();
    assert!(list.materialized_item(&key).is_some());
}

#[test]
fn focus_materializes_an_unseen_item_and_semantics_cover_the_collection() {
    let mut list = VirtualList::new(Rows::sequential(1_000), 18.0).unwrap();
    list.set_viewport(90.0, 0.0).unwrap();
    let target = ItemKey::numeric(900);

    list.focus_key(&target).unwrap();
    list.select(&target, true).unwrap();
    assert!(list.materialized_item(&target).is_some());
    assert_eq!(list.focused_key(), Some(&target));

    let collection = list.collection_semantics();
    assert_eq!(collection.item_count, 1_000);
    assert_eq!(collection.current_item, Some(target.clone()));
    assert_eq!(collection.selected_count, 1);
    let item = list.item_semantics(&target).unwrap();
    assert_eq!(item.position_in_set, 901);
    assert_eq!(item.set_size, 1_000);
    assert!(item.current && item.focused && item.selected && item.materialized);

    assert!(list.handle_key(&NamedKey::ArrowDown).unwrap());
    assert_eq!(list.focused_key(), Some(&ItemKey::numeric(901)));
}

#[test]
fn asynchronous_measurements_reject_reused_and_revised_materializations() {
    let mut list = VirtualList::new(Rows::sequential(100), 20.0).unwrap();
    list.set_overscan(0.0).unwrap();
    list.set_viewport(100.0, 0.0).unwrap();
    let key = ItemKey::numeric(2);
    let old = list.request_measurement(&key).unwrap();

    list.set_scroll_offset(1_000.0).unwrap();
    list.focus_key(&key).unwrap();
    assert_eq!(
        list.complete_measurement(old, 33.0).unwrap(),
        MeasurementCompletion::Stale
    );

    let before_refresh = list.request_measurement(&key).unwrap();
    list.refresh().unwrap();
    assert_eq!(
        list.complete_measurement(before_refresh, 33.0).unwrap(),
        MeasurementCompletion::Stale
    );
    let current = list.request_measurement(&key).unwrap();
    assert!(matches!(
        list.complete_measurement(current, 33.0).unwrap(),
        MeasurementCompletion::Applied(_)
    ));
    assert_eq!(list.item_height(2), Some(33.0));
}

#[test]
fn removing_a_key_runs_state_cleanup_and_duplicate_keys_are_rejected() {
    let cleanup_count = Rc::new(Cell::new(0));
    let mut list = VirtualList::new(Rows::sequential(10), 20.0).unwrap();
    let removed = ItemKey::numeric(4);
    list.state_for(&removed, || "state".to_owned()).unwrap();
    list.register_state_cleanup(&removed, {
        let cleanup_count = cleanup_count.clone();
        move || cleanup_count.set(cleanup_count.get() + 1)
    })
    .unwrap();
    list.update_source(|rows| rows.keys.retain(|key| *key != 4))
        .unwrap();
    assert_eq!(cleanup_count.get(), 1);
    assert!(list.item_state::<String>(&removed).is_none());

    let duplicate = Rows {
        keys: vec![1, 1],
        builds: Rc::new(Cell::new(0)),
        destroys: Rc::new(Cell::new(0)),
    };
    assert!(VirtualList::new(duplicate, 20.0).is_err());
}
