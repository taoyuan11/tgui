//! Headless deterministic tools used by unit tests, examples, and benchmarks.
//!
//! These utilities never call a GPU or native window API. They intentionally
//! expose enough stable output to compare incremental work with a full rebuild.

use crate::animation::FrameClock;
use crate::core::{Color, DpiScale, ElementId, Error, Rect, Result, Size};
use crate::core::{PropertyId, WidgetKey};
use crate::diagnostics::{BudgetDomain, FixedBudgetResourceManager};
use crate::layout::{LayoutEngine, LayoutPassReport, LayoutSnapshot, compare_layout_snapshots};
use crate::render::{PaintCommand, SceneSnapshot};
use crate::widget::element::ElementTree;
use crate::widget::{
    ElementNodeDiagnostics, ElementTreeStats, PropertyValue, ReconcileReport, View, WidgetNode,
    WidgetType,
};
use std::cell::Cell;
use std::fmt;
use std::time::{Duration, Instant};

/// Public headless façade over the crate-private retained element tree.
///
/// It intentionally exposes copies and read-only diagnostics rather than the
/// mutable element arena itself.
pub struct WidgetHarness {
    tree: ElementTree,
    work_scheduled: bool,
}

impl WidgetHarness {
    pub fn new() -> Self {
        Self {
            tree: ElementTree::new(),
            work_scheduled: false,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            tree: ElementTree::with_capacity(capacity),
            work_scheduled: false,
        }
    }

    pub fn mount(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        let report = self.tree.mount(widget)?;
        self.work_scheduled = true;
        Ok(report)
    }

    pub fn reconcile(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        let report = self.tree.reconcile(widget)?;
        self.work_scheduled = true;
        Ok(report)
    }

    pub fn rebuild_view(&mut self, view: &dyn View) -> Result<ReconcileReport> {
        let report = self.tree.rebuild_view(view)?;
        self.work_scheduled = true;
        Ok(report)
    }

    pub fn unmount(&mut self) -> Result<ReconcileReport> {
        let report = self.tree.unmount()?;
        self.work_scheduled = true;
        Ok(report)
    }

    pub fn root(&self) -> Option<ElementId> {
        self.tree.root()
    }

    pub fn contains(&self, id: ElementId) -> bool {
        self.tree.contains(id)
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.tree.parent(id)
    }

    pub fn children(&self, parent: ElementId) -> Vec<ElementId> {
        self.tree.children(parent)
    }

    pub fn ids(&self) -> Vec<ElementId> {
        self.tree.ids().collect()
    }

    pub fn child_for_key(&self, parent: ElementId, key: &WidgetKey) -> Option<ElementId> {
        self.tree
            .children(parent)
            .into_iter()
            .find(|id| self.tree.key(*id) == Some(key))
    }

    pub fn elements_for_key(&self, key: &WidgetKey) -> Vec<ElementId> {
        self.tree
            .ids()
            .filter(|id| self.tree.key(*id) == Some(key))
            .collect()
    }

    pub fn widget_type(&self, id: ElementId) -> Option<WidgetType> {
        self.tree.widget_type(id).cloned()
    }

    pub fn property(&self, id: ElementId, property: PropertyId) -> Option<PropertyValue> {
        self.tree.property(id, property).cloned()
    }

    pub fn diagnostics(&self) -> Vec<ElementNodeDiagnostics> {
        self.tree.diagnostics()
    }

    pub fn stats(&self) -> ElementTreeStats {
        self.tree.stats()
    }

    /// Consumes a pending headless build/reconcile request. Repeated idle polls
    /// return false, proving that an inactive tree does not self-schedule.
    pub fn take_scheduled_work(&mut self) -> bool {
        std::mem::take(&mut self.work_scheduled)
    }
}

impl Default for WidgetHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Headless Element + Taffy harness used to compare incremental synchronization
/// with a fresh full-tree rebuild without a platform window or GPU.
pub struct LayoutHarness {
    tree: ElementTree,
    engine: LayoutEngine,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutComparison {
    pub incremental: LayoutSnapshot,
    pub rebuilt: LayoutSnapshot,
    pub incremental_report: LayoutPassReport,
    pub rebuilt_report: LayoutPassReport,
}

impl LayoutHarness {
    pub fn new() -> Self {
        Self {
            tree: ElementTree::new(),
            engine: LayoutEngine::new(),
        }
    }

    pub fn mount(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        self.tree.mount(widget)
    }

    pub fn reconcile(&mut self, widget: WidgetNode) -> Result<ReconcileReport> {
        self.tree.reconcile(widget)
    }

