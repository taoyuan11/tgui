//! Application, window, revision, and atomic CPU-snapshot contracts.
//!
//! The headless application supports multiple generational windows, retained
//! reactive views, and transactional event dispatch while exposing a
//! convenient first/main window. All mutation methods assert the creating UI
//! thread through [`crate::state::UiThread`].

use crate::accessibility::SemanticSnapshot;
use crate::core::{
    DenseArena, DpiScale, ElementId, Error, GenerationStamp, Result, RevisionSet, Size, WindowId,
};
use crate::event::{CommittedHitTarget, DispatchOutcome, EventDispatcher, UiEvent};
use crate::layout::LayoutSnapshot;
use crate::media::ResourceSnapshot;
use crate::render::SceneSnapshot;
use crate::state::{TxnReceipt, UiCommand, UiDispatcher, UiInbox, UiThread, UpdateTxn};
use crate::widget::element::ElementTree;
use crate::widget::{ElementNodeDiagnostics, ElementTreeStats, ReconcileReport, View, WidgetNode};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

/// Window creation/configuration contract independent of a native window type.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSpec {
    title: String,
    inner_size: Size,
    min_inner_size: Option<Size>,
    max_inner_size: Option<Size>,
    resizable: bool,
    dpi_scale: DpiScale,
}

impl WindowSpec {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn inner_size(&self) -> Size {
        self.inner_size
    }

    pub fn min_inner_size(&self) -> Option<Size> {
        self.min_inner_size
    }

    pub fn max_inner_size(&self) -> Option<Size> {
        self.max_inner_size
    }

    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    pub const fn dpi_scale(&self) -> DpiScale {
        self.dpi_scale
    }

    pub fn with_inner_size(mut self, size: Size) -> Self {
        self.inner_size = size;
        self
    }

    pub fn with_min_inner_size(mut self, size: Option<Size>) -> Self {
        self.min_inner_size = size;
        self
    }

    pub fn with_max_inner_size(mut self, size: Option<Size>) -> Self {
        self.max_inner_size = size;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_dpi_scale(mut self, dpi_scale: DpiScale) -> Self {
        self.dpi_scale = dpi_scale;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(Error::invalid_input(
                Some("title".to_owned()),
                "window title must not be empty",
            ));
        }
        self.inner_size.validate().map_err(Error::from)?;
        if let Some(min) = self.min_inner_size {
            min.validate().map_err(Error::from)?;
            if min.width > self.inner_size.width || min.height > self.inner_size.height {
                return Err(Error::invalid_input(
                    Some("min_inner_size".to_owned()),
                    "minimum size exceeds the initial size",
                ));
            }
        }
        if let Some(max) = self.max_inner_size {
            max.validate().map_err(Error::from)?;
            if max.width < self.inner_size.width || max.height < self.inner_size.height {
                return Err(Error::invalid_input(
                    Some("max_inner_size".to_owned()),
                    "maximum size is smaller than the initial size",
                ));
            }
        }
        if let (Some(min), Some(max)) = (self.min_inner_size, self.max_inner_size) {
            if min.width > max.width || min.height > max.height {
                return Err(Error::invalid_input(
                    Some("inner_size_bounds".to_owned()),
                    "minimum size exceeds maximum size",
                ));
            }
        }
        // DpiScale can only be constructed through its validator; this call
        // documents the invariant without exposing its representation.
        DpiScale::new(self.dpi_scale.get()).map_err(Error::from)?;
        Ok(())
    }
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self {
            title: "tgui".to_owned(),
            inner_size: Size::new(800.0, 600.0),
            min_inner_size: None,
            max_inner_size: None,
            resizable: true,
            dpi_scale: DpiScale::ONE,
        }
    }
}

/// Four immutable subsystem outputs committed together.
#[derive(Clone, Debug, PartialEq)]
pub struct CpuSnapshot {
    layout: LayoutSnapshot,
    scene: SceneSnapshot,
    resources: ResourceSnapshot,
    semantics: SemanticSnapshot,
}

impl CpuSnapshot {
    pub fn new(
        layout: LayoutSnapshot,
        scene: SceneSnapshot,
        resources: ResourceSnapshot,
        semantics: SemanticSnapshot,
    ) -> Result<Self> {
        let snapshot = Self {
            layout,
            scene,
            resources,
            semantics,
        };
        snapshot.validate_revisions()?;
        Ok(snapshot)
    }

    pub fn empty(revisions: RevisionSet) -> Self {
        Self {
            layout: LayoutSnapshot::empty(revisions.layout),
            scene: SceneSnapshot::empty(revisions.scene),
            resources: ResourceSnapshot::empty(revisions.resource),
            semantics: SemanticSnapshot::empty(revisions.semantic),
        }
    }

    pub const fn layout(&self) -> &LayoutSnapshot {
        &self.layout
    }

    pub const fn scene(&self) -> &SceneSnapshot {
        &self.scene
    }

    pub const fn resources(&self) -> &ResourceSnapshot {
        &self.resources
    }

    pub const fn semantics(&self) -> &SemanticSnapshot {
        &self.semantics
    }

    pub const fn revisions(&self) -> RevisionSet {
        RevisionSet::new(
            self.layout.revision(),
            self.scene.revision(),
            self.resources.revision(),
            self.semantics.revision(),
        )
    }

    fn validate_revisions(&self) -> Result<()> {
        // Component revisions are read directly into the tuple, so this method
        // is also the single place to add cross-component consistency checks.
        self.layout.viewport().validate().map_err(Error::from)
    }

