use tgui::core::{Color, DpiScale, Rect, ResourceId, SceneRevision, Size};
use tgui::layout::{Dimension, LayoutStyle};
use tgui::render::{
    Canvas, CompileContext, LayerSpec, PaintCommand, RenderCompiler, RendererCapabilities,
};
use tgui::test_support::RenderHarness;
use tgui::widget::{BuildContext, Widget};
use tgui::widgets::{Button, Container};
use tgui::{Application, WindowSpec};

#[test]
fn five_rectangles_compile_to_one_five_instance_quad_batch() {
    let mut canvas = Canvas::new();
    for index in 0..5 {
        canvas
            .fill_rect(
                Rect::from_xywh(index as f32 * 10.0, 0.0, 8.0, 8.0),
                Color::WHITE,
            )
            .unwrap();
    }
    let paint = canvas.snapshot().unwrap();
    let context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE)
        .with_scene_revision(SceneRevision::new(1));
    let scene = RenderCompiler::default()
        .compile(canvas.commands(), &context)
        .unwrap();

    assert_eq!(paint.command_count(), 5);
    assert_eq!(scene.batch_count(), 1);
    assert_eq!(scene.quad_instance_count(), 5);
    assert_eq!(scene.snapshot().paint_commands, 5);
}

#[test]
fn invalid_stacks_native_surfaces_and_transient_budget_are_atomic() {
    let context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE)
        .with_scene_revision(SceneRevision::new(1));
    let mut compiler = RenderCompiler::default();
    let good = [PaintCommand::FillRect {
        rect: Rect::from_xywh(0.0, 0.0, 2.0, 2.0),
        color: Color::WHITE,
    }];
    let committed = compiler.compile(&good, &context).unwrap();

    assert!(
        compiler
            .compile(&[PaintCommand::PopClip], &context)
            .is_err()
    );
    assert!(
        compiler
            .compile(
                &[PaintCommand::NativeSurface {
                    rect: Rect::from_xywh(0.0, 0.0, 2.0, 2.0),
                    surface: ResourceId::from_parts(1, 1),
                    opaque: true,
                }],
                &context,
            )
            .is_err()
    );
    let layer = [
        PaintCommand::BeginLayer(LayerSpec::new(Rect::from_xywh(0.0, 0.0, 100.0, 100.0))),
        good[0].clone(),
        PaintCommand::EndLayer,
    ];
    assert!(
        compiler
            .compile(&layer, &context.clone().with_transient_budget(64))
            .is_err()
    );
    assert_eq!(
        compiler.committed().unwrap().fingerprint,
        committed.fingerprint
    );
    assert_eq!(compiler.rejected_compiles(), 3);
}

#[test]
fn render_harness_collects_builtin_widgets_through_the_unified_pipeline() {
    let mut build = BuildContext::new();
    let button = Button::new("Render").build(&mut build).unwrap();
    let root = Container::new()
        .with_child(button)
        .build(&mut build)
        .unwrap()
        .with_layout_style(
            LayoutStyle::default().with_size(Dimension::Length(120.0), Dimension::Length(40.0)),
        );
    let mut harness = RenderHarness::new();
    harness.mount(root).unwrap();
    let frame = harness
        .render(Size::new(200.0, 100.0), DpiScale::ONE)
        .unwrap();

    assert_eq!(frame.tree.node_count, 3);
    assert_eq!(frame.scene.chunk_count(), 1);
    assert!(frame.scene.command_count() >= 2);
    assert!(frame.compiled.batches >= 2);
}

#[test]
fn application_commits_scene_atomically_without_false_revision_changes() {
    struct Custom;
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("P3").with_inner_size(Size::new(100.0, 50.0)))
        .unwrap();
    application
        .mount_widget(
            window,
            tgui::widget::WidgetNode::new::<Custom>().with_layout_style(
                LayoutStyle::default().with_size(Dimension::Length(40.0), Dimension::Length(20.0)),
            ),
        )
        .unwrap();

    let first = application.render_window(window).unwrap();
    let second = application.render_window(window).unwrap();
    assert_eq!(first.scene.revision(), SceneRevision::new(1));
    assert_eq!(second.scene.revision(), first.scene.revision());
    assert_eq!(second.scene.fingerprint(), first.scene.fingerprint());
    assert_eq!(
        application.compiled_scene(window).unwrap().scene_revision,
        SceneRevision::new(1)
    );
}
