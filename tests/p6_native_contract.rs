use tgui::core::{DpiScale, ElementId, Point, Rect, ResourceId, Size, WindowId};
use tgui::event::{PointerEvent, PointerId, PointerKind, UiEvent};
use tgui::native::{
    MockNativeHostCall, MockNativeHostConfig, MockNativeHostFactory, NativeCompositionStrategy,
    NativeHostCapabilities, NativeHostComposition, NativeHostCost, NativeHostCreateContext,
    NativeHostLayout, NativeHostLifecycle, NativeHostManager, NativeHostOutput,
    NativeHostScheduler, NativeHostWidget, ORDINARY_CONTROLS_MAY_USE_NATIVE_HOST,
};
use tgui::render::{
    BatchBoundaryReason, CompileContext, PaintCommand, RenderCompiler, RendererCapabilities,
};
use tgui::test_support::RenderHarness;
use tgui::widget::{BuildContext, Widget};
use tgui::widgets::{Button, Container, Text};
use tgui::{Application, WindowSpec};

fn host_context() -> NativeHostCreateContext {
    NativeHostCreateContext::new(WindowId::from_parts(2, 1), ElementId::from_parts(7, 3))
}

#[test]
fn mock_host_covers_layout_focus_input_composition_and_destroy() {
    let factory = MockNativeHostFactory::new(MockNativeHostConfig {
        capabilities: NativeHostCapabilities::independent_surface(),
        cost: NativeHostCost::new(1, 1, 0, 1),
        ..MockNativeHostConfig::default()
    });
    let mut hosts = NativeHostManager::new();
    let handle = hosts.create(&factory, host_context()).unwrap();
    hosts.mount(handle).unwrap();

    let layout = NativeHostLayout::new(
        Rect::from_xywh(10.0, 20.0, 320.0, 180.0),
        DpiScale::new(2.0).unwrap(),
    )
    .with_z_order(4);
    hosts.update_layout(handle, layout).unwrap();
    hosts.set_focus(handle, true).unwrap();
    hosts
        .forward_input(
            handle,
            &UiEvent::PointerDown(PointerEvent::new(
                PointerId::MOUSE,
                PointerKind::Mouse,
                Point::new(12.0, 25.0),
            )),
        )
        .unwrap();
    let composition = hosts
        .compose(handle, NativeCompositionStrategy::IndependentSurface)
        .unwrap();

    assert_eq!(
        hosts.status(handle).unwrap().lifecycle,
        NativeHostLifecycle::Mounted
    );
    assert_eq!(hosts.layout(handle).unwrap().z_order, 4);
    assert_eq!(composition.surface, ResourceId::from_parts(0, 1));
    hosts.destroy(handle).unwrap();
    assert!(!hosts.contains(handle));
    assert!(
        factory
            .calls()
            .iter()
            .any(|call| matches!(call, MockNativeHostCall::Focus(true)))
    );
    assert!(matches!(
        factory.calls().last(),
        Some(MockNativeHostCall::Destroy)
    ));
}

#[test]
fn scheduler_and_compiler_enforce_both_host_and_renderer_capabilities() {
    let capabilities = NativeHostCapabilities::independent_surface();
    let layout = NativeHostLayout::new(Rect::from_xywh(0.0, 0.0, 80.0, 40.0), DpiScale::ONE);
    let schedule =
        NativeHostScheduler::schedule(capabilities, NativeHostCost::new(0, 0, 0, 2), layout)
            .unwrap();
    assert_eq!(
        schedule.strategy,
        NativeCompositionStrategy::IndependentSurface
    );
    assert!(schedule.pass_boundary);
    assert_eq!(schedule.cost.independent_passes, 1);
    assert_eq!(schedule.cost.surfaces, 1);
    assert_eq!(schedule.cost.synchronization_points, 2);

    let command = schedule
        .paint_command(
            layout,
            tgui::native::NativeHostComposition::new(
                schedule.strategy,
                ResourceId::from_parts(9, 1),
                true,
            ),
        )
        .unwrap()
        .unwrap();
    let commands = [
        PaintCommand::FillRect {
            rect: Rect::from_xywh(0.0, 0.0, 2.0, 2.0),
            color: tgui::Color::WHITE,
        },
        command,
        PaintCommand::FillRect {
            rect: Rect::from_xywh(3.0, 0.0, 2.0, 2.0),
            color: tgui::Color::WHITE,
        },
    ];
    let rejected_context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE);
    let mut compiler = RenderCompiler::default();
    assert!(compiler.compile(&commands, &rejected_context).is_err());
    assert!(compiler.committed().is_none());

    let accepted_context = CompileContext::new(
        RendererCapabilities {
            supports_native_surface: true,
            ..RendererCapabilities::default()
        },
        DpiScale::ONE,
    );
    let compiled = compiler.compile(&commands, &accepted_context).unwrap();
    assert_eq!(compiled.batch_count(), 3);
    assert_eq!(
        compiled.batches[1].boundary_reason,
        Some(BatchBoundaryReason::NativeSurface)
    );
}

