use tgui::application::{Application, WindowSpec};
use tgui::core::{DpiScale, LayoutRevision, Point, Size};
use tgui::layout::{
    Dimension, LayoutStyle, MeasureHandle, MeasureInput, MeasureOutput, MeasureSpec,
};
use tgui::test_support::LayoutHarness;
use tgui::widget::WidgetNode;

struct Root;
struct Measured;

#[test]
fn public_application_layout_commits_logical_geometry_and_hit_identity() {
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P2").with_inner_size(Size::new(200.0, 100.0)))
        .unwrap();
    application
        .mount_widget(
            window,
            WidgetNode::new::<Root>().with_layout_style(
                LayoutStyle::default().with_size(Dimension::Length(80.0), Dimension::Length(40.0)),
            ),
        )
        .unwrap();
    let element = application.element_diagnostics(window).unwrap()[0].id;
    let frame = application.layout_window(window).unwrap();

    assert_eq!(frame.snapshot.revision(), LayoutRevision::new(1));
    assert_eq!(
        frame.snapshot.node(element).unwrap().rect().size,
        Size::new(80.0, 40.0)
    );
    assert_eq!(
        application
            .hit_test(window, Point::new(10.0, 10.0))
            .unwrap()
            .target(),
        Some(element)
    );
}

#[test]
fn public_headless_comparator_covers_custom_measure_and_revision() {
    let measure = MeasureHandle::text(|_input: MeasureInput| {
        Ok(MeasureOutput::new(Size::new(50.0, 12.0)).with_baseline(9.0))
    });
    let mut harness = LayoutHarness::new();
    harness
        .mount(
            WidgetNode::new::<Measured>()
                .with_measure(MeasureSpec::new(measure))
                .with_key("measured"),
        )
        .unwrap();
    let comparison = harness
        .layout_and_compare(Size::new(100.0, 100.0), DpiScale::ONE)
        .unwrap();

    assert_eq!(comparison.incremental, comparison.rebuilt);
    assert_eq!(comparison.incremental.revision(), LayoutRevision::new(1));
    assert_eq!(comparison.incremental.nodes()[0].baseline(), Some(9.0));
}