    pub fn layout(
        &mut self,
        viewport: Size,
        scale: DpiScale,
    ) -> Result<(LayoutSnapshot, LayoutPassReport)> {
        let inputs = self.tree.layout_inputs()?;
        let root = self.tree.root();
        let tree = &mut self.tree;
        self.engine.compute(
            inputs,
            root,
            viewport,
            scale,
            false,
            |element, handle, input| {
                tree.capture_phase_dependencies(
                    element,
                    crate::state::DependencyPhase::Measure,
                    || handle.measure(input),
                )
            },
        )
    }

    /// Computes the current tree through both the retained incremental engine
    /// and a new forced-full engine seeded with the same previous snapshot.
    pub fn layout_and_compare(
        &mut self,
        viewport: Size,
        scale: DpiScale,
    ) -> Result<LayoutComparison> {
        let previous = self.engine.committed().clone();
        let inputs = self.tree.layout_inputs()?;
        let root = self.tree.root();
        let tree = &mut self.tree;
        let (incremental, incremental_report) = self.engine.compute(
            inputs.clone(),
            root,
            viewport,
            scale,
            false,
            |element, handle, input| {
                tree.capture_phase_dependencies(
                    element,
                    crate::state::DependencyPhase::Measure,
                    || handle.measure(input),
                )
            },
        )?;
        let mut rebuilt_engine = LayoutEngine::new();
        rebuilt_engine.adopt_committed(previous);
        let (rebuilt, rebuilt_report) = rebuilt_engine.compute(
            inputs,
            root,
            viewport,
            scale,
            true,
            |element, handle, input| {
                tree.capture_phase_dependencies(
                    element,
                    crate::state::DependencyPhase::Measure,
                    || handle.measure(input),
                )
            },
        )?;
        compare_layout_snapshots(&incremental, &rebuilt)?;
        Ok(LayoutComparison {
            incremental,
            rebuilt,
            incremental_report,
            rebuilt_report,
        })
    }

    pub fn root(&self) -> Option<ElementId> {
        self.tree.root()
    }
}

impl Default for LayoutHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable textual representation of a recorded command stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSnapshot {
    text: String,
    command_count: usize,
    fingerprint: u64,
}

impl CommandSnapshot {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn scene_snapshot(&self, revision: crate::core::SceneRevision) -> SceneSnapshot {
        SceneSnapshot::new(revision, self.command_count, self.fingerprint)
    }
}

/// Backend-free command recorder with clip/transform stack validation.
#[derive(Clone, Debug, Default)]
pub struct TestRenderer {
    commands: Vec<PaintCommand>,
    clip_depth: usize,
    transform_depth: usize,
}

impl TestRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, command: PaintCommand) -> Result<()> {
        command.validate()?;
        match &command {
            PaintCommand::PushClip(_) => self.clip_depth += 1,
            PaintCommand::PopClip => {
                if self.clip_depth == 0 {
                    return Err(Error::compile("headless_paint", "clip stack underflow"));
                }
                self.clip_depth -= 1;
            }
            PaintCommand::PushTransform(_) => self.transform_depth += 1,
            PaintCommand::PopTransform => {
                if self.transform_depth == 0 {
                    return Err(Error::compile(
                        "headless_paint",
                        "transform stack underflow",
                    ));
                }
                self.transform_depth -= 1;
            }
            _ => {}
        }
        self.commands.push(command);
        Ok(())
    }

    pub fn clear(&mut self, color: Color) -> Result<()> {
        self.record(PaintCommand::Clear(color))
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) -> Result<()> {
        self.record(PaintCommand::FillRect { rect, color })
    }

    pub fn push_clip(&mut self, clip: crate::core::Clip) -> Result<()> {
        self.record(PaintCommand::PushClip(clip))
    }

    pub fn pop_clip(&mut self) -> Result<()> {
        self.record(PaintCommand::PopClip)
    }

    pub fn push_transform(&mut self, transform: crate::core::Transform2D) -> Result<()> {
        self.record(PaintCommand::PushTransform(transform))
    }

    pub fn pop_transform(&mut self) -> Result<()> {
        self.record(PaintCommand::PopTransform)
    }

    pub fn marker(&mut self, marker: impl Into<std::sync::Arc<str>>) -> Result<()> {
        self.record(PaintCommand::Marker(marker.into()))
    }

    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }

    pub fn snapshot(&self) -> Result<CommandSnapshot> {
        if self.clip_depth != 0 || self.transform_depth != 0 {
            return Err(Error::compile(
                "headless_paint",
                format!(
                    "unbalanced stacks (clip {}, transform {})",
                    self.clip_depth, self.transform_depth
                ),
            ));
        }
        Ok(make_snapshot(&self.commands))
    }

    pub fn finish(self) -> Result<CommandSnapshot> {
        if self.clip_depth != 0 || self.transform_depth != 0 {
            return Err(Error::compile(
                "headless_paint",
                "cannot finish a command stream with an unbalanced stack",
            ));
        }
        Ok(make_snapshot(&self.commands))
    }

    pub fn render<I>(&mut self, commands: I) -> Result<CommandSnapshot>
    where
        I: IntoIterator<Item = PaintCommand>,
    {
        let mut candidate = Self::new();
        for command in commands {
            candidate.record(command)?;
        }
        let snapshot = candidate.snapshot()?;
        *self = candidate;
        Ok(snapshot)
    }
}