    fn transition_is_valid(&self, older: &Self) -> Result<()> {
        let old = older.revisions();
        let new = self.revisions();
        if !new.does_not_regress_from(old) {
            return Err(Error::compile(
                "snapshot_commit",
                "a candidate snapshot regresses a committed revision",
            ));
        }

        validate_component(
            "layout",
            old.layout.get(),
            new.layout.get(),
            self.layout.observable_eq(&older.layout),
        )?;
        validate_component(
            "scene",
            old.scene.get(),
            new.scene.get(),
            self.scene.observable_eq(&older.scene),
        )?;
        validate_component(
            "resource",
            old.resource.get(),
            new.resource.get(),
            self.resources.observable_eq(&older.resources),
        )?;
        validate_component(
            "semantic",
            old.semantic.get(),
            new.semantic.get(),
            self.semantics.observable_eq(&older.semantics),
        )?;
        Ok(())
    }
}

fn validate_component(
    name: &'static str,
    old_revision: u64,
    new_revision: u64,
    same_observable_output: bool,
) -> Result<()> {
    if same_observable_output && old_revision != new_revision {
        return Err(Error::compile(
            "snapshot_commit",
            format!("{name} revision changed without an observable output change"),
        ));
    }
    if !same_observable_output && new_revision <= old_revision {
        return Err(Error::compile(
            "snapshot_commit",
            format!("{name} output changed without advancing its revision"),
        ));
    }
    Ok(())
}

/// Outcome returned only after a candidate has passed all consistency checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitReceipt {
    pub previous: Option<RevisionSet>,
    pub current: RevisionSet,
    pub replaced: bool,
}

/// Atomic store retaining the last valid snapshot after any compile failure.
#[derive(Clone, Debug, Default)]
pub struct AtomicSnapshotStore {
    committed: Option<Arc<CpuSnapshot>>,
    rejected_candidates: u64,
}

impl AtomicSnapshotStore {
    pub fn committed(&self) -> Option<Arc<CpuSnapshot>> {
        self.committed.clone()
    }

    pub const fn rejected_candidates(&self) -> u64 {
        self.rejected_candidates
    }

    pub fn try_commit(&mut self, candidate: CpuSnapshot) -> Result<CommitReceipt> {
        let result = self.commit_validated(candidate);
        if result.is_err() {
            self.rejected_candidates = self.rejected_candidates.saturating_add(1);
        }
        result
    }

    fn commit_validated(&mut self, candidate: CpuSnapshot) -> Result<CommitReceipt> {
        candidate.validate_revisions()?;
        if let Some(previous) = self.committed.as_deref() {
            candidate.transition_is_valid(previous)?;
            let receipt = CommitReceipt {
                previous: Some(previous.revisions()),
                current: candidate.revisions(),
                replaced: true,
            };
            self.committed = Some(Arc::new(candidate));
            Ok(receipt)
        } else {
            let receipt = CommitReceipt {
                previous: None,
                current: candidate.revisions(),
                replaced: false,
            };
            self.committed = Some(Arc::new(candidate));
            Ok(receipt)
        }
    }

    pub fn compile_and_commit(
        &mut self,
        compile: impl FnOnce() -> Result<CpuSnapshot>,
    ) -> Result<CommitReceipt> {
        let candidate = match compile() {
            Ok(candidate) => candidate,
            Err(error) => {
                self.rejected_candidates = self.rejected_candidates.saturating_add(1);
                return Err(error);
            }
        };
        self.try_commit(candidate)
    }
}

/// Read-only window metadata returned to platform adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub spec: WindowSpec,
    pub frame_requested: bool,
    pub committed_revisions: Option<RevisionSet>,
}

/// P0's owned, backend-neutral window context view.
pub type WindowContext = WindowInfo;

struct WindowState {
    spec: WindowSpec,
    snapshots: AtomicSnapshotStore,
    frame_requested: bool,
    elements: ElementTree,
    events: EventDispatcher,
    view: Option<Rc<dyn View>>,
}

/// Result of one complete capture/target/bubble dispatch and transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDispatchReceipt {
    pub outcome: DispatchOutcome,
    pub transaction: TxnReceipt,
    pub reconciliation: Option<ReconcileReport>,
}

/// Application work completed after one transaction was atomically published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationTxnReceipt {
    pub transaction: TxnReceipt,
    pub reconciliations: Vec<(WindowId, ReconcileReport)>,
}

/// Result of draining generation/revision-stamped worker messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundDispatchReceipt {
    pub accepted: usize,
    pub stale: usize,
    /// `None` when every drained message was stale (or the inbox was empty).
    pub application: Option<ApplicationTxnReceipt>,
}

/// Headless application runtime and window scheduler.
pub struct Application {
    owner: UiThread,
    windows: DenseArena<WindowState, WindowId>,
    main_window: Option<WindowId>,
    // Keep the public type explicitly non-Send even if internal fields change.
    _not_send: PhantomData<Rc<()>>,
}

impl fmt::Debug for Application {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Application")
            .field("main_window", &self.main_window)
            .field("window_count", &self.windows.len())
            .finish()
    }
}

impl Application {
    pub fn new() -> Self {
        Self {
            owner: UiThread::current(),
            windows: DenseArena::new(),
            main_window: None,
            _not_send: PhantomData,
        }
    }

    pub fn owner(&self) -> &UiThread {
        &self.owner
    }

