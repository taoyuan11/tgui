//! UI-thread-owned virtualized list with stable item identity.
//!
//! [`VirtualList`] materializes only the viewport plus a pixel overscan band.
//! Estimated and measured heights are stored in a Fenwick tree, so offset
//! lookup and height replacement are logarithmic in the data-source length.

use crate::accessibility::{ActionKind, CollectionInfo, CollectionItemInfo, Role, Semantics};
use crate::core::{Error, ItemKey, Result, WidgetKey};
use crate::event::NamedKey;
use crate::state::UiThread;
use crate::widget::{BuildContext, WidgetNode};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

/// Immutable data and declaration builder used by a virtual list.
///
/// `item_key` must return a unique, stable key for every current index. The
/// list installs that key on the returned root [`WidgetNode`], so retained
/// state cannot accidentally follow a recycled visible index.
pub trait VirtualListDataSource {
    fn len(&self) -> usize;

    fn item_key(&self, index: usize) -> ItemKey;

    fn build_item(
        &self,
        index: usize,
        key: &ItemKey,
        context: &mut BuildContext,
    ) -> Result<WidgetNode>;

    /// Called whenever a materialized declaration leaves the overscan range or
    /// its key disappears. Per-item persistent state is deliberately retained
    /// for the former case and removed for the latter.
    fn item_destroyed(&self, _index: usize, _key: &ItemKey) {}

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Root declaration currently retained for one data-source item.
pub struct MaterializedItem {
    index: usize,
    key: ItemKey,
    offset: f32,
    height: f32,
    generation: u64,
    node: WidgetNode,
}

impl MaterializedItem {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn key(&self) -> &ItemKey {
        &self.key
    }

    pub const fn offset(&self) -> f32 {
        self.offset
    }

    pub const fn height(&self) -> f32 {
        self.height
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn node(&self) -> &WidgetNode {
        &self.node
    }
}

impl fmt::Debug for MaterializedItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializedItem")
            .field("index", &self.index)
            .field("key", &self.key)
            .field("offset", &self.offset)
            .field("height", &self.height)
            .field("generation", &self.generation)
            .field("node", &self.node)
            .finish()
    }
}

/// Change summary for one materialization pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterializationReport {
    pub range: Range<usize>,
    pub added: Vec<ItemKey>,
    pub removed: Vec<ItemKey>,
    pub retained: usize,
}

impl MaterializationReport {
    pub fn changed(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }

    pub fn materialized_count(&self) -> usize {
        self.range.end.saturating_sub(self.range.start)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    None,
    #[default]
    Single,
    Multiple,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAlignment {
    Start,
    Center,
    End,
    #[default]
    Nearest,
}

/// Collection-level accessibility information available without materializing
/// every item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionSemantics {
    pub item_count: usize,
    pub current_item: Option<ItemKey>,
    pub selected_count: usize,
}

impl CollectionSemantics {
    pub fn accessibility(&self) -> Semantics {
        Semantics::new(Role::List)
            .with_collection(CollectionInfo {
                item_count: self.item_count,
                selected_count: self.selected_count,
            })
            .with_action(ActionKind::ScrollIntoView)
    }
}

/// Positional accessibility information for one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSemantics {
    pub key: ItemKey,
    pub index: usize,
    /// One-based position in the collection.
    pub position_in_set: usize,
    pub set_size: usize,
    pub selected: bool,
    pub focused: bool,
    pub current: bool,
    pub materialized: bool,
}

impl ItemSemantics {
    pub fn accessibility(&self) -> Semantics {
        Semantics::new(Role::ListItem)
            .with_selected(self.selected)
            .with_focused(self.focused)
            .with_focusable(true)
            .with_collection_item(CollectionItemInfo {
                position_in_set: self.position_in_set,
                set_size: self.set_size,
                current: self.current,
            })
            .with_actions([ActionKind::Focus, ActionKind::ScrollIntoView])
    }
}

/// Generation-stamped request suitable for returning an asynchronous item
/// measurement to the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasurementTicket {
    key: ItemKey,
    materialization_generation: u64,
    data_revision: u64,
}

impl MeasurementTicket {
    pub const fn key(&self) -> &ItemKey {
        &self.key
    }