fn make_snapshot(commands: &[PaintCommand]) -> CommandSnapshot {
    let mut text = String::new();
    let mut hash = 0xcbf29ce484222325_u64;
    for (index, command) in commands.iter().enumerate() {
        let line = format!("{index}:{}\n", format_command(command));
        for byte in line.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        text.push_str(&line);
    }
    CommandSnapshot {
        text,
        command_count: commands.len(),
        fingerprint: hash,
    }
}

fn format_command(command: &PaintCommand) -> String {
    match command {
        PaintCommand::Clear(color) => format!("clear {}", format_color(*color)),
        PaintCommand::FillRect { rect, color } => {
            format!("fill_rect {} {}", format_rect(*rect), format_color(*color))
        }
        PaintCommand::PushClip(clip) => match clip {
            crate::core::Clip::Rect(rect) => format!("push_clip rect {}", format_rect(*rect)),
            crate::core::Clip::RoundedRect { rect, radii } => format!(
                "push_clip rounded_rect {} [{},{},{},{}]",
                format_rect(*rect),
                number(radii.top_left),
                number(radii.top_right),
                number(radii.bottom_right),
                number(radii.bottom_left)
            ),
        },
        PaintCommand::PopClip => "pop_clip".to_owned(),
        PaintCommand::PushTransform(transform) => format!(
            "push_transform [{},{},{},{},{},{}]",
            number(transform.m11),
            number(transform.m12),
            number(transform.m21),
            number(transform.m22),
            number(transform.tx),
            number(transform.ty)
        ),
        PaintCommand::PopTransform => "pop_transform".to_owned(),
        PaintCommand::Marker(marker) => format!("marker {marker:?}"),
    }
}

fn format_rect(rect: Rect) -> String {
    format!(
        "[{},{},{},{}]",
        number(rect.origin.x),
        number(rect.origin.y),
        number(rect.size.width),
        number(rect.size.height)
    )
}

fn format_color(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

fn number(value: f32) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    format!("{value:.6}")
}

/// Deterministic, manually advanced frame clock.
#[derive(Clone, Debug, Default)]
pub struct FakeClock {
    now: Cell<Duration>,
}

impl FakeClock {
    pub const fn new() -> Self {
        Self {
            now: Cell::new(Duration::ZERO),
        }
    }

    pub fn set(&self, value: Duration) {
        self.now.set(value);
    }

    pub fn advance(&self, amount: Duration) -> Result<Duration> {
        let next = self.now.get().checked_add(amount).ok_or_else(|| {
            Error::invalid_input(Some("duration".to_owned()), "fake clock overflow")
        })?;
        self.now.set(next);
        Ok(next)
    }
}