    pub fn create_window(&mut self, spec: WindowSpec) -> Result<WindowId> {
        self.owner.assert_current()?;
        spec.validate()?;
        let id = self.windows.insert(WindowState {
            spec,
            snapshots: AtomicSnapshotStore::default(),
            frame_requested: false,
            elements: ElementTree::new(),
            events: EventDispatcher::new(),
            view: None,
        });
        if self.main_window.is_none() {
            self.main_window = Some(id);
        }
        Ok(id)
    }

    pub fn main_window(&self) -> Option<WindowId> {
        self.main_window
    }

    pub fn window_info(&self, id: WindowId) -> Option<WindowInfo> {
        let state = self.windows.get(id)?;
        Some(WindowInfo {
            id,
            spec: state.spec.clone(),
            frame_requested: state.frame_requested,
            committed_revisions: state
                .snapshots
                .committed()
                .map(|snapshot| snapshot.revisions()),
        })
    }

    pub fn window_context(&self, id: WindowId) -> Option<WindowContext> {
        self.window_info(id)
    }

    pub fn windows(&self) -> Vec<WindowInfo> {
        self.windows
            .ids()
            .filter_map(|id| self.window_info(id))
            .collect()
    }

    pub fn request_frame(&mut self, id: WindowId) -> Result<()> {
        self.owner.assert_current()?;
        let state = self.windows.get_mut(id).ok_or_else(|| {
            Error::invalid_input(
                Some("window_id".to_owned()),
                "window ID is stale or unknown",
            )
        })?;
        state.frame_requested = true;
        Ok(())
    }

    /// Installs a retained view builder and performs its initial reactive build.
    pub fn set_view<V>(&mut self, id: WindowId, view: V) -> Result<ReconcileReport>
    where
        V: View + 'static,
    {
        self.owner.assert_current()?;
        let view: Rc<dyn View> = Rc::new(view);
        let state = self.window_state_mut(id)?;
        if !state.elements.is_empty() && !state.elements.has_view_root() {
            return Err(Error::invalid_input(
                Some("view".to_owned()),
                "a direct widget root is already mounted",
            ));
        }
        let report = state.elements.rebuild_view(view.as_ref())?;
        state
            .events
            .elements_unmounted(report.removed_ids().iter().copied());
        state.events.reconcile_owners(&state.elements);
        state.view = Some(view);
        state.frame_requested = true;
        Ok(report)
    }