#[test]
fn builtin_controls_cannot_emit_native_surface_commands() {
    const { assert!(!ORDINARY_CONTROLS_MAY_USE_NATIVE_HOST) };
    let mut build = BuildContext::new();
    let root = Container::new()
        .with_child(Button::new("Apply").build(&mut build).unwrap())
        .with_child(Text::new("Status").build(&mut build).unwrap())
        .build(&mut build)
        .unwrap();
    let mut harness = RenderHarness::new();
    harness.mount(root).unwrap();
    harness
        .render(Size::new(300.0, 120.0), DpiScale::ONE)
        .unwrap();
    assert!(
        harness
            .render_tree()
            .commands()
            .iter()
            .all(|command| !matches!(command, PaintCommand::NativeSurface { .. }))
    );
}

#[test]
fn host_widget_and_messages_enter_the_application_pipeline() {
    let composition = NativeHostComposition::new(
        NativeCompositionStrategy::OffscreenTexture,
        ResourceId::from_parts(8, 2),
        false,
    );
    let mut build = BuildContext::new();
    let widget = NativeHostWidget::new()
        .with_key("host")
        .with_composition(composition)
        .build(&mut build)
        .unwrap();
    let mut harness = RenderHarness::new();
    harness.mount(widget.clone()).unwrap();
    harness
        .render(Size::new(100.0, 50.0), DpiScale::ONE)
        .unwrap();
    assert!(harness.render_tree().commands().iter().any(|command| {
        matches!(command, PaintCommand::DrawImage { image, .. } if image.slot() == 8 && image.generation() == 2)
    }));

    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("native host"))
        .unwrap();
    let report = application.mount_widget(window, widget).unwrap();
    application.render_window(window).unwrap();
    application.take_frame_requests().unwrap();
    let element = report.invalidations().next().unwrap().element();
    let factory = MockNativeHostFactory::new(MockNativeHostConfig {
        output_on_input: Some(NativeHostOutput::Command(
            tgui::state::UiCommand::RequestFrame(window),
        )),
        ..MockNativeHostConfig::default()
    });
    let mut hosts = NativeHostManager::new();
    let host = hosts
        .create(&factory, NativeHostCreateContext::new(window, element))
        .unwrap();
    hosts.mount(host).unwrap();
    let message = hosts
        .forward_input(
            host,
            &UiEvent::PointerDown(PointerEvent::new(
                PointerId::MOUSE,
                PointerKind::Mouse,
                Point::ZERO,
            )),
        )
        .unwrap()
        .pop()
        .unwrap();
    assert!(
        application
            .consume_native_host_message(&hosts, message)
            .unwrap()
            .is_some()
    );
    assert_eq!(application.take_frame_requests().unwrap(), [window]);

    let stale_message = tgui::native::NativeHostMessage {
        host,
        target: ElementId::from_parts(element.slot(), element.generation() + 1),
        window,
        output: NativeHostOutput::Command(tgui::state::UiCommand::RequestFrame(window)),
    };
    assert!(
        application
            .consume_native_host_message(&hosts, stale_message)
            .unwrap()
            .is_none()
    );
}

#[cfg(feature = "webview")]
#[test]
fn webview_feature_exposes_an_external_surface_host() {
    use tgui::native::webview::WebViewHostFactory;

    let factory = WebViewHostFactory::new(
        "https://example.invalid/",
        ResourceId::from_parts(12, 1),
        Size::new(640.0, 480.0),
    )
    .unwrap();
    assert_eq!(factory.url(), "https://example.invalid/");
    let mut hosts = NativeHostManager::new();
    let handle = hosts.create(&factory, host_context()).unwrap();
    hosts.mount(handle).unwrap();
    let composition = hosts
        .compose(handle, NativeCompositionStrategy::IndependentSurface)
        .unwrap();
    assert_eq!(composition.surface, ResourceId::from_parts(12, 1));
    assert!(
        hosts
            .compose(handle, NativeCompositionStrategy::OffscreenTexture)
            .is_err()
    );
}
