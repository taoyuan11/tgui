use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Range;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, Insets, Overflow, Value};
use crate::ui::unit::{dp, Dp};

use super::background::{BackgroundBrush, BackgroundImage};
use super::common::{
    CursorStyle, FocusScopeOptions, InteractionHandlers, LifecycleEventHandlers,
    MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::{StyleResolver, StyleSheet};
use super::ContainerStyle;

pub trait ItemSource<T>: Send + Sync + 'static {
    fn len(&self) -> usize;
    fn item(&self, index: usize) -> Option<T>;

    fn key(&self, _index: usize) -> Option<WidgetKey> {
        None
    }

    /// Monotonic revision for sources that mutate in place.
    ///
    /// Measured virtual layouts cache item extents by index. Sources backed by
    /// interior mutability should advance this revision whenever insertion,
    /// removal, reordering, or item content can invalidate those measurements.
    fn revision(&self) -> u64 {
        0
    }
}

impl<T> ItemSource<T> for Vec<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn item(&self, index: usize) -> Option<T> {
        self.get(index).cloned()
    }
}

impl<T> ItemSource<T> for Arc<[T]>
where
    T: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn item(&self, index: usize) -> Option<T> {
        self.as_ref().get(index).cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualArrangement {
    Linear(VirtualDirection),
    Grid {
        direction: VirtualDirection,
        lanes: usize,
    },
}

impl VirtualArrangement {
    pub(crate) fn direction(self) -> VirtualDirection {
        match self {
            Self::Linear(direction) => direction,
            Self::Grid { direction, .. } => direction,
        }
    }

    pub(crate) fn lanes(self) -> usize {
        match self {
            Self::Linear(_) => 1,
            Self::Grid { lanes, .. } => lanes.max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemLayout {
    Fixed {
        item_extent: Dp,
        spacing: Dp,
        overscan: usize,
    },
    Estimated {
        estimate: Dp,
        spacing: Dp,
        overscan: usize,
    },
    Measured {
        estimate: Dp,
        spacing: Dp,
        overscan: usize,
    },
}

impl ItemLayout {
    pub(crate) fn estimate(self) -> Dp {
        match self {
            Self::Fixed { item_extent, .. } => item_extent,
            Self::Estimated { estimate, .. } => estimate,
            Self::Measured { estimate, .. } => estimate,
        }
    }

    pub(crate) fn spacing(self) -> Dp {
        match self {
            Self::Fixed { spacing, .. } => spacing,
            Self::Estimated { spacing, .. } => spacing,
            Self::Measured { spacing, .. } => spacing,
        }
    }

    pub(crate) fn overscan(self) -> usize {
        match self {
            Self::Fixed { overscan, .. } => overscan,
            Self::Estimated { overscan, .. } => overscan,
            Self::Measured { overscan, .. } => overscan,
        }
    }

    pub(crate) fn is_measured(self) -> bool {
        matches!(self, Self::Measured { .. })
    }

    pub(crate) fn with_estimate(self, extent: Dp) -> Self {
        match self {
            Self::Fixed {
                spacing, overscan, ..
            } => Self::Fixed {
                item_extent: extent,
                spacing,
                overscan,
            },
            Self::Estimated {
                spacing, overscan, ..
            } => Self::Estimated {
                estimate: extent,
                spacing,
                overscan,
            },
            Self::Measured {
                spacing, overscan, ..
            } => Self::Measured {
                estimate: extent,
                spacing,
                overscan,
            },
        }
    }

    fn with_spacing(self, spacing: Dp) -> Self {
        match self {
            Self::Fixed {
                item_extent,
                overscan,
                ..
            } => Self::Fixed {
                item_extent,
                spacing,
                overscan,
            },
            Self::Estimated {
                estimate, overscan, ..
            } => Self::Estimated {
                estimate,
                spacing,
                overscan,
            },
            Self::Measured {
                estimate, overscan, ..
            } => Self::Measured {
                estimate,
                spacing,
                overscan,
            },
        }
    }

    fn with_overscan(self, overscan: usize) -> Self {
        match self {
            Self::Fixed {
                item_extent,
                spacing,
                ..
            } => Self::Fixed {
                item_extent,
                spacing,
                overscan,
            },
            Self::Estimated {
                estimate, spacing, ..
            } => Self::Estimated {
                estimate,
                spacing,
                overscan,
            },
            Self::Measured {
                estimate, spacing, ..
            } => Self::Measured {
                estimate,
                spacing,
                overscan,
            },
        }
    }
}

pub(crate) const MEASURED_EXTENT_INVALIDATION_EPSILON: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MeasuredVirtualSignature {
    source_revision: u64,
    total_items: usize,
    lanes: usize,
    estimate_bits: u32,
    viewport_cross_bits: u32,
}

impl MeasuredVirtualSignature {
    fn new(
        source_revision: u64,
        total_items: usize,
        lanes: usize,
        estimate: Dp,
        viewport_cross: Dp,
    ) -> Self {
        Self {
            source_revision,
            total_items,
            lanes: lanes.max(1),
            estimate_bits: estimate.max(Dp::ZERO).get().to_bits(),
            viewport_cross_bits: viewport_cross.max(Dp::ZERO).get().to_bits(),
        }
    }

    fn estimate(self) -> Dp {
        Dp::new(f32::from_bits(self.estimate_bits))
    }

    fn stripe_count(self) -> usize {
        self.total_items.div_ceil(self.lanes)
    }
}

#[derive(Default)]
struct SparseCorrectionNode {
    correction_sum: f32,
    left: Option<Box<Self>>,
    right: Option<Box<Self>>,
}

#[derive(Default)]
struct SparseCorrectionTree {
    root: Option<Box<SparseCorrectionNode>>,
}

impl SparseCorrectionTree {
    fn correction_sum(&self) -> f32 {
        self.root
            .as_ref()
            .map(|root| root.correction_sum)
            .unwrap_or(0.0)
    }

    fn set(&mut self, len: usize, index: usize, correction: f32) {
        if len == 0 || index >= len {
            return;
        }
        Self::set_node(&mut self.root, 0, len, index, correction);
    }

    fn set_node(
        node: &mut Option<Box<SparseCorrectionNode>>,
        start: usize,
        end: usize,
        index: usize,
        correction: f32,
    ) {
        if end - start == 1 {
            if correction == 0.0 {
                *node = None;
            } else {
                *node = Some(Box::new(SparseCorrectionNode {
                    correction_sum: correction,
                    left: None,
                    right: None,
                }));
            }
            return;
        }

        let current = node.get_or_insert_with(Default::default);
        let middle = start + (end - start) / 2;
        if index < middle {
            Self::set_node(&mut current.left, start, middle, index, correction);
        } else {
            Self::set_node(&mut current.right, middle, end, index, correction);
        }
        current.correction_sum = current
            .left
            .as_ref()
            .map(|child| child.correction_sum)
            .unwrap_or(0.0)
            + current
                .right
                .as_ref()
                .map(|child| child.correction_sum)
                .unwrap_or(0.0);
        if current.left.is_none() && current.right.is_none() {
            *node = None;
        }
    }

    fn prefix_sum(&self, len: usize, end: usize) -> f32 {
        Self::prefix_sum_node(self.root.as_deref(), 0, len, end.min(len))
    }

    fn prefix_sum_node(
        node: Option<&SparseCorrectionNode>,
        start: usize,
        end: usize,
        query_end: usize,
    ) -> f32 {
        if query_end <= start || node.is_none() {
            return 0.0;
        }
        let node = node.expect("checked above");
        if end <= query_end {
            return node.correction_sum;
        }
        let middle = start + (end - start) / 2;
        Self::prefix_sum_node(node.left.as_deref(), start, middle, query_end)
            + Self::prefix_sum_node(node.right.as_deref(), middle, end, query_end)
    }

    fn first_prefix_matching(
        &self,
        len: usize,
        base_step: f32,
        target: f32,
        inclusive: bool,
    ) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let total = (base_step * len as f32 + self.correction_sum()).max(0.0);
        let matches = if inclusive {
            total >= target
        } else {
            total > target
        };
        if !matches {
            return None;
        }
        Some(Self::first_prefix_matching_node(
            self.root.as_deref(),
            0,
            len,
            base_step,
            target,
            inclusive,
            0.0,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn first_prefix_matching_node(
        node: Option<&SparseCorrectionNode>,
        start: usize,
        end: usize,
        base_step: f32,
        target: f32,
        inclusive: bool,
        prefix_before: f32,
    ) -> usize {
        if end - start == 1 {
            return start;
        }
        let middle = start + (end - start) / 2;
        let left_correction = node
            .and_then(|node| node.left.as_deref())
            .map(|left| left.correction_sum)
            .unwrap_or(0.0);
        let left_end =
            prefix_before + (base_step * (middle - start) as f32 + left_correction).max(0.0);
        let left_matches = if inclusive {
            left_end >= target
        } else {
            left_end > target
        };
        if left_matches {
            Self::first_prefix_matching_node(
                node.and_then(|node| node.left.as_deref()),
                start,
                middle,
                base_step,
                target,
                inclusive,
                prefix_before,
            )
        } else {
            Self::first_prefix_matching_node(
                node.and_then(|node| node.right.as_deref()),
                middle,
                end,
                base_step,
                target,
                inclusive,
                left_end,
            )
        }
    }
}

#[derive(Default)]
struct MeasuredVirtualIndex {
    signature: Option<MeasuredVirtualSignature>,
    measured_extents: HashMap<usize, Dp>,
    stripe_corrections: HashMap<usize, Dp>,
    corrections: SparseCorrectionTree,
    revision: u64,
}

impl MeasuredVirtualIndex {
    fn prepare(&mut self, signature: MeasuredVirtualSignature) {
        if self.signature == Some(signature) {
            return;
        }
        self.signature = Some(signature);
        self.measured_extents.clear();
        self.stripe_corrections.clear();
        self.corrections = SparseCorrectionTree::default();
        self.revision = self.revision.wrapping_add(1);
    }

    fn stripe_extent(&self, stripe_index: usize) -> Dp {
        let Some(signature) = self.signature else {
            return Dp::ZERO;
        };
        signature.estimate()
            + self
                .stripe_corrections
                .get(&stripe_index)
                .copied()
                .unwrap_or(Dp::ZERO)
    }

    fn stripe_offset(&self, stripe_index: usize, spacing: Dp) -> Dp {
        let Some(signature) = self.signature else {
            return Dp::ZERO;
        };
        let stripe_count = signature.stripe_count();
        let stripe_index = stripe_index.min(stripe_count);
        let base_step = signature.estimate() + spacing;
        Dp::new(
            base_step.get() * stripe_index as f32
                + self.corrections.prefix_sum(stripe_count, stripe_index),
        )
    }

    fn total_main_extent(&self, spacing: Dp) -> Dp {
        let Some(signature) = self.signature else {
            return Dp::ZERO;
        };
        let stripe_count = signature.stripe_count();
        if stripe_count == 0 {
            return Dp::ZERO;
        }
        let base_step = signature.estimate() + spacing;
        Dp::new(
            base_step.get() * stripe_count as f32 + self.corrections.correction_sum()
                - spacing.get(),
        )
        .max(Dp::ZERO)
    }

    fn first_stripe_after(&self, spacing: Dp, target: Dp, inclusive: bool) -> Option<usize> {
        let signature = self.signature?;
        let stripe_count = signature.stripe_count();
        let base_step = (signature.estimate() + spacing).get();
        self.corrections
            .first_prefix_matching(stripe_count, base_step, target.get(), inclusive)
    }

    fn update_measurements(&mut self, measurements: &[(usize, Dp)]) -> bool {
        let Some(signature) = self.signature else {
            return false;
        };
        let mut changed_stripes = Vec::new();
        for (item_index, extent) in measurements {
            if *item_index >= signature.total_items {
                continue;
            }
            let extent = extent.max(Dp::ZERO);
            let changed = self
                .measured_extents
                .get(item_index)
                .copied()
                .map(|previous| (previous - extent).abs() > MEASURED_EXTENT_INVALIDATION_EPSILON)
                .unwrap_or(true);
            if !changed {
                continue;
            }
            self.measured_extents.insert(*item_index, extent);
            let stripe_index = *item_index / signature.lanes;
            if !changed_stripes.contains(&stripe_index) {
                changed_stripes.push(stripe_index);
            }
        }

        let mut prefix_changed = false;
        let stripe_count = signature.stripe_count();
        for stripe_index in changed_stripes {
            let start = stripe_index * signature.lanes;
            let end = ((stripe_index + 1) * signature.lanes).min(signature.total_items);
            let mut extent = Dp::ZERO;
            let mut all_measured = true;
            for item_index in start..end {
                if let Some(measured) = self.measured_extents.get(&item_index).copied() {
                    extent = extent.max(measured);
                } else {
                    all_measured = false;
                }
            }
            if !all_measured {
                extent = extent.max(signature.estimate());
            }
            let correction = extent - signature.estimate();
            let previous = self
                .stripe_corrections
                .get(&stripe_index)
                .copied()
                .unwrap_or(Dp::ZERO);
            if (previous - correction).abs() <= MEASURED_EXTENT_INVALIDATION_EPSILON {
                continue;
            }
            prefix_changed = true;
            if correction == Dp::ZERO {
                self.stripe_corrections.remove(&stripe_index);
            } else {
                self.stripe_corrections.insert(stripe_index, correction);
            }
            self.corrections
                .set(stripe_count, stripe_index, correction.get());
        }
        if prefix_changed {
            self.revision = self.revision.wrapping_add(1);
        }
        prefix_changed
    }
}

#[derive(Clone, Default)]
pub(crate) struct MeasuredVirtualState {
    inner: Arc<RwLock<MeasuredVirtualIndex>>,
}

impl MeasuredVirtualState {
    fn prepare(&self, signature: MeasuredVirtualSignature) {
        self.inner.write().prepare(signature);
    }

    pub(crate) fn measured_extent(&self, item_index: usize) -> Option<Dp> {
        self.inner.read().measured_extents.get(&item_index).copied()
    }

    pub(crate) fn signature(&self) -> Option<MeasuredVirtualSignature> {
        self.inner.read().signature
    }

    pub(crate) fn update_measurements(
        &self,
        signature: MeasuredVirtualSignature,
        measurements: &[(usize, Dp)],
    ) -> bool {
        let mut index = self.inner.write();
        index.prepare(signature);
        index.update_measurements(measurements)
    }

    #[cfg(test)]
    pub(crate) fn replace_measurements(
        &self,
        signature: MeasuredVirtualSignature,
        measurements: impl IntoIterator<Item = (usize, Dp)>,
    ) {
        let measurements = measurements.into_iter().collect::<Vec<_>>();
        let mut index = self.inner.write();
        index.prepare(signature);
        index.update_measurements(&measurements);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualViewportHint {
    pub(crate) width: Dp,
    pub(crate) height: Dp,
}

#[derive(Clone)]
pub(crate) struct VirtualRuntimeState {
    pub(crate) fallback_viewport_hint: VirtualViewportHint,
    pub(crate) viewport_hint: Option<VirtualViewportHint>,
    pub(crate) scroll_offset: Point,
    pub(crate) measurements: MeasuredVirtualState,
    pub(crate) widget_ids_by_key: HashMap<WidgetKey, WidgetId>,
    pub(crate) source_revision: u64,
    pub(crate) bootstrap: bool,
}

#[derive(Clone, Default)]
pub(crate) struct VirtualCacheState {
    pub(crate) viewport_hint: Option<VirtualViewportHint>,
    pub(crate) measurements: MeasuredVirtualState,
    pub(crate) widget_ids_by_key: HashMap<WidgetKey, WidgetId>,
}

impl VirtualCacheState {
    /// Returns the retained main-axis bounds for one virtual item.
    ///
    /// Fixed/estimated lists do not populate the measured index and use the
    /// caller's uniform fallback. Measured lists reuse the sparse prefix tree,
    /// keeping keyboard navigation to an off-window item O(log N) without
    /// walking the source.
    pub(crate) fn item_main_bounds(
        &self,
        item_index: usize,
        fallback_extent: Dp,
        spacing: Dp,
    ) -> (Dp, Dp) {
        let fallback_extent = fallback_extent.max(Dp::ZERO);
        let spacing = spacing.max(Dp::ZERO);
        let index = self.measurements.inner.read();
        let Some(signature) = index.signature else {
            let start = (fallback_extent + spacing) * item_index as f32;
            return (start, start + fallback_extent);
        };
        let stripe_index = item_index / signature.lanes.max(1);
        let start = index.stripe_offset(stripe_index, spacing);
        (start, start + index.stripe_extent(stripe_index))
    }

    /// Resolves the virtual item covering `main_offset` without materializing
    /// intervening source items. Uniform layouts are O(1); measured layouts
    /// reuse the sparse correction tree and remain O(log N).
    pub(crate) fn item_index_at_main_offset(
        &self,
        main_offset: Dp,
        fallback_extent: Dp,
        spacing: Dp,
        max_item_index: usize,
    ) -> usize {
        let main_offset = main_offset.max(Dp::ZERO);
        let fallback_extent = fallback_extent.max(Dp::ZERO);
        let spacing = spacing.max(Dp::ZERO);
        let index = self.measurements.inner.read();
        let Some(signature) = index.signature else {
            let step = fallback_extent + spacing;
            if step <= Dp::ZERO {
                return 0;
            }
            return ((main_offset / step).floor() as usize).min(max_item_index);
        };
        let stripe_count = signature.stripe_count();
        if stripe_count == 0 {
            return 0;
        }
        let stripe_index = index
            .first_stripe_after(spacing, main_offset, false)
            .unwrap_or(stripe_count - 1)
            .min(stripe_count - 1);
        stripe_index
            .saturating_mul(signature.lanes.max(1))
            .min(max_item_index)
    }
}

impl Default for VirtualRuntimeState {
    fn default() -> Self {
        Self {
            fallback_viewport_hint: VirtualViewportHint::default(),
            viewport_hint: None,
            scroll_offset: Point::ZERO,
            measurements: MeasuredVirtualState::default(),
            widget_ids_by_key: HashMap::new(),
            source_revision: 0,
            bootstrap: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualResolvedItemMeta {
    pub(crate) item_index: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualItemPlacement {
    pub(crate) item_index: usize,
    pub(crate) main_offset: Dp,
    pub(crate) cross_offset: Dp,
    pub(crate) cross_extent: Dp,
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualWindowPlan {
    #[allow(
        dead_code,
        reason = "retained for lifecycle snapshots and debug tooling"
    )]
    pub(crate) total_items: usize,
    #[allow(
        dead_code,
        reason = "retained for lifecycle snapshots and debug tooling"
    )]
    pub(crate) visible_range: Range<usize>,
    pub(crate) placements: Vec<VirtualItemPlacement>,
    pub(crate) total_main_extent: Dp,
    pub(crate) measurement_revision: u64,
    #[allow(
        dead_code,
        reason = "retained for lifecycle snapshots and debug tooling"
    )]
    pub(crate) viewport_hint: VirtualViewportHint,
    #[allow(
        dead_code,
        reason = "retained for lifecycle snapshots and debug tooling"
    )]
    pub(crate) bootstrap: bool,
}

#[derive(Clone)]
pub(crate) struct VirtualSceneStateUpdate {
    pub(crate) widget_id: WidgetId,
    pub(crate) viewport_hint: VirtualViewportHint,
    pub(crate) measured_extents: Vec<(usize, Dp)>,
    pub(crate) measurement_signature: Option<MeasuredVirtualSignature>,
    pub(crate) widget_ids_by_key: Vec<(WidgetKey, WidgetId)>,
    pub(crate) invalidate_layout: bool,
}

pub(crate) fn apply_virtual_runtime_state_to_element<VM>(
    element: &mut Element<VM>,
    scroll_offsets: &HashMap<WidgetId, Point>,
    virtual_states: &HashMap<WidgetId, VirtualCacheState>,
    fallback_viewport_hint: VirtualViewportHint,
) {
    match &mut element.kind {
        WidgetKind::Container { children, .. } => {
            for child_source in children {
                match child_source {
                    super::common::ChildSource::Static(children) => {
                        for child in children {
                            apply_virtual_runtime_state_to_element(
                                child,
                                scroll_offsets,
                                virtual_states,
                                fallback_viewport_hint.clone(),
                            );
                        }
                    }
                    super::common::ChildSource::Show { child, .. } => {
                        apply_virtual_runtime_state_to_element(
                            child,
                            scroll_offsets,
                            virtual_states,
                            fallback_viewport_hint.clone(),
                        );
                    }
                    super::common::ChildSource::Switch {
                        cases, fallback, ..
                    } => {
                        for child in cases {
                            apply_virtual_runtime_state_to_element(
                                child,
                                scroll_offsets,
                                virtual_states,
                                fallback_viewport_hint.clone(),
                            );
                        }
                        if let Some(child) = fallback {
                            apply_virtual_runtime_state_to_element(
                                child,
                                scroll_offsets,
                                virtual_states,
                                fallback_viewport_hint.clone(),
                            );
                        }
                    }
                    super::common::ChildSource::KeyedFor(_) => {}
                    super::common::ChildSource::Dynamic(_) => {}
                }
            }
        }
        WidgetKind::Virtual {
            runtime_state,
            arrangement,
            content_cross_extent,
            source,
            ..
        } => {
            if let Some(cache) = virtual_states.get(&element.id) {
                runtime_state.viewport_hint = cache.viewport_hint.clone();
                runtime_state.measurements = cache.measurements.clone();
                runtime_state.widget_ids_by_key = cache.widget_ids_by_key.clone();
                runtime_state.bootstrap = runtime_state.viewport_hint.is_none();
            } else {
                runtime_state.viewport_hint = None;
                runtime_state.measurements = MeasuredVirtualState::default();
                runtime_state.widget_ids_by_key.clear();
                runtime_state.bootstrap = true;
            }
            runtime_state.source_revision = source.revision();
            runtime_state.fallback_viewport_hint = fallback_viewport_hint.clone();
            runtime_state.scroll_offset = scroll_offsets
                .get(&element.id)
                .copied()
                .unwrap_or(Point::ZERO);
            if matches!(arrangement.direction(), VirtualDirection::Vertical)
                && content_cross_extent.is_none()
            {
                runtime_state.scroll_offset.x = Dp::ZERO;
            } else if matches!(arrangement.direction(), VirtualDirection::Horizontal)
                && content_cross_extent.is_none()
            {
                runtime_state.scroll_offset.y = Dp::ZERO;
            }
        }
        _ => {}
    }
}

pub(crate) fn resolve_virtual_window_plan(
    arrangement: VirtualArrangement,
    item_layout: ItemLayout,
    runtime_state: &VirtualRuntimeState,
    total_items: usize,
    fallback_viewport_hint: VirtualViewportHint,
    content_cross_extent: Option<Dp>,
) -> VirtualWindowPlan {
    let viewport_hint = runtime_state
        .viewport_hint
        .clone()
        .unwrap_or(fallback_viewport_hint);
    let viewport_main = match arrangement.direction() {
        VirtualDirection::Vertical => viewport_hint.height.max(Dp::ZERO),
        VirtualDirection::Horizontal => viewport_hint.width.max(Dp::ZERO),
    };
    let viewport_cross = match arrangement.direction() {
        VirtualDirection::Vertical => viewport_hint.width.max(Dp::ZERO),
        VirtualDirection::Horizontal => viewport_hint.height.max(Dp::ZERO),
    };
    let viewport_cross = content_cross_extent.unwrap_or(viewport_cross).max(Dp::ZERO);
    let scroll_main = match arrangement.direction() {
        VirtualDirection::Vertical => runtime_state.scroll_offset.y.max(Dp::ZERO),
        VirtualDirection::Horizontal => runtime_state.scroll_offset.x.max(Dp::ZERO),
    };
    let lanes = arrangement.lanes().max(1);
    let stripe_count = total_items.div_ceil(lanes);
    let spacing = item_layout.spacing().max(Dp::ZERO);
    let overscan = item_layout.overscan();
    let bootstrap = runtime_state.bootstrap;

    // Fixed and estimated layouts have one uniform extent per stripe. Building
    // an offset for every item made an otherwise bounded virtual window O(N) on
    // every layout/scroll update (including 100K+ item lists). Resolve the two
    // boundary stripes with binary search and materialize offsets only for the
    // visible window.
    if !item_layout.is_measured() {
        #[cfg(feature = "bench-support")]
        if legacy_uniform_window_plan::enabled() {
            return resolve_uniform_virtual_window_plan_legacy(
                total_items,
                lanes,
                item_layout.estimate().max(Dp::ZERO),
                spacing,
                overscan,
                viewport_main,
                viewport_cross,
                scroll_main,
                viewport_hint,
                bootstrap,
            );
        }
        return resolve_uniform_virtual_window_plan(
            total_items,
            lanes,
            item_layout.estimate().max(Dp::ZERO),
            spacing,
            overscan,
            viewport_main,
            viewport_cross,
            scroll_main,
            viewport_hint,
            bootstrap,
        );
    }

    let signature = MeasuredVirtualSignature::new(
        runtime_state.source_revision,
        total_items,
        lanes,
        item_layout.estimate(),
        viewport_cross,
    );
    runtime_state.measurements.prepare(signature);
    let index = runtime_state.measurements.inner.read();

    #[cfg(feature = "bench-support")]
    if legacy_uniform_window_plan::enabled() {
        return resolve_measured_virtual_window_plan_legacy(
            total_items,
            lanes,
            item_layout.estimate().max(Dp::ZERO),
            spacing,
            overscan,
            viewport_main,
            viewport_cross,
            scroll_main,
            viewport_hint,
            bootstrap,
            &index.measured_extents,
            index.revision,
        );
    }

    let total_main_extent = index.total_main_extent(spacing);
    let viewport_end = scroll_main + viewport_main;
    let (first_stripe, last_stripe) = if stripe_count == 0 {
        (0, 0)
    } else {
        let first = index
            .first_stripe_after(spacing, scroll_main, false)
            .unwrap_or(stripe_count - 1)
            .min(stripe_count - 1);
        let last = index
            .first_stripe_after(spacing, viewport_end, true)
            .unwrap_or(stripe_count - 1)
            .min(stripe_count - 1)
            .max(first);
        (first, last)
    };

    let visible_start = (first_stripe.saturating_sub(overscan)).saturating_mul(lanes);
    let visible_end = if stripe_count == 0 {
        0
    } else {
        ((last_stripe + 1 + overscan).min(stripe_count) * lanes).min(total_items)
    };

    let spacing_total = spacing * (lanes.saturating_sub(1) as f32);
    let lane_extent = ((viewport_cross - spacing_total).max(0.0)) / lanes as f32;
    let mut placements = Vec::with_capacity(visible_end.saturating_sub(visible_start));
    let first_visible_stripe = visible_start / lanes;
    let last_visible_stripe = visible_end.div_ceil(lanes);
    let mut main_offset = index.stripe_offset(first_visible_stripe, spacing);
    for stripe_index in first_visible_stripe..last_visible_stripe {
        let item_start = (stripe_index * lanes).max(visible_start);
        let item_end = ((stripe_index + 1) * lanes).min(visible_end);
        for item_index in item_start..item_end {
            let lane_index = item_index % lanes;
            placements.push(VirtualItemPlacement {
                item_index,
                main_offset,
                cross_offset: (lane_extent + spacing) * lane_index as f32,
                cross_extent: lane_extent,
            });
        }
        main_offset += index.stripe_extent(stripe_index) + spacing;
    }

    VirtualWindowPlan {
        total_items,
        visible_range: visible_start..visible_end,
        placements,
        total_main_extent,
        measurement_revision: index.revision,
        viewport_hint,
        bootstrap,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_uniform_virtual_window_plan(
    total_items: usize,
    lanes: usize,
    stripe_extent: Dp,
    spacing: Dp,
    overscan: usize,
    viewport_main: Dp,
    viewport_cross: Dp,
    scroll_main: Dp,
    viewport_hint: VirtualViewportHint,
    bootstrap: bool,
) -> VirtualWindowPlan {
    let stripe_count = total_items.div_ceil(lanes);
    let stripe_step = stripe_extent + spacing;
    let total_main_extent = if stripe_count == 0 {
        Dp::ZERO
    } else {
        stripe_extent * stripe_count as f32 + spacing * stripe_count.saturating_sub(1) as f32
    };

    let (first_stripe, last_stripe) = if stripe_count == 0 {
        (0, 0)
    } else {
        // Match the general path's half-open viewport semantics exactly:
        // - a stripe whose end equals scroll_main is no longer visible;
        // - a stripe whose start equals viewport_end is not yet visible.
        let first = partition_point(stripe_count.saturating_sub(1), |stripe_index| {
            stripe_step * (stripe_index + 1) as f32 <= scroll_main
        })
        .min(stripe_count - 1);
        let viewport_end = scroll_main + viewport_main;
        let remaining = stripe_count.saturating_sub(first + 1);
        let hidden_after = partition_point(remaining, |offset| {
            stripe_step * ((first + 1 + offset) as f32) < viewport_end
        });
        (first, first + hidden_after)
    };

    let visible_start = first_stripe.saturating_sub(overscan).saturating_mul(lanes);
    let visible_end = if stripe_count == 0 {
        0
    } else {
        ((last_stripe + 1 + overscan).min(stripe_count) * lanes).min(total_items)
    };
    let spacing_total = spacing * lanes.saturating_sub(1) as f32;
    let lane_extent = ((viewport_cross - spacing_total).max(0.0)) / lanes as f32;
    let mut placements = Vec::with_capacity(visible_end.saturating_sub(visible_start));
    for item_index in visible_start..visible_end {
        let stripe_index = item_index / lanes;
        let lane_index = item_index % lanes;
        placements.push(VirtualItemPlacement {
            item_index,
            main_offset: stripe_step * stripe_index as f32,
            cross_offset: (lane_extent + spacing) * lane_index as f32,
            cross_extent: lane_extent,
        });
    }

    VirtualWindowPlan {
        total_items,
        visible_range: visible_start..visible_end,
        placements,
        total_main_extent,
        measurement_revision: 0,
        viewport_hint,
        bootstrap,
    }
}

/// Returns the number of leading indexes in `0..len` for which `predicate`
/// holds. The predicate must be monotonic (`true...true,false...false`).
fn partition_point(mut len: usize, mut predicate: impl FnMut(usize) -> bool) -> usize {
    let mut base = 0usize;
    while len > 0 {
        let half = len / 2;
        let middle = base + half;
        if predicate(middle) {
            base = middle + 1;
            len -= half + 1;
        } else {
            len = half;
        }
    }
    base
}

#[cfg(any(test, feature = "bench-support"))]
#[allow(clippy::too_many_arguments)]
fn resolve_uniform_virtual_window_plan_legacy(
    total_items: usize,
    lanes: usize,
    stripe_extent: Dp,
    spacing: Dp,
    overscan: usize,
    viewport_main: Dp,
    viewport_cross: Dp,
    scroll_main: Dp,
    viewport_hint: VirtualViewportHint,
    bootstrap: bool,
) -> VirtualWindowPlan {
    let stripe_count = total_items.div_ceil(lanes);
    let mut stripe_offsets = Vec::with_capacity(stripe_count);
    let mut total_main_extent = Dp::ZERO;
    for stripe_index in 0..stripe_count {
        stripe_offsets.push(total_main_extent);
        total_main_extent += stripe_extent;
        if stripe_index + 1 < stripe_count {
            total_main_extent += spacing;
        }
    }
    let viewport_end = scroll_main + viewport_main;
    let mut first_stripe = 0usize;
    while first_stripe + 1 < stripe_count {
        let end = stripe_offsets[first_stripe] + stripe_extent + spacing;
        if end > scroll_main {
            break;
        }
        first_stripe += 1;
    }
    let mut last_stripe = first_stripe;
    while last_stripe + 1 < stripe_count {
        if stripe_offsets[last_stripe + 1] >= viewport_end {
            break;
        }
        last_stripe += 1;
    }
    if stripe_count == 0 {
        last_stripe = 0;
    }
    let visible_start = first_stripe.saturating_sub(overscan).saturating_mul(lanes);
    let visible_end = if stripe_count == 0 {
        0
    } else {
        ((last_stripe + 1 + overscan).min(stripe_count) * lanes).min(total_items)
    };
    let spacing_total = spacing * lanes.saturating_sub(1) as f32;
    let lane_extent = ((viewport_cross - spacing_total).max(0.0)) / lanes as f32;
    let placements = (visible_start..visible_end)
        .map(|item_index| {
            let stripe_index = item_index / lanes;
            let lane_index = item_index % lanes;
            VirtualItemPlacement {
                item_index,
                main_offset: stripe_offsets
                    .get(stripe_index)
                    .copied()
                    .unwrap_or(Dp::ZERO),
                cross_offset: (lane_extent + spacing) * lane_index as f32,
                cross_extent: lane_extent,
            }
        })
        .collect();
    VirtualWindowPlan {
        total_items,
        visible_range: visible_start..visible_end,
        placements,
        total_main_extent,
        measurement_revision: 0,
        viewport_hint,
        bootstrap,
    }
}

#[cfg(any(test, feature = "bench-support"))]
#[allow(clippy::too_many_arguments)]
fn resolve_measured_virtual_window_plan_legacy(
    total_items: usize,
    lanes: usize,
    estimate: Dp,
    spacing: Dp,
    overscan: usize,
    viewport_main: Dp,
    viewport_cross: Dp,
    scroll_main: Dp,
    viewport_hint: VirtualViewportHint,
    bootstrap: bool,
    measured_extents: &HashMap<usize, Dp>,
    measurement_revision: u64,
) -> VirtualWindowPlan {
    let stripe_count = total_items.div_ceil(lanes);
    let item_main_extent = |item_index: usize| -> Dp {
        measured_extents
            .get(&item_index)
            .copied()
            .unwrap_or(estimate)
            .max(Dp::ZERO)
    };
    let mut stripe_offsets = Vec::with_capacity(stripe_count);
    let mut total_main_extent = Dp::ZERO;
    for stripe_index in 0..stripe_count {
        stripe_offsets.push(total_main_extent);
        total_main_extent += stripe_extent_for(stripe_index, lanes, total_items, &item_main_extent);
        if stripe_index + 1 < stripe_count {
            total_main_extent += spacing;
        }
    }
    let stripe_extent = |stripe_index: usize| -> Dp {
        stripe_extent_for(stripe_index, lanes, total_items, &item_main_extent)
    };
    let viewport_end = scroll_main + viewport_main;
    let mut first_stripe = 0usize;
    while first_stripe + 1 < stripe_count {
        let end = stripe_offsets[first_stripe] + stripe_extent(first_stripe) + spacing;
        if end > scroll_main {
            break;
        }
        first_stripe += 1;
    }
    let mut last_stripe = first_stripe;
    while last_stripe + 1 < stripe_count {
        if stripe_offsets[last_stripe + 1] >= viewport_end {
            break;
        }
        last_stripe += 1;
    }
    if stripe_count == 0 {
        last_stripe = 0;
    }
    let visible_start = first_stripe.saturating_sub(overscan).saturating_mul(lanes);
    let visible_end = if stripe_count == 0 {
        0
    } else {
        ((last_stripe + 1 + overscan).min(stripe_count) * lanes).min(total_items)
    };
    let spacing_total = spacing * lanes.saturating_sub(1) as f32;
    let lane_extent = ((viewport_cross - spacing_total).max(0.0)) / lanes as f32;
    let placements = (visible_start..visible_end)
        .map(|item_index| {
            let stripe_index = item_index / lanes;
            let lane_index = item_index % lanes;
            VirtualItemPlacement {
                item_index,
                main_offset: stripe_offsets
                    .get(stripe_index)
                    .copied()
                    .unwrap_or(Dp::ZERO),
                cross_offset: (lane_extent + spacing) * lane_index as f32,
                cross_extent: lane_extent,
            }
        })
        .collect();
    VirtualWindowPlan {
        total_items,
        visible_range: visible_start..visible_end,
        placements,
        total_main_extent,
        measurement_revision,
        viewport_hint,
        bootstrap,
    }
}

#[cfg(feature = "bench-support")]
pub(crate) mod legacy_uniform_window_plan {
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn enabled() -> bool {
        ENABLED.with(Cell::get)
    }

    pub(crate) fn with_enabled<R>(f: impl FnOnce() -> R) -> R {
        ENABLED.with(|enabled| {
            let previous = enabled.replace(true);
            struct Reset<'a> {
                enabled: &'a Cell<bool>,
                previous: bool,
            }
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.enabled.set(self.previous);
                }
            }
            let _reset = Reset { enabled, previous };
            f()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_dp_close(actual: Dp, expected: Dp) {
        assert!(
            (actual - expected).abs() <= 0.02,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn assert_plans_equivalent(actual: &VirtualWindowPlan, expected: &VirtualWindowPlan) {
        assert_eq!(actual.visible_range, expected.visible_range);
        assert_eq!(actual.placements.len(), expected.placements.len());
        assert_dp_close(actual.total_main_extent, expected.total_main_extent);
        for (actual, expected) in actual.placements.iter().zip(&expected.placements) {
            assert_eq!(actual.item_index, expected.item_index);
            assert_dp_close(actual.main_offset, expected.main_offset);
            assert_eq!(actual.cross_offset, expected.cross_offset);
            assert_eq!(actual.cross_extent, expected.cross_extent);
        }
    }

    #[test]
    fn measured_prefix_plan_matches_full_scan_for_sparse_large_sources() {
        for total_items in [0usize, 1, 31, 1_000, 10_000, 100_000] {
            for lanes in [1usize, 2, 4, 7] {
                let estimate = dp(40.0);
                let spacing = dp(4.0);
                let viewport_cross = dp(317.0);
                let signature =
                    MeasuredVirtualSignature::new(11, total_items, lanes, estimate, viewport_cross);
                let measurements = MeasuredVirtualState::default();
                let stripe_count = total_items.div_ceil(lanes);
                let mut seeded = Vec::new();
                for stripe_index in [
                    0usize,
                    1,
                    17,
                    stripe_count / 2,
                    stripe_count.saturating_sub(1),
                ] {
                    if stripe_index >= stripe_count {
                        continue;
                    }
                    let start = stripe_index * lanes;
                    let end = ((stripe_index + 1) * lanes).min(total_items);
                    for item_index in start..end {
                        let extent = if stripe_index % 2 == 0 {
                            dp(18.0 + (item_index % lanes) as f32 * 3.0)
                        } else {
                            dp(64.0 + (item_index % lanes) as f32 * 5.0)
                        };
                        seeded.push((item_index, extent));
                    }
                }
                measurements.replace_measurements(signature, seeded);

                let mut state = VirtualRuntimeState {
                    viewport_hint: Some(VirtualViewportHint {
                        width: viewport_cross,
                        height: dp(137.0),
                    }),
                    measurements,
                    source_revision: 11,
                    bootstrap: false,
                    ..Default::default()
                };
                let end_scroll = estimate * stripe_count as f32;
                for scroll_main in [
                    Dp::ZERO,
                    dp(44.0),
                    dp(44.0 * 17.0),
                    end_scroll / 2.0,
                    end_scroll,
                    end_scroll + dp(400.0),
                ] {
                    state.scroll_offset = Point::new(Dp::ZERO, scroll_main);
                    let item_layout = ItemLayout::Measured {
                        estimate,
                        spacing,
                        overscan: 3,
                    };
                    let actual = resolve_virtual_window_plan(
                        VirtualArrangement::Grid {
                            direction: VirtualDirection::Vertical,
                            lanes,
                        },
                        item_layout,
                        &state,
                        total_items,
                        VirtualViewportHint::default(),
                        None,
                    );
                    let index = state.measurements.inner.read();
                    let expected = resolve_measured_virtual_window_plan_legacy(
                        total_items,
                        lanes,
                        estimate,
                        spacing,
                        3,
                        dp(137.0),
                        viewport_cross,
                        scroll_main,
                        state.viewport_hint.clone().unwrap(),
                        false,
                        &index.measured_extents,
                        index.revision,
                    );
                    assert_plans_equivalent(&actual, &expected);
                }
            }
        }
    }

    #[test]
    fn measured_prefix_updates_only_when_the_lane_max_changes() {
        let signature = MeasuredVirtualSignature::new(7, 5, 2, dp(40.0), dp(200.0));
        let measurements = MeasuredVirtualState::default();
        assert!(measurements.update_measurements(signature, &[(0, dp(80.0))]));
        assert!(measurements.update_measurements(signature, &[(1, dp(90.0))]));
        assert!(measurements.update_measurements(signature, &[(1, dp(20.0))]));
        assert!(
            !measurements.update_measurements(signature, &[(1, dp(30.0))]),
            "changing a non-max lane must not perturb the prefix index"
        );
        assert!(measurements.update_measurements(signature, &[(0, dp(20.0))]));

        let index = measurements.inner.read();
        assert_eq!(index.stripe_extent(0), dp(30.0));
        assert_eq!(index.stripe_extent(1), dp(40.0));
        assert_eq!(index.stripe_extent(2), dp(40.0));
        assert_eq!(index.total_main_extent(dp(4.0)), dp(118.0));
    }

    #[test]
    fn measured_signature_change_discards_stale_indexed_feedback() {
        let measurements = MeasuredVirtualState::default();
        let initial = MeasuredVirtualSignature::new(1, 100_000, 1, dp(40.0), dp(300.0));
        assert!(measurements.update_measurements(initial, &[(50_000, dp(100.0))]));
        assert_eq!(measurements.measured_extent(50_000), Some(dp(100.0)));

        let reordered = MeasuredVirtualSignature::new(2, 100_000, 1, dp(40.0), dp(300.0));
        measurements.prepare(reordered);
        assert_eq!(measurements.measured_extent(50_000), None);
        let index = measurements.inner.read();
        assert_eq!(index.total_main_extent(Dp::ZERO), dp(4_000_000.0));
    }

    #[test]
    fn uniform_window_plan_matches_legacy_boundaries_for_large_sources() {
        for total_items in [0usize, 1, 2, 31, 1_000, 10_000, 100_000] {
            for lanes in [1usize, 2, 4, 7] {
                for stripe_extent in [Dp::ZERO, dp(20.0), dp(40.0)] {
                    for spacing in [Dp::ZERO, dp(4.0)] {
                        let step = stripe_extent + spacing;
                        let stripe_count = total_items.div_ceil(lanes);
                        let end = step * stripe_count.saturating_sub(1) as f32;
                        for scroll_main in [
                            Dp::ZERO,
                            step,
                            (step - dp(0.25)).max(Dp::ZERO),
                            step * 17.0,
                            end,
                            end + step,
                        ] {
                            for overscan in [0usize, 1, 3] {
                                let viewport_hint = VirtualViewportHint {
                                    width: dp(311.0),
                                    height: dp(123.0),
                                };
                                let fast = resolve_uniform_virtual_window_plan(
                                    total_items,
                                    lanes,
                                    stripe_extent,
                                    spacing,
                                    overscan,
                                    dp(123.0),
                                    dp(311.0),
                                    scroll_main,
                                    viewport_hint.clone(),
                                    false,
                                );
                                let legacy = resolve_uniform_virtual_window_plan_legacy(
                                    total_items,
                                    lanes,
                                    stripe_extent,
                                    spacing,
                                    overscan,
                                    dp(123.0),
                                    dp(311.0),
                                    scroll_main,
                                    viewport_hint,
                                    false,
                                );
                                assert_eq!(fast.visible_range, legacy.visible_range);
                                assert_eq!(fast.placements.len(), legacy.placements.len());
                                assert_eq!(fast.total_main_extent, legacy.total_main_extent);
                                for (actual, expected) in
                                    fast.placements.iter().zip(&legacy.placements)
                                {
                                    assert_eq!(actual.item_index, expected.item_index);
                                    assert_eq!(actual.main_offset, expected.main_offset);
                                    assert_eq!(actual.cross_offset, expected.cross_offset);
                                    assert_eq!(actual.cross_extent, expected.cross_extent);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_and_estimated_layouts_route_through_equivalent_uniform_plan() {
        let mut state = VirtualRuntimeState::default();
        state.viewport_hint = Some(VirtualViewportHint {
            width: dp(320.0),
            height: dp(120.0),
        });
        state.scroll_offset = Point::new(Dp::ZERO, dp(400_000.0));
        state.bootstrap = false;
        for item_layout in [
            ItemLayout::Fixed {
                item_extent: dp(40.0),
                spacing: dp(4.0),
                overscan: 2,
            },
            ItemLayout::Estimated {
                estimate: dp(40.0),
                spacing: dp(4.0),
                overscan: 2,
            },
        ] {
            let plan = resolve_virtual_window_plan(
                VirtualArrangement::Linear(VirtualDirection::Vertical),
                item_layout,
                &state,
                100_000,
                VirtualViewportHint::default(),
                None,
            );
            let expected = resolve_uniform_virtual_window_plan_legacy(
                100_000,
                1,
                dp(40.0),
                dp(4.0),
                2,
                dp(120.0),
                dp(320.0),
                dp(400_000.0),
                state.viewport_hint.clone().unwrap(),
                false,
            );
            assert_eq!(plan.visible_range, expected.visible_range);
            assert_eq!(plan.total_main_extent, expected.total_main_extent);
            assert_eq!(plan.placements.len(), expected.placements.len());
        }
    }
}

#[cfg(any(test, feature = "bench-support"))]
fn stripe_extent_for<F>(
    stripe_index: usize,
    lanes: usize,
    total_items: usize,
    item_main_extent: &F,
) -> Dp
where
    F: Fn(usize) -> Dp,
{
    let start = stripe_index * lanes;
    let end = ((stripe_index + 1) * lanes).min(total_items);
    let mut extent = Dp::ZERO;
    for item_index in start..end {
        extent = extent.max(item_main_extent(item_index));
    }
    extent
}

type VirtualBuildFn<VM> = dyn for<'a, 'b> Fn(usize, StyleContext<'a>, &'b StyleSheet) -> Option<Element<VM>>
    + Send
    + Sync;

pub(crate) struct ErasedVirtualItemSource<VM> {
    len_fn: Arc<dyn Fn() -> usize + Send + Sync>,
    revision_fn: Arc<dyn Fn() -> u64 + Send + Sync>,
    key_fn: Arc<dyn Fn(usize) -> Option<WidgetKey> + Send + Sync>,
    build_fn: Arc<VirtualBuildFn<VM>>,
}

impl<VM> Clone for ErasedVirtualItemSource<VM> {
    fn clone(&self) -> Self {
        Self {
            len_fn: self.len_fn.clone(),
            revision_fn: self.revision_fn.clone(),
            key_fn: self.key_fn.clone(),
            build_fn: self.build_fn.clone(),
        }
    }
}

impl<VM> ErasedVirtualItemSource<VM> {
    pub(crate) fn revision(&self) -> u64 {
        (self.revision_fn)()
    }
}

impl<VM: 'static> ErasedVirtualItemSource<VM> {
    pub(crate) fn new<T, S>(
        source: Arc<S>,
        render: Arc<dyn Fn(usize, &T) -> Element<VM> + Send + Sync>,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            len_fn: {
                let source = source.clone();
                Arc::new(move || source.len())
            },
            revision_fn: {
                let source = source.clone();
                Arc::new(move || source.revision())
            },
            key_fn: {
                let source = source.clone();
                Arc::new(move |index| source.key(index))
            },
            build_fn: Arc::new(move |index, _context, _style_sheet| {
                let item = source.item(index)?;
                Some(render(index, &item))
            }),
        }
    }

    pub(crate) fn new_with_style_context<T, S>(
        source: Arc<S>,
        render: Arc<
            dyn for<'a, 'b> Fn(usize, &T, StyleContext<'a>, &'b StyleSheet) -> Element<VM>
                + Send
                + Sync,
        >,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            len_fn: {
                let source = source.clone();
                Arc::new(move || source.len())
            },
            revision_fn: {
                let source = source.clone();
                Arc::new(move || source.revision())
            },
            key_fn: {
                let source = source.clone();
                Arc::new(move |index| source.key(index))
            },
            build_fn: Arc::new(move |index, context, style_sheet| {
                let item = source.item(index)?;
                Some(render(index, &item, context, style_sheet))
            }),
        }
    }

    pub(crate) fn new_with_context<T, S>(
        source: Arc<S>,
        render: Arc<dyn Fn(crate::ui::widget::ListItemContext<T>) -> Element<VM> + Send + Sync>,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            len_fn: {
                let source = source.clone();
                Arc::new(move || source.len())
            },
            revision_fn: {
                let source = source.clone();
                Arc::new(move || source.revision())
            },
            key_fn: {
                let source = source.clone();
                Arc::new(move |index| source.key(index))
            },
            build_fn: Arc::new(move |index, _context, _style_sheet| {
                let item = source.item(index)?;
                let key = source.key(index).unwrap_or_else(|| WidgetKey::from(index));
                Some(render(crate::ui::widget::ListItemContext {
                    index,
                    key,
                    item,
                    selected: false,
                    disabled: false,
                }))
            }),
        }
    }

    pub(crate) fn len(&self) -> usize {
        (self.len_fn)()
    }

    pub(crate) fn key(&self, index: usize) -> Option<WidgetKey> {
        (self.key_fn)(index)
    }

    pub(crate) fn build(
        &self,
        index: usize,
        context: StyleContext<'_>,
        style_sheet: &StyleSheet,
    ) -> Option<Element<VM>> {
        (self.build_fn)(index, context, style_sheet)
    }

    pub(crate) fn scope<RootVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ErasedVirtualItemSource<RootVm>
    where
        VM: 'static,
    {
        let len_fn = self.len_fn.clone();
        let revision_fn = self.revision_fn.clone();
        let key_fn = self.key_fn.clone();
        let build_fn = self.build_fn.clone();
        ErasedVirtualItemSource {
            len_fn,
            revision_fn,
            key_fn,
            build_fn: Arc::new(move |index, context, style_sheet| {
                build_fn(index, context, style_sheet)
                    .map(|element| element.scope_with_selector(selector.clone()))
            }),
        }
    }
}

pub struct VirtualViewport<T, VM> {
    element: Element<VM>,
    marker: PhantomData<fn() -> T>,
}

impl<T, VM: 'static> VirtualViewport<T, VM> {
    pub fn new<S>(
        source: S,
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        render: impl Fn(usize, &T) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        let source = Arc::new(source);
        let render = Arc::new(render);
        let (overflow_x, overflow_y) = match arrangement.direction() {
            VirtualDirection::Vertical => (Overflow::Hidden, Overflow::Scroll),
            VirtualDirection::Horizontal => (Overflow::Scroll, Overflow::Hidden),
        };

        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: crate::ui::layout::LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                tooltip: None,
                popover: None,
                menu: None,
                context_menu: None,
                modal: None,
                drawer: None,
                tab_trigger: None,
                list_item: None,
                tree_root: None,
                tree_node: None,
                data_grid_root: None,
                data_grid_cell: None,
                data_grid_header: None,
                data_grid_resize_handle: None,
                splitter_handle: None,
                carousel_auto_play: None,
                kind: WidgetKind::Virtual {
                    arrangement,
                    item_layout,
                    source: ErasedVirtualItemSource::new::<T, S>(source, render),
                    content_cross_extent: None,
                    overflow_x,
                    overflow_y,
                    style: None,
                    runtime_layout: None,
                    runtime_state: VirtualRuntimeState::default(),
                },
            },
            marker: PhantomData,
        }
    }

    pub(crate) fn new_with_style_context<S>(
        source: S,
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        render: impl for<'a, 'b> Fn(usize, &T, StyleContext<'a>, &'b StyleSheet) -> Element<VM>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        let source = Arc::new(source);
        let render = Arc::new(render);
        let (overflow_x, overflow_y) = match arrangement.direction() {
            VirtualDirection::Vertical => (Overflow::Hidden, Overflow::Scroll),
            VirtualDirection::Horizontal => (Overflow::Scroll, Overflow::Hidden),
        };

        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: crate::ui::layout::LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                tooltip: None,
                popover: None,
                menu: None,
                context_menu: None,
                modal: None,
                drawer: None,
                tab_trigger: None,
                list_item: None,
                tree_root: None,
                tree_node: None,
                data_grid_root: None,
                data_grid_cell: None,
                data_grid_header: None,
                data_grid_resize_handle: None,
                splitter_handle: None,
                carousel_auto_play: None,
                kind: WidgetKind::Virtual {
                    arrangement,
                    item_layout,
                    source: ErasedVirtualItemSource::new_with_style_context::<T, S>(source, render),
                    content_cross_extent: None,
                    overflow_x,
                    overflow_y,
                    style: None,
                    runtime_layout: None,
                    runtime_state: VirtualRuntimeState::default(),
                },
            },
            marker: PhantomData,
        }
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    pub(crate) fn widget_id(mut self, id: WidgetId) -> Self {
        self.element.id = id;
        self
    }

    pub fn arrangement(mut self, arrangement: VirtualArrangement) -> Self {
        if let WidgetKind::Virtual {
            arrangement: current,
            overflow_x,
            overflow_y,
            ..
        } = &mut self.element.kind
        {
            *current = arrangement;
            let (next_x, next_y) = virtual_default_overflow(arrangement);
            *overflow_x = next_x;
            *overflow_y = next_y;
        }
        self
    }

    pub fn direction(mut self, direction: VirtualDirection) -> Self {
        if let WidgetKind::Virtual {
            arrangement,
            overflow_x,
            overflow_y,
            ..
        } = &mut self.element.kind
        {
            *arrangement = match *arrangement {
                VirtualArrangement::Linear(_) => VirtualArrangement::Linear(direction),
                VirtualArrangement::Grid { lanes, .. } => {
                    VirtualArrangement::Grid { direction, lanes }
                }
            };
            let (next_x, next_y) = virtual_default_overflow(*arrangement);
            *overflow_x = next_x;
            *overflow_y = next_y;
        }
        self
    }

    pub fn item_layout(mut self, layout: ItemLayout) -> Self {
        if let WidgetKind::Virtual { item_layout, .. } = &mut self.element.kind {
            *item_layout = layout;
        }
        self
    }

    pub fn content_cross_extent(mut self, extent: impl Into<Value<Dp>>) -> Self {
        if let WidgetKind::Virtual {
            content_cross_extent,
            ..
        } = &mut self.element.kind
        {
            *content_cross_extent = Some(extent.into());
        }
        self
    }

    pub fn spacing(mut self, spacing: Dp) -> Self {
        if let WidgetKind::Virtual { item_layout, .. } = &mut self.element.kind {
            *item_layout = item_layout.with_spacing(spacing);
        }
        self
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        if let WidgetKind::Virtual { item_layout, .. } = &mut self.element.kind {
            *item_layout = item_layout.with_overscan(overscan);
        }
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut ContainerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Virtual { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::mutate(
                |context| ContainerStyle::default_for_theme(context.theme),
                mutator,
            ));
        }
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ContainerStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Virtual { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::full(resolver));
        }
        self
    }

    pub(crate) fn style_full_with_style_sheet(
        mut self,
        resolver: impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        if let WidgetKind::Virtual { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::full_with_style_sheet(resolver));
        }
        self
    }

    pub(crate) fn runtime_layout(
        mut self,
        resolver: impl Fn(
                &mut crate::ui::layout::LayoutStyle,
                &mut ItemLayout,
                &StyleContext<'_>,
                &StyleSheet,
                &VisualStyle,
            ) + Send
            + Sync
            + 'static,
    ) -> Self {
        if let WidgetKind::Virtual { runtime_layout, .. } = &mut self.element.kind {
            *runtime_layout = Some(Arc::new(resolver));
        }
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element.focus.focusable = Some(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.element.focus.tab_index = Some(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.element.focus.scope = Some(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.element.focus.scope = Some(
            self.element
                .focus
                .scope
                .take()
                .unwrap_or_default()
                .auto_focus_first(auto_focus_first),
        );
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual {
            overflow_x,
            overflow_y,
            ..
        } = &mut self.element.kind
        {
            *overflow_x = overflow;
            *overflow_y = overflow;
        }
        self
    }

    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual { overflow_x, .. } = &mut self.element.kind {
            *overflow_x = overflow;
        }
        self
    }

    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual { overflow_y, .. } = &mut self.element.kind {
            *overflow_y = overflow;
        }
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.element.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.element.visual.offset = offset.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.element.visual.border_radius = Some(radius.into());
        self
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        set_layout_lengths(&mut self.element.layout, width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.height, height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.min_width, width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.min_height, height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.max_width, width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.max_height, height);
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
        self.element.layout.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    pub fn margin(mut self, margin: impl Into<Value<Insets>>) -> Self {
        self.element.layout.margin = margin.into();
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.element.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.element.layout.shrink = shrink.into();
        self
    }

    pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
        self.element.layout.basis = Some(basis.into_length_value());
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.element.layout.align_self = Some(align);
        self
    }

    pub fn justify_self(mut self, align: Align) -> Self {
        self.element.layout.justify_self = Some(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.left, value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.top, value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.right, value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.bottom, value);
        self
    }

    pub fn inset(
        self,
        left: impl IntoLengthValue,
        top: impl IntoLengthValue,
        right: impl IntoLengthValue,
        bottom: impl IntoLengthValue,
    ) -> Self {
        self.left(left).top(top).right(right).bottom(bottom)
    }

    pub fn background(mut self, color: impl Into<Value<crate::foundation::color::Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn background_brush(mut self, brush: impl Into<Value<BackgroundBrush>>) -> Self {
        self.element.visual.background_brush = Some(brush.into());
        self
    }

    pub fn background_image(mut self, image: impl Into<Value<BackgroundImage>>) -> Self {
        self.element.visual.background_image = Some(image.into());
        self
    }

    pub fn background_blur(mut self, blur: impl Into<Value<Dp>>) -> Self {
        self.element.visual.background_blur = blur.into();
        self
    }
}

impl<T, VM> From<VirtualViewport<T, VM>> for Element<VM> {
    fn from(value: VirtualViewport<T, VM>) -> Self {
        value.element
    }
}

pub struct VirtualList<T, VM> {
    viewport: VirtualViewport<T, VM>,
}

impl<T, VM: 'static> VirtualList<T, VM> {
    pub fn new<S>(
        source: S,
        render: impl Fn(usize, &T) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            viewport: VirtualViewport::new(
                source,
                VirtualArrangement::Linear(VirtualDirection::Vertical),
                ItemLayout::Fixed {
                    item_extent: dp(40.0),
                    spacing: Dp::ZERO,
                    overscan: 2,
                },
                render,
            ),
        }
    }

    pub(crate) fn new_with_style_context<S>(
        source: S,
        render: impl for<'a, 'b> Fn(usize, &T, StyleContext<'a>, &'b StyleSheet) -> Element<VM>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            viewport: VirtualViewport::new_with_style_context(
                source,
                VirtualArrangement::Linear(VirtualDirection::Vertical),
                ItemLayout::Fixed {
                    item_extent: dp(40.0),
                    spacing: Dp::ZERO,
                    overscan: 2,
                },
                render,
            ),
        }
    }

    pub fn new_with_context<S>(
        source: S,
        render: impl Fn(crate::ui::widget::ListItemContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
        S: ItemSource<T>,
    {
        let source = Arc::new(source);
        let render = Arc::new(render);
        Self {
            viewport: VirtualViewport {
                element: Element {
                    id: WidgetId::next(),
                    key: None,
                    layout: crate::ui::layout::LayoutStyle::default(),
                    focus: Default::default(),
                    visual: VisualStyle::default(),
                    interactions: InteractionHandlers::default(),
                    lifecycle_events: LifecycleEventHandlers::default(),
                    media_events: MediaEventHandlers::default(),
                    background: None,
                    tooltip: None,
                    popover: None,
                    menu: None,
                    context_menu: None,
                    modal: None,
                    drawer: None,
                    tab_trigger: None,
                    list_item: None,
                    tree_root: None,
                    tree_node: None,
                    data_grid_root: None,
                    data_grid_cell: None,
                    data_grid_header: None,
                    data_grid_resize_handle: None,
                    splitter_handle: None,
                    carousel_auto_play: None,
                    kind: WidgetKind::Virtual {
                        arrangement: VirtualArrangement::Linear(VirtualDirection::Vertical),
                        item_layout: ItemLayout::Fixed {
                            item_extent: dp(40.0),
                            spacing: Dp::ZERO,
                            overscan: 2,
                        },
                        source: ErasedVirtualItemSource::new_with_context::<T, S>(source, render),
                        content_cross_extent: None,
                        overflow_x: Overflow::Hidden,
                        overflow_y: Overflow::Scroll,
                        style: None,
                        runtime_layout: None,
                        runtime_state: VirtualRuntimeState::default(),
                    },
                },
                marker: PhantomData,
            },
        }
    }

    pub fn arrangement(mut self, arrangement: VirtualArrangement) -> Self {
        self.viewport = self.viewport.arrangement(arrangement);
        self
    }

    pub fn direction(mut self, direction: VirtualDirection) -> Self {
        self.viewport = self.viewport.direction(direction);
        self
    }

    pub fn item_layout(mut self, layout: ItemLayout) -> Self {
        self.viewport = self.viewport.item_layout(layout);
        self
    }

    pub fn content_cross_extent(mut self, extent: impl Into<Value<Dp>>) -> Self {
        self.viewport = self.viewport.content_cross_extent(extent);
        self
    }

    pub fn spacing(mut self, spacing: Dp) -> Self {
        self.viewport = self.viewport.spacing(spacing);
        self
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        self.viewport = self.viewport.overscan(overscan);
        self
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.viewport = self.viewport.key(key);
        self
    }

    pub(crate) fn widget_id(mut self, id: WidgetId) -> Self {
        self.viewport = self.viewport.widget_id(id);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut ContainerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.viewport = self.viewport.style(mutator);
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ContainerStyle + Send + Sync + 'static,
    ) -> Self {
        self.viewport = self.viewport.style_full(resolver);
        self
    }

    pub(crate) fn style_full_with_style_sheet(
        mut self,
        resolver: impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.viewport = self.viewport.style_full_with_style_sheet(resolver);
        self
    }

    pub(crate) fn runtime_layout(
        mut self,
        resolver: impl Fn(
                &mut crate::ui::layout::LayoutStyle,
                &mut ItemLayout,
                &StyleContext<'_>,
                &StyleSheet,
                &VisualStyle,
            ) + Send
            + Sync
            + 'static,
    ) -> Self {
        self.viewport = self.viewport.runtime_layout(resolver);
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_click(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_double_click(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_mouse_enter(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_mouse_leave(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.viewport = self.viewport.on_mouse_move(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_mount(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_unmount(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.viewport = self.viewport.on_update(command);
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.viewport = self.viewport.cursor(cursor);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.viewport = self.viewport.focusable(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.viewport = self.viewport.tab_index(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.viewport = self.viewport.focus_scope(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.viewport = self.viewport.auto_focus_first(auto_focus_first);
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.viewport = self.viewport.overflow(overflow);
        self
    }

    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        self.viewport = self.viewport.overflow_x(overflow);
        self
    }

    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        self.viewport = self.viewport.overflow_y(overflow);
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.viewport = self.viewport.opacity(opacity);
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.viewport = self.viewport.offset(offset);
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.viewport = self.viewport.border_radius(radius);
        self
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.size(width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.width(width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.height(height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.min_width(width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.min_height(height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.max_width(width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.max_height(height);
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
        self.viewport = self.viewport.aspect_ratio(aspect_ratio);
        self
    }

    pub fn margin(mut self, margin: impl Into<Value<Insets>>) -> Self {
        self.viewport = self.viewport.margin(margin);
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.viewport = self.viewport.grow(grow);
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.viewport = self.viewport.shrink(shrink);
        self
    }

    pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.basis(basis);
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.viewport = self.viewport.align_self(align);
        self
    }

    pub fn justify_self(mut self, align: Align) -> Self {
        self.viewport = self.viewport.justify_self(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.viewport = self.viewport.position_absolute();
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.left(value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.top(value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.right(value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        self.viewport = self.viewport.bottom(value);
        self
    }

    pub fn inset(
        mut self,
        left: impl IntoLengthValue,
        top: impl IntoLengthValue,
        right: impl IntoLengthValue,
        bottom: impl IntoLengthValue,
    ) -> Self {
        self.viewport = self.viewport.inset(left, top, right, bottom);
        self
    }

    pub fn background(mut self, color: impl Into<Value<crate::foundation::color::Color>>) -> Self {
        self.viewport = self.viewport.background(color);
        self
    }

    pub fn background_brush(mut self, brush: impl Into<Value<BackgroundBrush>>) -> Self {
        self.viewport = self.viewport.background_brush(brush);
        self
    }

    pub fn background_image(mut self, image: impl Into<Value<BackgroundImage>>) -> Self {
        self.viewport = self.viewport.background_image(image);
        self
    }

    pub fn background_blur(mut self, blur: impl Into<Value<Dp>>) -> Self {
        self.viewport = self.viewport.background_blur(blur);
        self
    }
}

impl<T, VM> From<VirtualList<T, VM>> for Element<VM> {
    fn from(value: VirtualList<T, VM>) -> Self {
        value.viewport.into()
    }
}

fn virtual_default_overflow(arrangement: VirtualArrangement) -> (Overflow, Overflow) {
    match arrangement.direction() {
        VirtualDirection::Vertical => (Overflow::Hidden, Overflow::Scroll),
        VirtualDirection::Horizontal => (Overflow::Scroll, Overflow::Hidden),
    }
}
