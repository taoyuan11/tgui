//! Application, window, revision, and atomic CPU-snapshot contracts.
//!
//! The P0 application is headless and supports multiple generational windows,
//! while exposing a convenient first/main window. All mutation methods assert
//! the creating UI thread through [`crate::state::UiThread`].

use crate::accessibility::SemanticSnapshot;
use crate::core::{DenseArena, DpiScale, Error, Result, RevisionSet, Size, WindowId};
use crate::layout::LayoutSnapshot;
use crate::media::ResourceSnapshot;
use crate::render::SceneSnapshot;
use crate::state::{UiDispatcher, UiInbox, UiThread};
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
    use crate::core::{RevisionChanges, SceneRevision};

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
}
