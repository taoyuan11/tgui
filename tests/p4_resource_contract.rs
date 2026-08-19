use tgui::core::{ResourceId, ResourceRevision};
use tgui::diagnostics::{BudgetDomain, CacheBudgetLimits, ResourceBudgetConfig};
use tgui::widget::WidgetNode;
use tgui::{Application, ResourceCompletion, WindowSpec};

struct ResourceOwner;

fn tiny_budgets() -> ResourceBudgetConfig {
    ResourceBudgetConfig::new(
        CacheBudgetLimits::new(8, 12),
        CacheBudgetLimits::new(16, 24),
        CacheBudgetLimits::new(4, 8),
    )
}

#[test]
fn window_budget_overrides_are_bounded_and_reported() {
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P4 budgets").with_resource_budgets(tiny_budgets()))
        .unwrap();

    let snapshots = application.resource_budget_snapshots(window).unwrap();
    assert_eq!(snapshots.cpu_cache.soft_limit_bytes, 8);
    assert_eq!(snapshots.gpu_cache.hard_limit_bytes, 24);
    assert_eq!(snapshots.transient_gpu.hard_limit_bytes, 8);

    application
        .reserve_resource_bytes(window, BudgetDomain::GpuCache, 20)
        .unwrap();
    assert!(
        application
            .reserve_resource_bytes(window, BudgetDomain::GpuCache, 5)
            .is_err()
    );
    let released = application
        .release_resource_bytes(window, BudgetDomain::GpuCache, 20)
        .unwrap();
    assert_eq!(released.current_bytes, 0);
    assert_eq!(released.peak_bytes, 20);
}

#[test]
fn stale_resource_completion_cannot_replace_the_current_binding() {
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P4 resource completion"))
        .unwrap();
    application
        .mount_widget(window, WidgetNode::new::<ResourceOwner>())
        .unwrap();
    let element = application.element_diagnostics(window).unwrap()[0].id;
    application.layout_window(window).unwrap();
    application.take_frame_requests().unwrap();

    let stale = application
        .begin_resource_request(window, element, ResourceId::from_parts(3, 1).stamp())
        .unwrap();
    let current = application
        .begin_resource_request(window, element, ResourceId::from_parts(3, 2).stamp())
        .unwrap();
    let texture = ResourceId::from_parts(9, 1);

    let dropped = application
        .complete_resource_request(ResourceCompletion::new(stale, [texture], 11))
        .unwrap();
    assert!(dropped.stale);
    assert!(!dropped.accepted);
    assert_eq!(dropped.revision, ResourceRevision::ZERO);

    let accepted = application
        .complete_resource_request(
            ResourceCompletion::new(current, [texture], 12)
                .with_intrinsic_size_changed(true)
                .with_upload_bytes(64),
        )
        .unwrap();
    assert!(accepted.accepted);
    assert!(accepted.observable_changed);
    assert_eq!(accepted.revision, ResourceRevision::new(1));
    let snapshot = application.committed_snapshot(window).unwrap();
    assert_eq!(snapshot.resources().references(), &[texture]);
    assert_eq!(application.take_frame_requests().unwrap(), vec![window]);

    let frame = application.layout_window(window).unwrap();
    assert_eq!(frame.metrics.dirty_roots.resource, 1);
    assert_eq!(frame.metrics.dirty_roots.layout, 1);
    assert_eq!(frame.metrics.resources.upload_bytes, 64);
    assert_eq!(frame.metrics.gpu_budget.unwrap().upload_bytes, 64);

    let resized = application
        .begin_resource_request(window, element, ResourceId::from_parts(3, 2).stamp())
        .unwrap();
    let resized = application
        .complete_resource_request(
            ResourceCompletion::new(resized, [texture], 12).with_intrinsic_size_changed(true),
        )
        .unwrap();
    assert!(resized.accepted);
    assert!(!resized.observable_changed);
    assert_eq!(resized.revision, ResourceRevision::new(1));
    assert_eq!(application.take_frame_requests().unwrap(), vec![window]);
    let frame = application.layout_window(window).unwrap();
    assert_eq!(frame.metrics.dirty_roots.resource, 1);
    assert_eq!(frame.metrics.dirty_roots.layout, 1);
}
