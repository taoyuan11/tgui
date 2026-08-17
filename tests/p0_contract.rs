use std::thread;
use tgui::CpuSnapshot;
use tgui::accessibility::SemanticSnapshot;
use tgui::application::AtomicSnapshotStore;
use tgui::core::{
    Color, DenseArena, ElementId, LayoutRevision, Rect, ResourceRevision, RevisionSet,
    SceneRevision, SemanticRevision, Size, WindowId,
};
use tgui::diagnostics::{BudgetDomain, FixedBudgetResourceManager};
use tgui::layout::LayoutSnapshot;
use tgui::media::ResourceSnapshot;
use tgui::render::SceneSnapshot;
use tgui::state::ui_channel;
use tgui::test_support::TestRenderer;

fn snapshot(revisions: RevisionSet, scene_fingerprint: u64) -> CpuSnapshot {
    CpuSnapshot::new(
        LayoutSnapshot::new(revisions.layout, Size::new(320.0, 200.0), 1, 10).unwrap(),
        SceneSnapshot::new(revisions.scene, 1, scene_fingerprint),
        ResourceSnapshot::new(revisions.resource, [], 30),
        SemanticSnapshot::new(revisions.semantic, 1, 40),
    )
    .unwrap()
}

#[test]
fn public_arena_contract_rejects_a_reused_generation() {
    let mut arena = DenseArena::<&str, ElementId>::new();
    let old = arena.insert("old");
    assert_eq!(arena.remove(old), Some("old"));
    let current = arena.insert("current");
    assert_eq!(old.slot(), current.slot());
    assert_ne!(old.generation(), current.generation());
    assert_eq!(arena.get(old), None);
    assert_eq!(arena.get(current), Some(&"current"));
}

#[test]
fn snapshot_revisions_are_independent_and_failed_candidates_are_atomic() {
    let mut store = AtomicSnapshotStore::default();
    store.try_commit(snapshot(RevisionSet::ZERO, 1)).unwrap();

    let scene_changed = RevisionSet::new(
        LayoutRevision::ZERO,
        SceneRevision::new(1),
        ResourceRevision::ZERO,
        SemanticRevision::ZERO,
    );
    store.try_commit(snapshot(scene_changed, 2)).unwrap();
    let committed = store.committed().unwrap();

    let changed_without_revision = snapshot(scene_changed, 3);
    assert!(store.try_commit(changed_without_revision).is_err());
    assert_eq!(store.committed().unwrap().as_ref(), committed.as_ref());
    assert_eq!(store.rejected_candidates(), 1);
}

#[test]
fn only_the_dispatcher_crosses_to_a_worker_thread() {
    let (dispatcher, inbox) = ui_channel::<String>();
    let window = WindowId::from_parts(0, 1);
    let source = ElementId::from_parts(5, 3);
    thread::spawn(move || {
        dispatcher
            .dispatch(
                window,
                source.stamp(),
                RevisionSet::ZERO,
                "ready".to_owned(),
            )
            .unwrap();
    })
    .join()
    .unwrap();

    let batch = inbox
        .drain_valid(|message| message.source.matches(source))
        .unwrap();
    assert_eq!(batch.stale, 0);
    assert_eq!(batch.accepted[0].payload, "ready");
}

#[test]
fn budget_and_headless_renderer_work_without_backend_features() {
    let mut resources = FixedBudgetResourceManager::new(BudgetDomain::CpuCache, 8, 8).unwrap();
    resources.insert("first", vec![1_u8; 4], 4).unwrap();
    resources.insert("second", vec![2_u8; 4], 4).unwrap();
    resources.mark_in_flight(&"first");
    resources.insert("third", vec![3_u8; 4], 4).unwrap();
    assert!(resources.contains_key(&"first"));
    assert!(!resources.contains_key(&"second"));

    let mut renderer = TestRenderer::new();
    renderer
        .fill_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), Color::WHITE)
        .unwrap();
    let commands = renderer.snapshot().unwrap();
    assert_eq!(commands.command_count(), 1);
    assert!(commands.text().contains("#FFFFFFFF"));
}