    pub const fn materialization_generation(&self) -> u64 {
        self.materialization_generation
    }

    pub const fn data_revision(&self) -> u64 {
        self.data_revision
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightUpdate {
    pub index: usize,
    pub old_height: f32,
    pub new_height: f32,
    pub scroll_adjustment: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasurementCompletion {
    Applied(HeightUpdate),
    Stale,
}

/// Bounded-materialization diagnostics for benchmarks and frame metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VirtualListMetrics {
    pub total_items: usize,
    pub materialized_items: usize,
    pub peak_materialized_items: usize,
}

impl From<VirtualListMetrics> for crate::diagnostics::VirtualizationMetrics {
    fn from(metrics: VirtualListMetrics) -> Self {
        Self {
            total_items: u64::try_from(metrics.total_items).unwrap_or(u64::MAX),
            materialized_items: u64::try_from(metrics.materialized_items).unwrap_or(u64::MAX),
            peak_materialized_items: u64::try_from(metrics.peak_materialized_items)
                .unwrap_or(u64::MAX),
        }
    }
}

#[derive(Debug)]
struct FenwickHeights {
    values: Vec<f32>,
    tree: Vec<f64>,
}

impl FenwickHeights {
    fn from_values(values: Vec<f32>) -> Self {
        let mut heights = Self {
            tree: vec![0.0; values.len() + 1],
            values: vec![0.0; values.len()],
        };
        for (index, value) in values.into_iter().enumerate() {
            heights.set(index, value);
        }
        heights
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn get(&self, index: usize) -> Option<f32> {
        self.values.get(index).copied()
    }

    fn set(&mut self, index: usize, value: f32) -> Option<f32> {
        let old = *self.values.get(index)?;
        self.values[index] = value;
        let delta = f64::from(value) - f64::from(old);
        let mut node = index + 1;
        while node < self.tree.len() {
            self.tree[node] += delta;
            node += node & node.wrapping_neg();
        }
        Some(old)
    }

    fn prefix_sum(&self, end: usize) -> f32 {
        let mut node = end.min(self.len());
        let mut sum = 0.0_f64;
        while node > 0 {
            sum += self.tree[node];
            node &= node - 1;
        }
        sum as f32
    }

    fn total(&self) -> f32 {
        self.prefix_sum(self.len())
    }

    /// Returns the largest element count whose prefix sum is <= `offset`.
    fn max_prefix_at_most(&self, offset: f32) -> usize {
        if self.is_empty() {
            return 0;
        }
        let target = f64::from(offset.max(0.0));
        let mut index = 0_usize;
        let mut sum = 0.0_f64;
        let mut bit = 1_usize << (usize::BITS - self.len().leading_zeros() - 1);
        while bit != 0 {
            let next = index + bit;
            if next <= self.len() && sum + self.tree[next] <= target {
                index = next;
                sum += self.tree[next];
            }
            bit >>= 1;
        }
        index
    }

    fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

struct StateRecord {
    value: Box<dyn Any>,
    cleanup: Vec<Box<dyn FnOnce()>>,
}

impl StateRecord {
    fn new(value: impl Any) -> Self {
        Self {
            value: Box::new(value),
            cleanup: Vec::new(),
        }
    }
}

impl Drop for StateRecord {
    fn drop(&mut self) {
        for cleanup in self.cleanup.drain(..).rev() {
            cleanup();
        }
    }
}

/// Stateful virtual-list controller. It is intentionally `!Send + !Sync` and
/// all mutating operations validate the owning UI thread.
pub struct VirtualList<D: VirtualListDataSource> {
    owner: UiThread,
    source: D,
    keys: Vec<ItemKey>,
    key_indices: BTreeMap<ItemKey, usize>,
    heights: FenwickHeights,
    measured_heights: BTreeMap<ItemKey, f32>,
    materialized: BTreeMap<usize, MaterializedItem>,
    item_state: BTreeMap<ItemKey, StateRecord>,
    selected: BTreeSet<ItemKey>,
    focused: Option<ItemKey>,
    selection_mode: SelectionMode,
    estimated_item_height: f32,
    viewport_height: f32,
    scroll_offset: f32,
    overscan: f32,
    next_materialization_generation: u64,
    data_revision: u64,
    peak_materialized_items: usize,
}

impl<D: VirtualListDataSource> VirtualList<D> {
    pub fn new(source: D, estimated_item_height: f32) -> Result<Self> {
        validate_height("estimated_item_height", estimated_item_height)?;
        let (keys, key_indices) = collect_keys(&source)?;
        let heights = FenwickHeights::from_values(vec![estimated_item_height; keys.len()]);
        Ok(Self {
            owner: UiThread::current(),
            source,
            keys,
            key_indices,
            heights,
            measured_heights: BTreeMap::new(),
            materialized: BTreeMap::new(),
            item_state: BTreeMap::new(),
            selected: BTreeSet::new(),
            focused: None,
            selection_mode: SelectionMode::Single,
            estimated_item_height,
            viewport_height: 0.0,
            scroll_offset: 0.0,
            overscan: estimated_item_height * 2.0,
            next_materialization_generation: 0,
            data_revision: 0,
            peak_materialized_items: 0,
        })
    }

    pub const fn source(&self) -> &D {
        &self.source
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub const fn estimated_item_height(&self) -> f32 {
        self.estimated_item_height
    }

    pub const fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    pub const fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    pub const fn overscan(&self) -> f32 {
        self.overscan
    }

    pub const fn data_revision(&self) -> u64 {
        self.data_revision
    }

    pub fn total_height(&self) -> f32 {
        self.heights.total()
    }

    pub fn item_height(&self, index: usize) -> Option<f32> {
        self.heights.get(index)
    }

    pub fn offset_of_index(&self, index: usize) -> Option<f32> {
        (index <= self.len()).then(|| self.heights.prefix_sum(index))
    }

    pub fn index_at_offset(&self, offset: f32) -> Option<usize> {
        if self.is_empty() || !offset.is_finite() {
            return None;
        }
        Some(self.heights.max_prefix_at_most(offset).min(self.len() - 1))
    }

    pub fn key_at(&self, index: usize) -> Option<&ItemKey> {
        self.keys.get(index)
    }

    pub fn index_of(&self, key: &ItemKey) -> Option<usize> {
        self.key_indices.get(key).copied()
    }

    pub fn materialized_range(&self) -> Range<usize> {
        self.target_range()
    }

    pub fn materialized_count(&self) -> usize {
        self.materialized.len()
    }

    pub fn metrics(&self) -> VirtualListMetrics {
        VirtualListMetrics {
            total_items: self.len(),
            materialized_items: self.materialized.len(),
            peak_materialized_items: self.peak_materialized_items,
        }
    }

    pub fn materialized_items(&self) -> impl DoubleEndedIterator<Item = &MaterializedItem> {
        self.materialized.values()
    }

    pub fn materialized_item(&self, key: &ItemKey) -> Option<&MaterializedItem> {
        let index = self.index_of(key)?;
        self.materialized
            .get(&index)
            .filter(|item| item.key() == key)
    }

    pub fn set_overscan(&mut self, overscan: f32) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        validate_non_negative("overscan", overscan)?;
        self.overscan = overscan;
        self.rematerialize()
    }

    pub fn set_viewport(
        &mut self,
        viewport_height: f32,
        scroll_offset: f32,
    ) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        validate_non_negative("viewport_height", viewport_height)?;
        validate_non_negative("scroll_offset", scroll_offset)?;
        self.viewport_height = viewport_height;
        self.scroll_offset = self.clamp_scroll(scroll_offset);
        self.rematerialize()
    }

    pub fn set_scroll_offset(&mut self, scroll_offset: f32) -> Result<MaterializationReport> {
        self.set_viewport(self.viewport_height, scroll_offset)
    }

    pub fn refresh(&mut self) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        let (keys, key_indices) = collect_keys(&self.source)?;
        let anchor = self.scroll_anchor();
        let current_keys = self.key_indices.keys().cloned().collect::<BTreeSet<_>>();
        let next_keys = key_indices.keys().cloned().collect::<BTreeSet<_>>();
        let removed = current_keys
            .difference(&next_keys)
            .cloned()
            .collect::<Vec<_>>();
        let values = keys
            .iter()
            .map(|key| {
                self.measured_heights
                    .get(key)
                    .copied()
                    .unwrap_or(self.estimated_item_height)
            })
            .collect();

        self.keys = keys;
        self.key_indices = key_indices;
        self.heights = FenwickHeights::from_values(values);
        self.data_revision = self.data_revision.checked_add(1).ok_or_else(|| {
            Error::resource(None, "virtual-list data revision exhausted u64", false)
        })?;
        for key in removed {
            self.measured_heights.remove(&key);
            self.item_state.remove(&key);
            self.selected.remove(&key);
            if self.focused.as_ref() == Some(&key) {
                self.focused = None;
            }
        }
        self.restore_scroll_anchor(anchor);
        self.rematerialize()
    }

    /// Mutates the source and immediately re-reads its key sequence.
    pub fn update_source(&mut self, update: impl FnOnce(&mut D)) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        update(&mut self.source);
        self.refresh()
    }

    pub fn report_item_height(&mut self, key: &ItemKey, height: f32) -> Result<HeightUpdate> {
        self.owner.assert_current()?;
        validate_height("item_height", height)?;
        let index = self.index_of(key).ok_or_else(|| missing_key(key))?;
        let old_height = self.heights.get(index).expect("validated item index");
        if old_height == height {
            return Ok(HeightUpdate {
                index,
                old_height,
                new_height: height,
                scroll_adjustment: 0.0,
            });
        }

        let anchor = self.scroll_anchor();
        let old_scroll = self.scroll_offset;
        self.heights.set(index, height);
        self.measured_heights.insert(key.clone(), height);
        self.restore_scroll_anchor(anchor);
        let scroll_adjustment = self.scroll_offset - old_scroll;
        self.rematerialize()?;
        Ok(HeightUpdate {
            index,
            old_height,
            new_height: height,
            scroll_adjustment,
        })
    }

    pub fn request_measurement(&self, key: &ItemKey) -> Option<MeasurementTicket> {
        let item = self.materialized_item(key)?;
        Some(MeasurementTicket {
            key: key.clone(),
            materialization_generation: item.generation,
            data_revision: self.data_revision,
        })
    }

    pub fn complete_measurement(
        &mut self,
        ticket: MeasurementTicket,
        height: f32,
    ) -> Result<MeasurementCompletion> {
        self.owner.assert_current()?;
        let current = self.materialized_item(&ticket.key);
        if ticket.data_revision != self.data_revision
            || current.is_none_or(|item| item.generation != ticket.materialization_generation)
        {
            return Ok(MeasurementCompletion::Stale);
        }
        self.report_item_height(&ticket.key, height)
            .map(MeasurementCompletion::Applied)
    }

    pub const fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    pub fn set_selection_mode(&mut self, mode: SelectionMode) -> Result<()> {
        self.owner.assert_current()?;
        self.selection_mode = mode;
        match mode {
            SelectionMode::None => self.selected.clear(),
            SelectionMode::Single if self.selected.len() > 1 => {
                let retained = self.selected.iter().next().cloned();
                self.selected.clear();
                self.selected.extend(retained);
            }
            SelectionMode::Single | SelectionMode::Multiple => {}
        }
        Ok(())
    }

    pub fn select(&mut self, key: &ItemKey, selected: bool) -> Result<bool> {
        self.owner.assert_current()?;
        if !self.key_indices.contains_key(key) {
            return Err(missing_key(key));
        }
        match (self.selection_mode, selected) {
            (SelectionMode::None, true) => Ok(false),
            (SelectionMode::Single, true) => {
                let changed = self.selected.len() != 1 || !self.selected.contains(key);
                self.selected.clear();
                self.selected.insert(key.clone());
                Ok(changed)
            }
            (_, true) => Ok(self.selected.insert(key.clone())),
            (_, false) => Ok(self.selected.remove(key)),
        }
    }

    pub fn is_selected(&self, key: &ItemKey) -> bool {
        self.selected.contains(key)
    }

    pub fn selected_keys(&self) -> impl Iterator<Item = &ItemKey> {
        self.selected.iter()
    }

    pub fn focused_key(&self) -> Option<&ItemKey> {
        self.focused.as_ref()
    }

    /// Scrolls and materializes an item before assigning logical list focus.
    pub fn focus_key(&mut self, key: &ItemKey) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        let report = self.scroll_to_key(key, ScrollAlignment::Nearest)?;
        if self.materialized_item(key).is_none() {
            return Err(Error::invalid_input(
                Some("viewport_height".to_owned()),
                "a positive viewport is required to focus an item",
            ));
        }
        self.focused = Some(key.clone());
        Ok(report)
    }