    /// Rebuilds the installed view while replacing its build dependencies.
    pub fn rebuild_view(&mut self, id: WindowId) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let state = self.window_state_mut(id)?;
        let view = state.view.clone().ok_or_else(|| {
            Error::invalid_input(Some("view".to_owned()), "the window has no installed view")
        })?;
        let report = state.elements.rebuild_view(view.as_ref())?;
        state
            .events
            .elements_unmounted(report.removed_ids().iter().copied());
        state.events.reconcile_owners(&state.elements);
        state.frame_requested = true;
        Ok(report)
    }

    /// Mounts a static declaration without storing a reactive view builder.
    pub fn mount_widget(&mut self, id: WindowId, widget: WidgetNode) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let state = self.window_state_mut(id)?;
        if state.view.is_some() {
            return Err(Error::invalid_input(
                Some("widget".to_owned()),
                "the window already owns a reactive view",
            ));
        }
        let report = state.elements.mount(widget)?;
        state.events.reconcile_owners(&state.elements);
        state.frame_requested = true;
        Ok(report)
    }

    pub fn reconcile_widget(
        &mut self,
        id: WindowId,
        widget: WidgetNode,
    ) -> Result<ReconcileReport> {
        self.owner.assert_current()?;
        let state = self.window_state_mut(id)?;
        if state.view.is_some() {
            return Err(Error::invalid_input(
                Some("widget".to_owned()),
                "use rebuild_view for a window with an installed view",
            ));
        }
        let report = state.elements.reconcile(widget)?;
        state
            .events
            .elements_unmounted(report.removed_ids().iter().copied());
        state.events.reconcile_owners(&state.elements);
        state.frame_requested = true;
        Ok(report)
    }

    pub fn widget_stats(&self, id: WindowId) -> Option<ElementTreeStats> {
        self.windows.get(id).map(|state| state.elements.stats())
    }

    pub fn element_diagnostics(&self, id: WindowId) -> Option<Vec<ElementNodeDiagnostics>> {
        self.windows
            .get(id)
            .map(|state| state.elements.diagnostics())
    }

    pub fn focused_element(&self, id: WindowId) -> Option<ElementId> {
        self.windows.get(id)?.events.focused()
    }

    /// Routes one event using the previous committed hit result, commits its
    /// shared transaction, and rebuilds an installed view when build state was
    /// invalidated.
    pub fn dispatch_event(
        &mut self,
        id: WindowId,
        committed_hit: CommittedHitTarget,
        event: &UiEvent,
    ) -> Result<EventDispatchReceipt> {
        self.owner.assert_current()?;
        if committed_hit.window() != Some(id) {
            return Err(Error::invalid_input(
                Some("hit_test_window".to_owned()),
                "event hit target belongs to a different window or is unscoped",
            ));
        }
        if let UiEvent::AccessibilityAction(action) = event {
            if action.window() != Some(id) {
                return Err(Error::invalid_input(
                    Some("accessibility_window".to_owned()),
                    "accessibility action belongs to a different window or is unscoped",
                ));
            }
        }
        if let UiEvent::WindowResized(size) = event {
            size.validate().map_err(Error::from)?;
        }

        let (outcome, events_before, mut pending) = {
            let state = self.window_state_mut(id)?;
            let committed_layout_revision = state
                .snapshots
                .committed()
                .map(|snapshot| snapshot.layout().revision())
                .unwrap_or(crate::core::LayoutRevision::ZERO);
            if committed_hit.revision() != committed_layout_revision {
                return Err(Error::invalid_input(
                    Some("hit_test_revision".to_owned()),
                    "event hit target does not come from the latest committed layout",
                ));
            }
            let mut pending = UpdateTxn::<UiCommand>::new();
            let events_before = state.events.clone();
            let outcome =
                state
                    .events
                    .dispatch(&state.elements, committed_hit, event, &mut pending)?;
            (outcome, events_before, pending)
        };

        if matches!(event, UiEvent::WindowCloseRequested) && !outcome.default_prevented {
            pending.push(UiCommand::CloseWindow(id))?;
        }

        let (transaction, commands) = match self.commit_update_txn(pending) {
            Ok(committed) => committed,
            Err(error) => {
                if let Some(state) = self.windows.get_mut(id) {
                    state.events = events_before;
                }
                return Err(error);
            }
        };

        if let Some(state) = self.windows.get_mut(id) {
            match event {
                UiEvent::WindowResized(size) => state.spec.inner_size = *size,
                UiEvent::WindowDpiChanged(scale) => state.spec.dpi_scale = *scale,
                _ => {}
            }
        }

        let reconciliations = self.route_committed_transaction(&transaction, commands)?;
        let current_reconciliation = reconciliations
            .into_iter()
            .find_map(|(window, report)| (window == id).then_some(report));
        Ok(EventDispatchReceipt {
            outcome,
            transaction,
            reconciliation: current_reconciliation,
        })
    }

    /// Atomically commits a UI transaction, then routes every resulting
    /// invalidation and command through the application scheduler.
    pub fn apply_transaction(
        &mut self,
        pending: UpdateTxn<UiCommand>,
    ) -> Result<ApplicationTxnReceipt> {
        self.owner.assert_current()?;
        let (transaction, commands) = self.commit_update_txn(pending)?;
        let reconciliations = self.route_committed_transaction(&transaction, commands)?;
        Ok(ApplicationTxnReceipt {
            transaction,
            reconciliations,
        })
    }

    /// Drains worker results for one live source generation.
    ///
    /// A result is accepted only when its target window still exists, its
    /// source generation equals `current_source`, and every revision selected
    /// by its [`crate::state::RevisionMask`] still matches that target's latest
    /// committed snapshot. Accepted payloads are staged into one transaction;
    /// stale payloads never reach `stage` and cannot modify application state.
    pub fn consume_background_results<T>(
        &mut self,
        inbox: &UiInbox<T>,
        current_source: GenerationStamp,
        mut stage: impl FnMut(WindowId, T, &mut UpdateTxn<UiCommand>) -> Result<()>,
    ) -> Result<BackgroundDispatchReceipt> {
        self.owner.assert_current()?;
        let messages = inbox.drain()?;
        let mut pending = UpdateTxn::new();
        let mut accepted = 0;
        let mut stale = 0;

        for message in messages {
            let Some(state) = self.windows.get(message.target) else {
                stale += 1;
                continue;
            };
            let current_revisions = state
                .snapshots
                .committed()
                .map(|snapshot| snapshot.revisions())
                .unwrap_or(RevisionSet::ZERO);
            if !current_source.is_well_formed()
                || !message.is_current(message.target, current_source, current_revisions)
            {
                stale += 1;
                continue;
            }

            stage(message.target, message.payload, &mut pending)?;
            accepted += 1;
        }

        let application = if accepted == 0 {
            None
        } else {
            Some(self.apply_transaction(pending)?)
        };
        Ok(BackgroundDispatchReceipt {
            accepted,
            stale,
            application,
        })
    }

    fn commit_update_txn(
        &self,
        pending: UpdateTxn<UiCommand>,
    ) -> Result<(TxnReceipt, Vec<UiCommand>)> {
        let live_windows = self.windows.ids().collect::<Vec<_>>();
        let mut commands = Vec::new();
        let transaction = pending.commit(|batch| {
            for command in &batch {
                let target = match command {
                    UiCommand::RequestFrame(window) | UiCommand::CloseWindow(window) => *window,
                };
                if !live_windows.contains(&target) {
                    return Err(Error::invalid_input(
                        Some("command_window".to_owned()),
                        "transaction command targets a stale or unknown window",
                    ));
                }
            }
            commands = batch;
            Ok(())
        })?;
        Ok((transaction, commands))
    }

    fn route_committed_transaction(
        &mut self,
        transaction: &TxnReceipt,
        commands: Vec<UiCommand>,
    ) -> Result<Vec<(WindowId, ReconcileReport)>> {
        let mut reconciliations = Vec::new();
        let mut first_error = None;
        for (window, state) in self.windows.iter_mut() {
            let owner = state.elements.dependency_owner();
            let mut affected = false;
            let mut rebuild = false;
            for invalidation in transaction.invalidations() {
                if invalidation.owner() == owner && state.elements.contains(invalidation.element())
                {
                    affected = true;
                    rebuild |= invalidation.phase() == crate::state::DependencyPhase::Build;
                }
            }
            if !affected {
                continue;
            }
            state.frame_requested = true;
            if rebuild {
                if let Some(view) = state.view.clone() {
                    match state.elements.rebuild_view(view.as_ref()) {
                        Ok(report) => {
                            state
                                .events
                                .elements_unmounted(report.removed_ids().iter().copied());
                            state.events.reconcile_owners(&state.elements);
                            reconciliations.push((window, report));
                        }
                        Err(error) => {
                            if first_error.is_none() {
                                first_error = Some(error);
                            }
                        }
                    }
                }
            }
        }

        let mut close_windows = Vec::new();
        for command in commands {
            match command {
                UiCommand::RequestFrame(window) => {
                    if let Err(error) = self.request_frame(window) {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }
                UiCommand::CloseWindow(window) => close_windows.push(window),
            }
        }
        for window in close_windows {
            if let Err(error) = self.destroy_window(window) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(reconciliations)
    }

    pub fn take_frame_requests(&mut self) -> Result<Vec<WindowId>> {
        self.owner.assert_current()?;
        let mut requested = Vec::new();
        for (id, state) in self.windows.iter_mut() {
            if state.frame_requested {
                state.frame_requested = false;
                requested.push(id);
            }
        }
        Ok(requested)
    }

    pub fn committed_snapshot(&self, id: WindowId) -> Option<Arc<CpuSnapshot>> {
        self.windows.get(id)?.snapshots.committed()
    }

    pub fn commit_snapshot(
        &mut self,
        id: WindowId,
        snapshot: CpuSnapshot,
    ) -> Result<CommitReceipt> {
        self.owner.assert_current()?;
        let state = self.windows.get_mut(id).ok_or_else(|| {
            Error::invalid_input(
                Some("window_id".to_owned()),
                "window ID is stale or unknown",
            )
        })?;
        state.snapshots.try_commit(snapshot)
    }

    pub fn compile_and_commit(
        &mut self,
        id: WindowId,
        compile: impl FnOnce() -> Result<CpuSnapshot>,
    ) -> Result<CommitReceipt> {
        self.owner.assert_current()?;
        let state = self.windows.get_mut(id).ok_or_else(|| {
            Error::invalid_input(
                Some("window_id".to_owned()),
                "window ID is stale or unknown",
            )
        })?;
        state.snapshots.compile_and_commit(compile)
    }

    pub fn rejected_snapshot_count(&self, id: WindowId) -> Option<u64> {
        self.windows
            .get(id)
            .map(|state| state.snapshots.rejected_candidates())
    }

    pub fn destroy_window(&mut self, id: WindowId) -> Result<bool> {
        self.owner.assert_current()?;
        let existed = self.windows.remove(id).is_some();
        if existed && self.main_window == Some(id) {
            self.main_window = self.windows.ids().next();
        }
        Ok(existed)
    }

    pub fn dispatcher<T: Send + 'static>(&self) -> (UiDispatcher<T>, UiInbox<T>) {
        // The receiver records this same owner thread. No application reference
        // crosses into the worker sender.
        crate::state::ui_channel()
    }

    pub fn empty_snapshot(&self) -> CpuSnapshot {
        CpuSnapshot::empty(RevisionSet::ZERO)
    }

    fn window_state_mut(&mut self, id: WindowId) -> Result<&mut WindowState> {
        self.windows.get_mut(id).ok_or_else(|| {
            Error::invalid_input(
                Some("window_id".to_owned()),
                "window ID is stale or unknown",
            )
        })
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

/// Alias documenting that a store is the commit boundary, not a second UI tree.
pub type SnapshotStore = AtomicSnapshotStore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        LayoutRevision, Point, ResourceRevision, RevisionChanges, SceneRevision, SemanticRevision,
        WidgetKey,
    };
    use crate::event::{
        AccessibilityAction, AccessibilityActionEvent, EventHandler, EventPhase, PointerEvent,
        PointerId, PointerKind,
    };
    use crate::state::{RevisionMask, State};
    use crate::widget::{BuildContext, Widget};
    use crate::widgets::{Button, Container};
    use std::cell::Cell;

    fn snapshot(revisions: RevisionSet, fingerprint: u64) -> CpuSnapshot {
        CpuSnapshot::new(
            LayoutSnapshot::new(revisions.layout, Size::new(100.0, 50.0), 1, fingerprint).unwrap(),
            SceneSnapshot::new(revisions.scene, 1, fingerprint),
            ResourceSnapshot::new(revisions.resource, [], fingerprint),
            SemanticSnapshot::new(revisions.semantic, 1, fingerprint),
        )
        .unwrap()
    }

    #[test]
    fn application_supports_multiple_windows_and_stale_ids() {
        let mut application = Application::new();
        let first = application.create_window(WindowSpec::new("first")).unwrap();
        let second = application
            .create_window(WindowSpec::new("second"))
            .unwrap();
        assert_eq!(application.main_window(), Some(first));
        assert_eq!(application.windows().len(), 2);
        assert!(application.destroy_window(first).unwrap());
        assert_eq!(application.main_window(), Some(second));
        assert!(!application.destroy_window(first).unwrap());
    }

    #[test]
    fn failed_compile_preserves_the_last_committed_snapshot() {
        let mut store = AtomicSnapshotStore::default();
        let first = snapshot(RevisionSet::ZERO, 10);
        store.try_commit(first).unwrap();
        let before = store.committed().unwrap();

        let failed = store.compile_and_commit(|| Err(Error::compile("test", "deliberate failure")));
        assert!(failed.is_err());
        assert_eq!(store.committed().unwrap().as_ref(), before.as_ref());
        assert_eq!(store.rejected_candidates(), 1);
    }

    #[test]
    fn changed_observable_output_requires_its_revision() {
        let mut store = AtomicSnapshotStore::default();
        store.try_commit(snapshot(RevisionSet::ZERO, 1)).unwrap();
        let invalid = snapshot(RevisionSet::ZERO, 2);
        assert!(store.try_commit(invalid).is_err());
        assert_eq!(store.rejected_candidates(), 1);

        let mut revisions = RevisionSet::ZERO;
        revisions.advance(RevisionChanges::ALL).unwrap();
        assert_eq!(revisions.scene, SceneRevision::new(1));
        store.try_commit(snapshot(revisions, 2)).unwrap();
    }

    #[derive(Clone)]
    struct ReorderView {
        reversed: State<bool>,
    }

    impl View for ReorderView {
        fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode> {
            let reversed = context.read_state(&self.reversed)?;
            let state = self.reversed.clone();
            let toggle = EventHandler::new(1, move |event, context| {
                if context.phase() == EventPhase::Target && matches!(event, UiEvent::PointerDown(_))
                {
                    state.update(context.transaction(), |value| *value = !*value)?;
                }
                Ok(())
            });
            let a = Button::new("a").with_key("a").build(context)?;
            let b = Button::new("b")
                .with_key("b")
                .with_event_handler(toggle)
                .build(context)?;
            let children = if reversed { vec![b, a] } else { vec![a, b] };
            Container::new().with_children(children).build(context)
        }
    }

    #[test]
    fn event_state_commit_rebuilds_view_and_preserves_keyed_focus() {
        let reversed = State::new(false);
        let mut application = Application::new();
        let window = application.create_window(WindowSpec::new("P1")).unwrap();
        application
            .set_view(
                window,
                ReorderView {
                    reversed: reversed.clone(),
                },
            )
            .unwrap();
        let button_key = WidgetKey::from("b");
        let before = application
            .element_diagnostics(window)
            .unwrap()
            .into_iter()
            .find(|element| element.key.as_ref() == Some(&button_key))
            .unwrap()
            .id;

        let pointer = UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            Point::new(10.0, 10.0),
        ));
        let receipt = application
            .dispatch_event(
                window,
                CommittedHitTarget::for_window(
                    window,
                    crate::core::LayoutRevision::ZERO,
                    Some(before),
                ),
                &pointer,
            )
            .unwrap();
        assert_eq!(receipt.transaction.changed_state_count, 1);
        assert!(receipt.reconciliation.is_some());
        assert!(reversed.get().unwrap());

        let after = application
            .element_diagnostics(window)
            .unwrap()
            .into_iter()
            .find(|element| element.key.as_ref() == Some(&button_key))
            .unwrap()
            .id;
        assert_eq!(after, before);
        assert_eq!(application.focused_element(window), Some(before));

        // The rebuild replaced, rather than accumulated, its dependency set.
        // A second event through the same stable ID still invalidates/rebuilds.
        let second = application
            .dispatch_event(
                window,
                CommittedHitTarget::for_window(
                    window,
                    crate::core::LayoutRevision::ZERO,
                    Some(before),
                ),
                &pointer,
            )
            .unwrap();
        assert_eq!(second.transaction.invalidations().len(), 1);
        assert!(!reversed.get().unwrap());
        assert_eq!(application.focused_element(window), Some(before));
    }

    #[test]
    fn close_request_is_a_preventable_transactional_default() {
        struct Root;
        let prevent = EventHandler::new(1, |event, context| {
            if matches!(event, UiEvent::WindowCloseRequested) {
                context.prevent_default();
            }
            Ok(())
        });
        let mut application = Application::new();
        let window = application.create_window(WindowSpec::new("close")).unwrap();
        application
            .mount_widget(
                window,
                WidgetNode::new::<Root>().with_event_handler(prevent),
            )
            .unwrap();
        let committed =
            CommittedHitTarget::miss_for_window(window, crate::core::LayoutRevision::ZERO);
        let prevented = application
            .dispatch_event(window, committed, &UiEvent::WindowCloseRequested)
            .unwrap();
        assert!(prevented.outcome.default_prevented);
        assert!(application.window_info(window).is_some());

        application
            .reconcile_widget(window, WidgetNode::new::<Root>())
            .unwrap();
        application
            .dispatch_event(window, committed, &UiEvent::WindowCloseRequested)
            .unwrap();
        assert!(application.window_info(window).is_none());
    }

    #[derive(Clone)]
    struct SharedView {
        value: State<u32>,
        builds: Rc<Cell<usize>>,
    }

    impl View for SharedView {
        fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode> {
            self.builds.set(self.builds.get() + 1);
            let value = context.read_state(&self.value)?;
            let state = self.value.clone();
            Button::new(format!("value {value}"))
                .with_key("shared")
                .with_event_handler(EventHandler::new(1, move |event, context| {
                    if context.phase() == EventPhase::Target
                        && matches!(event, UiEvent::PointerDown(_))
                    {
                        state.update(context.transaction(), |value| *value += 1)?;
                    }
                    Ok(())
                }))
                .build(context)
        }
    }

    #[test]
    fn shared_state_invalidations_are_namespaced_and_rebuild_every_window() {
        let value = State::new(0_u32);
        let first_builds = Rc::new(Cell::new(0));
        let second_builds = Rc::new(Cell::new(0));
        let mut application = Application::new();
        let first = application.create_window(WindowSpec::new("first")).unwrap();
        let second = application
            .create_window(WindowSpec::new("second"))
            .unwrap();
        application
            .set_view(
                first,
                SharedView {
                    value: value.clone(),
                    builds: first_builds.clone(),
                },
            )
            .unwrap();
        application
            .set_view(
                second,
                SharedView {
                    value: value.clone(),
                    builds: second_builds.clone(),
                },
            )
            .unwrap();
        application.take_frame_requests().unwrap();

        let key = WidgetKey::from("shared");
        let button = application
            .element_diagnostics(first)
            .unwrap()
            .into_iter()
            .find(|element| element.key.as_ref() == Some(&key))
            .unwrap()
            .id;
        let event = UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            Point::ZERO,
        ));

        let wrong_window =
            CommittedHitTarget::for_window(second, crate::core::LayoutRevision::ZERO, Some(button));
        assert!(
            application
                .dispatch_event(first, wrong_window, &event)
                .is_err()
        );
        assert_eq!(value.get().unwrap(), 0);

        let receipt = application
            .dispatch_event(
                first,
                CommittedHitTarget::for_window(
                    first,
                    crate::core::LayoutRevision::ZERO,
                    Some(button),
                ),
                &event,
            )
            .unwrap();
        assert_eq!(receipt.transaction.invalidations().len(), 2);
        assert_eq!(first_builds.get(), 2);
        assert_eq!(second_builds.get(), 2);
        let mut frames = application.take_frame_requests().unwrap();
        frames.sort_by_key(|window| (window.slot(), window.generation()));
        let mut expected = vec![first, second];
        expected.sort_by_key(|window| (window.slot(), window.generation()));
        assert_eq!(frames, expected);
    }

    #[test]
    fn rejected_event_commands_roll_back_state_and_input_ownership() {
        struct Root;
        let value = State::new(0_u32);
        let handler_state = value.clone();
        let handler = EventHandler::new(1, move |event, context| {
            if context.phase() == EventPhase::Target && matches!(event, UiEvent::PointerDown(_)) {
                handler_state.set(context.transaction(), 1)?;
                context.command(UiCommand::RequestFrame(WindowId::from_parts(999, 1)))?;
            }
            Ok(())
        });
        let mut application = Application::new();
        let window = application
            .create_window(WindowSpec::new("atomic"))
            .unwrap();
        application
            .mount_widget(
                window,
                WidgetNode::new::<Root>()
                    .with_focusable(true)
                    .with_event_handler(handler),
            )
            .unwrap();
        let root = application.element_diagnostics(window).unwrap()[0].id;
        let event = UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            Point::ZERO,
        ));
        let result = application.dispatch_event(
            window,
            CommittedHitTarget::for_window(window, crate::core::LayoutRevision::ZERO, Some(root)),
            &event,
        );
        assert!(result.is_err());
        assert_eq!(value.get().unwrap(), 0);
        assert_eq!(application.focused_element(window), None);
    }

    #[test]
    fn worker_results_validate_stamps_and_rebuild_every_subscribed_window() {
        let value = State::new(0_u32);
        let first_builds = Rc::new(Cell::new(0));
        let second_builds = Rc::new(Cell::new(0));
        let mut application = Application::new();
        let first = application.create_window(WindowSpec::new("first")).unwrap();
        let second = application
            .create_window(WindowSpec::new("second"))
            .unwrap();
        let stale_target = application.create_window(WindowSpec::new("stale")).unwrap();
        application
            .set_view(
                first,
                SharedView {
                    value: value.clone(),
                    builds: first_builds.clone(),
                },
            )
            .unwrap();
        application
            .set_view(
                second,
                SharedView {
                    value: value.clone(),
                    builds: second_builds.clone(),
                },
            )
            .unwrap();
        application.destroy_window(stale_target).unwrap();
        application.take_frame_requests().unwrap();

        let source = GenerationStamp::new(17, 3);
        let old_source = GenerationStamp::new(17, 2);
        let irrelevant_layout = RevisionSet::new(
            LayoutRevision::new(99),
            SceneRevision::ZERO,
            ResourceRevision::ZERO,
            SemanticRevision::ZERO,
        );
        let stale_resource = RevisionSet::new(
            LayoutRevision::ZERO,
            SceneRevision::ZERO,
            ResourceRevision::new(1),
            SemanticRevision::ZERO,
        );
        let (dispatcher, inbox) = application.dispatcher::<u32>();
        dispatcher
            .dispatch_with_mask(first, source, irrelevant_layout, RevisionMask::RESOURCE, 7)
            .unwrap();
        dispatcher
            .dispatch_with_mask(
                first,
                old_source,
                RevisionSet::ZERO,
                RevisionMask::NONE,
                101,
            )
            .unwrap();
        dispatcher
            .dispatch_with_mask(first, source, stale_resource, RevisionMask::RESOURCE, 102)
            .unwrap();
        dispatcher
            .dispatch(stale_target, source, RevisionSet::ZERO, 103)
            .unwrap();

        let stage_calls = Cell::new(0);
        let state = value.clone();
        let receipt = application
            .consume_background_results(&inbox, source, |target, payload, transaction| {
                assert_eq!(target, first);
                stage_calls.set(stage_calls.get() + 1);
                state.set(transaction, payload)
            })
            .unwrap();

        assert_eq!(receipt.accepted, 1);
        assert_eq!(receipt.stale, 3);
        assert_eq!(stage_calls.get(), 1);
        assert_eq!(value.get().unwrap(), 7);
        let committed = receipt.application.unwrap();
        assert_eq!(committed.transaction.changed_state_count, 1);
        assert_eq!(committed.transaction.invalidations().len(), 2);
        assert_eq!(committed.reconciliations.len(), 2);
        assert_eq!(first_builds.get(), 2);
        assert_eq!(second_builds.get(), 2);

        let mut frames = application.take_frame_requests().unwrap();
        frames.sort_by_key(|window| (window.slot(), window.generation()));
        let mut expected = vec![first, second];
        expected.sort_by_key(|window| (window.slot(), window.generation()));
        assert_eq!(frames, expected);
    }

    #[derive(Clone)]
    struct OwnershipView {
        mode: State<u8>,
    }

    struct OwnershipNode;

    impl View for OwnershipView {
        fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode> {
            let mode = context.read_state(&self.mode)?;
            Ok(WidgetNode::new::<OwnershipNode>()
                .with_key("owner")
                .with_focusable(mode != 1)
                .with_enabled(mode != 2)
                .with_event_handler(EventHandler::new(1, |event, context| {
                    if context.phase() == EventPhase::Target
                        && matches!(event, UiEvent::PointerDown(_))
                    {
                        context.capture_pointer(PointerId::MOUSE);
                    }
                    Ok(())
                })))
        }
    }

    #[test]
    fn reconciliation_revalidates_retained_focus_and_pointer_capture() {
        let mode = State::new(0_u8);
        let mut application = Application::new();
        let window = application
            .create_window(WindowSpec::new("ownership"))
            .unwrap();
        application
            .set_view(window, OwnershipView { mode: mode.clone() })
            .unwrap();
        let key = WidgetKey::from("owner");
        let owner = application
            .element_diagnostics(window)
            .unwrap()
            .into_iter()
            .find(|element| element.key.as_ref() == Some(&key))
            .unwrap()
            .id;
        let pointer_down = UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            Point::ZERO,
        ));
        let hit =
            CommittedHitTarget::for_window(window, crate::core::LayoutRevision::ZERO, Some(owner));
        application
            .dispatch_event(window, hit, &pointer_down)
            .unwrap();
        assert_eq!(application.focused_element(window), Some(owner));
        assert_eq!(
            application
                .windows
                .get(window)
                .unwrap()
                .events
                .pointer_capture(PointerId::MOUSE),
            Some(owner)
        );

        let mut non_focusable = UpdateTxn::new();
        mode.set(&mut non_focusable, 1).unwrap();
        application.apply_transaction(non_focusable).unwrap();
        assert_eq!(application.focused_element(window), None);
        assert_eq!(
            application
                .windows
                .get(window)
                .unwrap()
                .events
                .pointer_capture(PointerId::MOUSE),
            Some(owner)
        );

        let mut enabled = UpdateTxn::new();
        mode.set(&mut enabled, 0).unwrap();
        application.apply_transaction(enabled).unwrap();
        application
            .dispatch_event(window, hit, &pointer_down)
            .unwrap();
        assert_eq!(application.focused_element(window), Some(owner));

        let mut disabled = UpdateTxn::new();
        mode.set(&mut disabled, 2).unwrap();
        application.apply_transaction(disabled).unwrap();
        assert_eq!(application.focused_element(window), None);
        assert_eq!(
            application
                .windows
                .get(window)
                .unwrap()
                .events
                .pointer_capture(PointerId::MOUSE),
            None
        );
        let retained = application
            .element_diagnostics(window)
            .unwrap()
            .into_iter()
            .find(|element| element.key.as_ref() == Some(&key))
            .unwrap()
            .id;
        assert_eq!(retained, owner);
    }

    #[test]
    fn accessibility_actions_are_rejected_across_application_windows() {
        struct Root;
        let invocations = Rc::new(Cell::new(0));
        let handler_invocations = invocations.clone();
        let handler = EventHandler::new(1, move |event, context| {
            if context.phase() == EventPhase::Target
                && matches!(event, UiEvent::AccessibilityAction(_))
            {
                handler_invocations.set(handler_invocations.get() + 1);
            }
            Ok(())
        });
        let mut application = Application::new();
        let first = application.create_window(WindowSpec::new("first")).unwrap();
        let second = application
            .create_window(WindowSpec::new("second"))
            .unwrap();
        application
            .mount_widget(first, WidgetNode::new::<Root>().with_event_handler(handler))
            .unwrap();
        application
            .mount_widget(second, WidgetNode::new::<Root>())
            .unwrap();
        let first_root = application.element_diagnostics(first).unwrap()[0].id;
        let second_root = application.element_diagnostics(second).unwrap()[0].id;
        assert_eq!(first_root, second_root);
        let hit = CommittedHitTarget::for_window(
            first,
            crate::core::LayoutRevision::ZERO,
            Some(first_root),
        );

        let wrong_window = UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
            second,
            first_root,
            AccessibilityAction::Activate,
        ));
        assert!(
            application
                .dispatch_event(first, hit, &wrong_window)
                .is_err()
        );
        let unscoped = UiEvent::AccessibilityAction(AccessibilityActionEvent::new(
            first_root,
            AccessibilityAction::Activate,
        ));
        assert!(application.dispatch_event(first, hit, &unscoped).is_err());
        assert_eq!(invocations.get(), 0);

        let scoped = UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
            first,
            first_root,
            AccessibilityAction::Activate,
        ));
        application.dispatch_event(first, hit, &scoped).unwrap();
        assert_eq!(invocations.get(), 1);
    }
}