impl FrameClock for FakeClock {
    fn now(&self) -> Duration {
        self.now.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaBenchmarkResult {
    pub node_count: usize,
    pub allocation: Duration,
    pub traversal: Duration,
    pub release: Duration,
    pub allocations: u64,
    pub slot_reuses: u64,
    pub peak_live: usize,
    pub estimated_reserved_bytes: usize,
}

/// Runs a small deterministic-shape arena benchmark for each requested size.
pub fn run_arena_benchmark(sizes: &[usize]) -> Vec<ArenaBenchmarkResult> {
    sizes.iter().copied().map(run_one_benchmark).collect()
}

pub fn default_arena_benchmark() -> Vec<ArenaBenchmarkResult> {
    run_arena_benchmark(&[10, 100, 1_000])
}

fn run_one_benchmark(node_count: usize) -> ArenaBenchmarkResult {
    let mut arena = crate::core::DenseArena::<u64, ElementId>::with_capacity(node_count);
    let start = Instant::now();
    let ids = (0..node_count)
        .map(|value| arena.insert(value as u64))
        .collect::<Vec<_>>();
    let allocation = start.elapsed();

    let start = Instant::now();
    let mut checksum = 0_u64;
    for (_, value) in arena.iter() {
        checksum = checksum.wrapping_add(*value);
    }
    std::hint::black_box(checksum);
    let traversal = start.elapsed();

    let start = Instant::now();
    for id in ids {
        let _ = arena.remove(id);
    }
    let release = start.elapsed();
    let stats = arena.stats();
    ArenaBenchmarkResult {
        node_count,
        allocation,
        traversal,
        release,
        allocations: stats.fresh_slot_allocations + stats.slot_reuses,
        slot_reuses: stats.slot_reuses,
        peak_live: stats.peak_live,
        estimated_reserved_bytes: stats.estimated_reserved_bytes,
    }
}

/// Convenient name for fixed-budget test resources.
pub type TestResourceManager<K, V> = FixedBudgetResourceManager<K, V>;

pub fn test_resource_manager<K, V>(
    soft_limit_bytes: u64,
    hard_limit_bytes: u64,
) -> Result<TestResourceManager<K, V>>
where
    K: Eq + std::hash::Hash + Clone,
{
    TestResourceManager::new(BudgetDomain::CpuCache, soft_limit_bytes, hard_limit_bytes)
        .map_err(|error| Error::resource(None, error.to_string(), true))
}

impl fmt::Display for CommandSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Clip, RevisionSet};
    use crate::layout::{Dimension, LayoutStyle};

    #[test]
    fn command_snapshot_is_stable_and_stack_checked() {
        let rect = Rect::from_xywh(0.0, 0.0, 10.0, 5.0);
        let mut renderer = TestRenderer::new();
        renderer.fill_rect(rect, Color::WHITE).unwrap();
        let first = renderer.snapshot().unwrap();
        let second = renderer.snapshot().unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .text()
                .contains("fill_rect [0.000000,0.000000,10.000000,5.000000]")
        );

        renderer.push_clip(Clip::Rect(rect)).unwrap();
        assert!(renderer.snapshot().is_err());
        renderer.pop_clip().unwrap();
        assert_eq!(renderer.snapshot().unwrap().command_count(), 3);
    }

    #[test]
    fn fake_clock_is_deterministic() {
        let clock = FakeClock::new();
        assert_eq!(clock.now(), Duration::ZERO);
        clock.advance(Duration::from_millis(16)).unwrap();
        assert_eq!(clock.now(), Duration::from_millis(16));
        clock.set(Duration::from_secs(2));
        assert_eq!(clock.now(), Duration::from_secs(2));
    }

    #[test]
    fn benchmark_covers_requested_sizes() {
        let results = run_arena_benchmark(&[10, 100, 1_000]);
        assert_eq!(
            results
                .iter()
                .map(|result| result.node_count)
                .collect::<Vec<_>>(),
            [10, 100, 1_000]
        );
        assert!(
            results
                .iter()
                .all(|result| result.peak_live == result.node_count)
        );
        let _ = RevisionSet::ZERO;
    }

    #[test]
    fn layout_harness_compares_incremental_and_full_snapshots() {
        struct Root;
        struct Child;
        let root_style =
            LayoutStyle::default().with_size(Dimension::Length(100.0), Dimension::Length(50.0));
        let child = |width| {
            WidgetNode::new::<Child>().with_layout_style(
                LayoutStyle::default().with_size(Dimension::Length(width), Dimension::Length(10.0)),
            )
        };
        let mut harness = LayoutHarness::new();
        harness
            .mount(
                WidgetNode::new::<Root>()
                    .with_layout_style(root_style.clone())
                    .with_child(child(10.0)),
            )
            .unwrap();
        harness
            .layout(Size::new(100.0, 50.0), DpiScale::ONE)
            .unwrap();
        harness
            .reconcile(
                WidgetNode::new::<Root>()
                    .with_layout_style(root_style)
                    .with_child(child(25.0))
                    .with_child(child(15.0)),
            )
            .unwrap();
        let comparison = harness
            .layout_and_compare(Size::new(100.0, 50.0), DpiScale::ONE)
            .unwrap();
        assert_eq!(comparison.incremental, comparison.rebuilt);
        assert!(!comparison.incremental_report.full_rebuild);
        assert!(comparison.rebuilt_report.full_rebuild);
    }
}