    pub fn clear_focus(&mut self) -> Result<()> {
        self.owner.assert_current()?;
        self.focused = None;
        Ok(())
    }

    pub fn focus_next(&mut self) -> Result<MaterializationReport> {
        let index = self
            .focused
            .as_ref()
            .and_then(|key| self.index_of(key))
            .map_or(0, |index| (index + 1).min(self.len().saturating_sub(1)));
        self.focus_index(index)
    }

    pub fn focus_previous(&mut self) -> Result<MaterializationReport> {
        let index = self
            .focused
            .as_ref()
            .and_then(|key| self.index_of(key))
            .unwrap_or(0)
            .saturating_sub(1);
        self.focus_index(index)
    }

    /// Handles conventional vertical-list keyboard navigation and selection.
    pub fn handle_key(&mut self, key: &NamedKey) -> Result<bool> {
        if self.is_empty() {
            return Ok(false);
        }
        match key {
            NamedKey::ArrowDown => self.focus_next().map(|_| true),
            NamedKey::ArrowUp => self.focus_previous().map(|_| true),
            NamedKey::Home => self.focus_index(0).map(|_| true),
            NamedKey::End => self.focus_index(self.len() - 1).map(|_| true),
            NamedKey::PageDown => {
                let offset =
                    (self.scroll_offset + self.viewport_height).min(self.maximum_scroll_offset());
                self.focus_index(self.index_at_offset(offset).unwrap_or(0))
                    .map(|_| true)
            }
            NamedKey::PageUp => {
                let offset = (self.scroll_offset - self.viewport_height).max(0.0);
                self.focus_index(self.index_at_offset(offset).unwrap_or(0))
                    .map(|_| true)
            }
            NamedKey::Enter | NamedKey::Space => {
                let Some(focused) = self.focused.clone() else {
                    return Ok(false);
                };
                let selected = !self.is_selected(&focused);
                self.select(&focused, selected)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn scroll_to_key(
        &mut self,
        key: &ItemKey,
        alignment: ScrollAlignment,
    ) -> Result<MaterializationReport> {
        self.owner.assert_current()?;
        let index = self.index_of(key).ok_or_else(|| missing_key(key))?;
        let start = self.heights.prefix_sum(index);
        let end = start + self.heights.get(index).expect("validated item index");
        let offset = match alignment {
            ScrollAlignment::Start => start,
            ScrollAlignment::Center => start - (self.viewport_height - (end - start)) * 0.5,
            ScrollAlignment::End => end - self.viewport_height,
            ScrollAlignment::Nearest if start < self.scroll_offset => start,
            ScrollAlignment::Nearest if end > self.scroll_offset + self.viewport_height => {
                end - self.viewport_height
            }
            ScrollAlignment::Nearest => self.scroll_offset,
        };
        self.scroll_offset = self.clamp_scroll(offset);
        self.rematerialize()
    }

    pub fn state_for<T: Any>(&mut self, key: &ItemKey, init: impl FnOnce() -> T) -> Result<&mut T> {
        self.owner.assert_current()?;
        if !self.key_indices.contains_key(key) {
            return Err(missing_key(key));
        }
        let record = self
            .item_state
            .entry(key.clone())
            .or_insert_with(|| StateRecord::new(init()));
        record.value.downcast_mut::<T>().ok_or_else(|| {
            Error::invalid_input(
                Some("item_state".to_owned()),
                "state type does not match the value already stored for this ItemKey",
            )
        })
    }

    pub fn item_state<T: Any>(&self, key: &ItemKey) -> Option<&T> {
        self.item_state.get(key)?.value.downcast_ref::<T>()
    }

    pub fn register_state_cleanup(
        &mut self,
        key: &ItemKey,
        cleanup: impl FnOnce() + 'static,
    ) -> Result<()> {
        self.owner.assert_current()?;
        let record = self.item_state.get_mut(key).ok_or_else(|| {
            Error::invalid_input(
                Some("item_state".to_owned()),
                "initialize item state before registering cleanup",
            )
        })?;
        record.cleanup.push(Box::new(cleanup));
        Ok(())
    }

    pub fn clear_item_state(&mut self, key: &ItemKey) -> Result<bool> {
        self.owner.assert_current()?;
        Ok(self.item_state.remove(key).is_some())
    }

    pub fn collection_semantics(&self) -> CollectionSemantics {
        CollectionSemantics {
            item_count: self.len(),
            current_item: self.current_key().cloned(),
            selected_count: self.selected.len(),
        }
    }

    pub fn item_semantics(&self, key: &ItemKey) -> Option<ItemSemantics> {
        let index = self.index_of(key)?;
        Some(ItemSemantics {
            key: key.clone(),
            index,
            position_in_set: index + 1,
            set_size: self.len(),
            selected: self.is_selected(key),
            focused: self.focused.as_ref() == Some(key),
            current: self.current_key() == Some(key),
            materialized: self.materialized_item(key).is_some(),
        })
    }

    pub fn materialized_semantics(&self) -> impl Iterator<Item = ItemSemantics> + '_ {
        self.materialized
            .values()
            .filter_map(|item| self.item_semantics(item.key()))
    }

    fn focus_index(&mut self, index: usize) -> Result<MaterializationReport> {
        let key =
            self.keys.get(index).cloned().ok_or_else(|| {
                Error::invalid_input(Some("focus_index".to_owned()), "list is empty")
            })?;
        self.focus_key(&key)
    }

    fn current_key(&self) -> Option<&ItemKey> {
        self.focused
            .as_ref()
            .or_else(|| self.selected.iter().next())
    }

    fn maximum_scroll_offset(&self) -> f32 {
        (self.total_height() - self.viewport_height).max(0.0)
    }

    fn clamp_scroll(&self, offset: f32) -> f32 {
        offset.max(0.0).min(self.maximum_scroll_offset())
    }

    fn target_range(&self) -> Range<usize> {
        if self.is_empty() || self.viewport_height <= 0.0 {
            return 0..0;
        }
        let start_offset = (self.scroll_offset - self.overscan).max(0.0);
        let end_offset =
            (self.scroll_offset + self.viewport_height + self.overscan).min(self.total_height());
        let start = self
            .heights
            .max_prefix_at_most(start_offset)
            .min(self.len() - 1);
        let boundary = self.heights.max_prefix_at_most(end_offset);
        let end = if boundary >= self.len() {
            self.len()
        } else if self.heights.prefix_sum(boundary) < end_offset {
            boundary + 1
        } else {
            boundary
        };
        start..end.max(start)
    }

    fn scroll_anchor(&self) -> Option<(ItemKey, f32)> {
        let index = self.index_at_offset(self.scroll_offset)?;
        let key = self.keys.get(index)?.clone();
        let intra_item_offset = self.scroll_offset - self.heights.prefix_sum(index);
        Some((key, intra_item_offset))
    }

    fn restore_scroll_anchor(&mut self, anchor: Option<(ItemKey, f32)>) {
        if let Some((key, intra_item_offset)) = anchor {
            if let Some(index) = self.index_of(&key) {
                self.scroll_offset =
                    self.clamp_scroll(self.heights.prefix_sum(index) + intra_item_offset);
                return;
            }
        }
        self.scroll_offset = self.clamp_scroll(self.scroll_offset);
    }

    fn rematerialize(&mut self) -> Result<MaterializationReport> {
        let range = self.target_range();
        let target_keys = range
            .clone()
            .map(|index| self.keys[index].clone())
            .collect::<BTreeSet<_>>();
        let old_keys = self
            .materialized
            .values()
            .map(|item| item.key.clone())
            .collect::<BTreeSet<_>>();
        let added = target_keys
            .difference(&old_keys)
            .cloned()
            .collect::<Vec<_>>();
        let removed = old_keys
            .difference(&target_keys)
            .cloned()
            .collect::<Vec<_>>();

        // Build every new declaration first. A builder error leaves the
        // previously committed materialized set untouched.
        let mut built = BTreeMap::new();
        let mut context = BuildContext::new();
        for key in &added {
            let index = self.key_indices[key];
            let node = self
                .source
                .build_item(index, key, &mut context)?
                .with_optional_key(Some(WidgetKey::from(key)));
            built.insert(key.clone(), node);
        }

        let mut old_by_key = std::mem::take(&mut self.materialized)
            .into_values()
            .map(|item| (item.key.clone(), item))
            .collect::<BTreeMap<_, _>>();
        for key in &removed {
            if let Some(item) = old_by_key.remove(key) {
                self.source.item_destroyed(item.index, key);
            }
        }

        let mut next = BTreeMap::new();
        for index in range.clone() {
            let key = self.keys[index].clone();
            let offset = self.heights.prefix_sum(index);
            let height = self.heights.get(index).expect("materialized index");
            let item = if let Some(mut item) = old_by_key.remove(&key) {
                item.index = index;
                item.offset = offset;
                item.height = height;
                item
            } else {
                self.next_materialization_generation = self
                    .next_materialization_generation
                    .checked_add(1)
                    .ok_or_else(|| {
                        Error::resource(
                            None,
                            "virtual-list materialization generation exhausted u64",
                            false,
                        )
                    })?;
                MaterializedItem {
                    index,
                    key: key.clone(),
                    offset,
                    height,
                    generation: self.next_materialization_generation,
                    node: built.remove(&key).expect("new item was built"),
                }
            };
            next.insert(index, item);
        }
        self.materialized = next;
        self.peak_materialized_items = self.peak_materialized_items.max(self.materialized.len());
        Ok(MaterializationReport {
            range,
            added,
            removed,
            retained: target_keys.intersection(&old_keys).count(),
        })
    }
}

impl<D: VirtualListDataSource> Drop for VirtualList<D> {
    fn drop(&mut self) {
        for item in self.materialized.values() {
            self.source.item_destroyed(item.index, &item.key);
        }
    }
}

fn collect_keys<D: VirtualListDataSource>(
    source: &D,
) -> Result<(Vec<ItemKey>, BTreeMap<ItemKey, usize>)> {
    let mut keys = Vec::with_capacity(source.len());
    let mut indices = BTreeMap::new();
    for index in 0..source.len() {
        let key = source.item_key(index);
        if let Some(first) = indices.insert(key.clone(), index) {
            return Err(Error::invalid_input(
                Some("item_key".to_owned()),
                format!("duplicate ItemKey at indices {first} and {index}: {key:?}"),
            ));
        }
        keys.push(key);
    }
    Ok((keys, indices))
}

fn validate_height(field: &str, height: f32) -> Result<()> {
    if height.is_finite() && height > 0.0 {
        Ok(())
    } else {
        Err(Error::invalid_input(
            Some(field.to_owned()),
            "must be finite and positive",
        ))
    }
}

fn validate_non_negative(field: &str, value: f32) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::invalid_input(
            Some(field.to_owned()),
            "must be finite and non-negative",
        ))
    }
}

