use tgui::core::{DpiScale, Size};
use tgui::layout::{Dimension, LayoutStyle};
use tgui::test_support::RenderHarness;
use tgui::widget::{BuildContext, Widget};
use tgui::widgets::{Button, Container};

fn main() -> tgui::Result<()> {
    let mut context = BuildContext::new();
    let button = Button::new("Compile").build(&mut context)?;
    let root = Container::new()
        .with_child(button)
        .build(&mut context)?
        .with_layout_style(
            LayoutStyle::default().with_size(Dimension::Length(160.0), Dimension::Length(48.0)),
        );

    let mut renderer = RenderHarness::new();
    renderer.mount(root)?;
    let frame = renderer.render(Size::new(320.0, 200.0), DpiScale::ONE)?;
    println!(
        "nodes={} chunks={} commands={} passes={} batches={} instances={} fingerprint={}",
        frame.tree.node_count,
        frame.scene.chunk_count(),
        frame.scene.command_count(),
        frame.compiled.passes,
        frame.compiled.batches,
        frame.compiled.quad_instances,
        frame.compiled.fingerprint,
    );
    Ok(())
}
