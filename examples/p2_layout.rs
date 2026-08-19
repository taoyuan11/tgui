use tgui::application::{Application, WindowSpec};
use tgui::core::{DpiScale, Point, Size};
use tgui::layout::{
    Dimension, FlexDirection, LayoutStyle, MeasureHandle, MeasureInput, MeasureOutput, MeasureSpec,
};
use tgui::test_support::LayoutHarness;
use tgui::widget::WidgetNode;

struct Root;
struct IntrinsicText;

fn main() -> tgui::Result<()> {
    let measure = MeasureHandle::text(|_input: MeasureInput| {
        Ok(MeasureOutput::new(Size::new(96.0, 20.0)).with_baseline(15.0))
    });
    let mut root_style =
        LayoutStyle::flex().with_size(Dimension::Length(320.0), Dimension::Length(180.0));
    root_style.flex_direction = FlexDirection::Column;
    let declaration = WidgetNode::new::<Root>()
        .with_layout_style(root_style)
        .with_child(
            WidgetNode::new::<IntrinsicText>()
                .with_key("text")
                .with_measure(MeasureSpec::new(measure)),
        );

    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P2 headless").with_inner_size(Size::new(320.0, 180.0)))?;
    application.mount_widget(window, declaration.clone())?;
    let frame = application.layout_window(window)?;
    let hit = application.hit_test(window, Point::new(10.0, 10.0))?;

    let mut harness = LayoutHarness::new();
    harness.mount(declaration)?;
    let comparison = harness.layout_and_compare(Size::new(320.0, 180.0), DpiScale::ONE)?;

    println!(
        "layout_revision={} nodes={} dirty_layout_roots={} hit={:?} equivalent={}",
        frame.snapshot.revision().get(),
        frame.snapshot.node_count(),
        frame.metrics.dirty_roots.layout,
        hit.target(),
        comparison.incremental == comparison.rebuilt,
    );
    Ok(())
}