fn missing_key(key: &ItemKey) -> Error {
    Error::invalid_input(
        Some("item_key".to_owned()),
        format!("ItemKey is not present in the data source: {key:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Row;

    struct Source(usize);

    impl VirtualListDataSource for Source {
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
        ) -> Result<WidgetNode> {
            Ok(WidgetNode::new::<Row>())
        }
    }

    #[test]
    fn fenwick_lookup_observes_exact_boundaries() {
        let heights = FenwickHeights::from_values(vec![10.0, 20.0, 30.0]);
        assert_eq!(heights.prefix_sum(2), 30.0);
        assert_eq!(heights.max_prefix_at_most(0.0), 0);
        assert_eq!(heights.max_prefix_at_most(10.0), 1);
        assert_eq!(heights.max_prefix_at_most(29.0), 1);
        assert_eq!(heights.max_prefix_at_most(30.0), 2);
        assert_eq!(heights.max_prefix_at_most(100.0), 3);
    }

    #[test]
    fn materialization_is_bounded_by_viewport_and_overscan() {
        let mut list = VirtualList::new(Source(100_000), 20.0).unwrap();
        list.set_overscan(40.0).unwrap();
        list.set_viewport(200.0, 1_000_000.0).unwrap();
        assert!(list.materialized_count() <= 14);
    }
}
