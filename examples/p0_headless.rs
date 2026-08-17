use tgui::core::{DenseArena, ElementId, RevisionSet, Size};
use tgui::test_support::{FakeClock, TestRenderer};
use tgui::{Application, CpuSnapshot, WindowSpec};

fn main() -> tgui::Result<()> {
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P0 headless").with_inner_size(Size::new(640.0, 480.0)))?;
    let _window_context = application
        .window_context(window)
        .expect("new window has a context");

    let mut arena = DenseArena::<u32, ElementId>::new();
    let node = arena.insert(1);
    let _ = arena.remove(node);
    let replacement = arena.insert(2);
    assert!(!arena.contains(node));
    assert!(arena.contains(replacement));

    let renderer = TestRenderer::new();
    let commands = renderer.finish()?;
    let snapshot = CpuSnapshot::empty(RevisionSet::ZERO);
    application.commit_snapshot(window, snapshot)?;
    let metrics = tgui::diagnostics::FrameMetrics::empty(0, RevisionSet::ZERO);
    let clock = FakeClock::new();

    println!(
        "window={window:?} commands={} fingerprint={} nodes={} frame={} now_ns={}",
        commands.command_count(),
        commands.fingerprint(),
        arena.len(),
        metrics.frame_index,
        tgui::animation::FrameClock::now(&clock).as_nanos()
    );
    Ok(())
}
