use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tgui::accessibility::{
    AccessibilityTree, SemanticNodeInput, SemanticSnapshot, SemanticUpdateReasons,
    compare_accessibility_snapshots,
};
use tgui::animation::{Animated, AnimationImpact, AnimationKey, AnimationSpec};
use tgui::core::{
    Color, DpiScale, ImageHandle, ItemKey, Point, ResourceId, RevisionSet, SceneRevision,
    SemanticRevision, Size,
};
use tgui::diagnostics::{BudgetDomain, CacheBudgetLimits, ResourceBudgetConfig};
use tgui::event::{
    AccessibilityAction, AccessibilityActionEvent, CommittedHitTarget, EventHandler, EventPhase,
    PointerEvent, PointerId, PointerKind, UiEvent,
};
use tgui::media::{
    DecodedImage, ImageDecodeResult, ImagePresentation, ImageRegistry, ImageRequestKey, ImageSize,
    ImageSource,
};
use tgui::native::{
    MockNativeHostFactory, NativeHostCreateContext, NativeHostLayout, NativeHostLifecycle,
    NativeHostManager, NativeHostScheduler,
};
use tgui::render::{CompileContext, PaintCommand, RenderCompiler, RendererCapabilities};
use tgui::test_support::{FakeClock, RenderHarness};
use tgui::virtualization::{VirtualList, VirtualListDataSource};
use tgui::widget::{BuildContext, OPACITY, View, Widget, WidgetNode};
use tgui::widgets::{Button, Container, Image, Text};
use tgui::{
    Application, AtomicSnapshotStore, CpuSnapshot, Error, ResourceCompletion, State, WindowSpec,
};

const BUTTON_KEY: &str = "button";
const IMAGE_KEY: &str = "image";

struct ImagePanel;

#[derive(Clone)]
struct IntegratedView {
    count: State<u32>,
    trace: Rc<RefCell<Vec<EventPhase>>>,
}

impl View for IntegratedView {
    fn build_view(&self, context: &mut BuildContext) -> tgui::Result<WidgetNode> {
        let count = context.read_state(&self.count)?;
        let count_state = self.count.clone();
        let trace = self.trace.clone();
        let button = Button::new(format!("count={count}"))
            .with_key(BUTTON_KEY)
            .with_event_handler(EventHandler::new(1, move |event, context| {
                if matches!(
                    event,
                    UiEvent::PointerDown(_)
                        | UiEvent::AccessibilityAction(AccessibilityActionEvent {
                            action: AccessibilityAction::Activate,
                            ..
                        })
                ) {
                    trace.borrow_mut().push(context.phase());
                    if context.phase() == EventPhase::Target {
                        count_state.update(context.transaction(), |value| *value += 1)?;
                    }
                }
                Ok(())
            }))
            .build(context)?;
        let title = Text::new("P7 / 中文 / مرحبا").build(context)?;
        Container::new()
            .with_children([
                title,
                button,
                WidgetNode::new::<ImagePanel>().with_key(IMAGE_KEY),
            ])
            .build(context)
    }
}

struct Rows(usize);
struct Row;

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(index as u64)
    }

    fn build_item(
        &self,
        index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Ok(WidgetNode::new::<Row>().with_key(index as u64))
    }
}

fn element(application: &Application, window: tgui::WindowId, key: &str) -> tgui::ElementId {
    application
        .element_diagnostics(window)
        .unwrap()
        .into_iter()
        .find(|node| node.key.as_ref().and_then(|key| key.as_str()) == Some(key))
        .unwrap()
        .id
}

#[test]
fn integrated_frame_orders_updates_before_atomic_cpu_commit() {
    let clock = Rc::new(FakeClock::new());
    let count = State::new(0_u32);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut application = Application::with_frame_clock(clock.clone());
    let window = application
        .create_window(WindowSpec::new("P7 frame order"))
        .unwrap();
    application
        .set_view(
            window,
            IntegratedView {
                count: count.clone(),
                trace: trace.clone(),
            },
        )
        .unwrap();
    application.render_window(window).unwrap();
    #[cfg(feature = "text")]
    {
        let compiled = application.compiled_scene(window).unwrap();
        assert!(!compiled.glyph_page_uploads.is_empty());
        assert!(
            compiled
                .batches
                .iter()
                .any(|batch| matches!(&batch.kind, tgui::render::BatchKind::Glyph { .. }))
        );
    }
    let previous = application.committed_snapshot(window).unwrap();
    let button = element(&application, window, BUTTON_KEY);
    let semantic_id = previous
        .semantics()
        .node_for_element(button)
        .expect("button has semantics")
        .id();

    let event = UiEvent::PointerDown(PointerEvent::new(
        PointerId::MOUSE,
        PointerKind::Mouse,
        Point::new(1.0, 1.0),
    ));
    application
        .dispatch_event(
            window,
            CommittedHitTarget::for_window(window, previous.layout().revision(), Some(button)),
            &event,
        )
        .unwrap();
    assert_eq!(count.get().unwrap(), 1);
    assert_eq!(trace.borrow().as_slice(), [EventPhase::Target]);
    assert_eq!(
        application.committed_snapshot(window).unwrap().as_ref(),
        previous.as_ref(),
        "update/reconciliation does not expose a half-built CPU snapshot"
    );

    let opacity = Animated::new(1.0_f32);
    application
        .animate(
            window,
            AnimationKey::new(button, OPACITY),
            &opacity,
            0.5,
            AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
        )
        .unwrap();
    clock.advance(Duration::from_millis(50)).unwrap();
    application.tick_animations(window).unwrap();

    let image = element(&application, window, IMAGE_KEY);
    let stale = application
        .begin_resource_request(window, image, ImageHandle::from_parts(2, 1).stamp())
        .unwrap();
    let current = application
        .begin_resource_request(window, image, ImageHandle::from_parts(2, 2).stamp())
        .unwrap();
    let dropped = application
        .complete_resource_request(ResourceCompletion::new(
            stale,
            [ResourceId::from_parts(4, 1)],
            1,
        ))
        .unwrap();
    assert!(dropped.stale);
    application
        .complete_resource_request(ResourceCompletion::new(
            current,
            [ResourceId::from_parts(4, 2)],
            2,
        ))
        .unwrap();

    let mut list = VirtualList::new(Rows(100_000), 20.0).unwrap();
    list.set_viewport(300.0, 100.0).unwrap();
    list.set_scroll_offset(40_000.0).unwrap();
    application
        .record_virtualization_metrics(window, list.metrics().into())
        .unwrap();

    let factory = MockNativeHostFactory::default();
    let mut hosts = NativeHostManager::new();
    let host = hosts
        .create(&factory, NativeHostCreateContext::new(window, image))
        .unwrap();
    hosts.mount(host).unwrap();
    let host_layout =
        NativeHostLayout::new(tgui::Rect::from_xywh(0.0, 0.0, 80.0, 40.0), DpiScale::ONE);
    hosts.update_layout(host, host_layout).unwrap();
    let schedule = NativeHostScheduler::schedule(
        hosts.capabilities(host).unwrap(),
        hosts.cost(host).unwrap(),
        host_layout,
    )
    .unwrap();
    let composition = hosts.compose(host, schedule.strategy).unwrap();
    assert!(
        schedule
            .paint_command(host_layout, composition)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        hosts.status(host).unwrap().lifecycle,
        NativeHostLifecycle::Mounted
    );

    let frame = application.render_window(window).unwrap();
    let committed = application.committed_snapshot(window).unwrap();
    let compiled = application.compiled_scene(window).unwrap();
    assert_eq!(frame.layout.snapshot, *committed.layout());
    assert_eq!(frame.scene, *committed.scene());
    assert_eq!(compiled.scene_revision, committed.scene().revision());
    assert_eq!(
        committed.resources().references(),
        &[ResourceId::from_parts(4, 2)]
    );
    assert_eq!(
        committed.revisions(),
        application
            .window_info(window)
            .unwrap()
            .committed_revisions
            .unwrap()
    );
    let metrics = application.frame_metrics(window).unwrap();
    assert!(metrics.dirty_elements > 0);
    assert!(metrics.scene.paint_commands > 0);
    assert!(metrics.virtualization.materialized_items < 100);
    assert_eq!(opacity.value(), 0.75);

    assert_eq!(
        committed
            .semantics()
            .node_for_element(button)
            .expect("keyed button remains semantic")
            .id(),
        semantic_id
    );
    let inputs = committed
        .semantics()
        .nodes()
        .iter()
        .map(|node| {
            SemanticNodeInput::new(node.element(), node.semantics().clone())
                .with_parent(node.parent().and_then(|parent| parent.element()))
                .with_bounds(node.bounds())
        })
        .collect::<Vec<_>>();
    let previous_semantic_revision =
        SemanticRevision::new(committed.semantics().revision().get().saturating_sub(1));
    let mut rebuilt_semantics =
        AccessibilityTree::from_snapshot(SemanticSnapshot::empty(previous_semantic_revision));
    let rebuilt = rebuilt_semantics
        .update(
            inputs,
            committed
                .semantics()
                .focus()
                .and_then(|focus| focus.element()),
            SemanticUpdateReasons::SEMANTICS
                .union(SemanticUpdateReasons::FOCUS)
                .union(SemanticUpdateReasons::LAYOUT_BOUNDS),
        )
        .unwrap();
    compare_accessibility_snapshots(committed.semantics(), &rebuilt.snapshot).unwrap();

    application
        .dispatch_event(
            window,
            CommittedHitTarget::for_window(window, committed.layout().revision(), Some(button)),
            &UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
                window,
                button,
                AccessibilityAction::Activate,
            )),
        )
        .unwrap();
    assert_eq!(count.get().unwrap(), 2);
    application.render_window(window).unwrap();
    assert_eq!(
        application
            .committed_snapshot(window)
            .unwrap()
            .semantics()
            .node_for_element(button)
            .unwrap()
            .id(),
        semantic_id
    );
    hosts.destroy(host).unwrap();
    assert!(!hosts.contains(host));
}

#[test]
fn fault_matrix_preserves_previous_outputs_or_uses_explicit_fallbacks() {
    let mut snapshots = AtomicSnapshotStore::default();
    snapshots
        .try_commit(CpuSnapshot::empty(RevisionSet::ZERO))
        .unwrap();
    let committed = snapshots.committed().unwrap();
    assert!(
        snapshots
            .compile_and_commit(|| Err(Error::compile("injected", "compile failure")))
            .is_err()
    );
    assert_eq!(snapshots.committed().unwrap().as_ref(), committed.as_ref());

    let context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE)
        .with_scene_revision(SceneRevision::new(1));
    let mut compiler = RenderCompiler::default();
    compiler
        .compile(
            &[PaintCommand::FillRect {
                rect: tgui::Rect::from_xywh(0.0, 0.0, 4.0, 4.0),
                color: Color::WHITE,
            }],
            &context,
        )
        .unwrap();
    let compiled = compiler.committed().unwrap();
    assert!(
        compiler
            .compile(&[PaintCommand::PopClip], &context)
            .is_err()
    );
    assert_eq!(
        compiler.committed().unwrap().fingerprint,
        compiled.fingerprint
    );

    let mut images = ImageRegistry::new();
    let first_key = ImageRequestKey::new(ImageSource::bytes([1_u8].as_slice()));
    let first = images.request(first_key.clone());
    assert_eq!(
        images.presentation(first.handle),
        ImagePresentation::Placeholder
    );
    let decoded = DecodedImage::new(ImageSize::new(1, 1).unwrap(), [1, 2, 3, 255]).unwrap();
    images.complete(
        first.handle.stamp(),
        &ImageDecodeResult {
            handle: first.handle,
            key: first_key,
            decoded: Ok(decoded),
        },
    );
    let replacement_key = ImageRequestKey::new(ImageSource::bytes([2_u8].as_slice()));
    let replacement = images
        .replace(first.handle, replacement_key.clone())
        .unwrap();
    assert_eq!(
        images.presentation(replacement.handle),
        ImagePresentation::Texture(first.handle),
        "loading replacement keeps the last ready texture"
    );
    assert!(matches!(
        images.complete(
            first.handle.stamp(),
            &ImageDecodeResult {
                handle: first.handle,
                key: replacement_key,
                decoded: Err(tgui::media::ImageLoadError::InvalidDimensions),
            },
        ),
        tgui::media::ImageCompletion::Stale
    ));

    let tiny = ResourceBudgetConfig::new(
        CacheBudgetLimits::new(4, 4),
        CacheBudgetLimits::new(4, 4),
        CacheBudgetLimits::new(4, 4),
    );
    let mut application = Application::new();
    let window = application
        .create_window(
            WindowSpec::new("P7 faults")
                .with_inner_size(Size::new(100.0, 50.0))
                .with_resource_budgets(tiny),
        )
        .unwrap();
    application
        .mount_widget(window, WidgetNode::new::<ImagePanel>())
        .unwrap();
    application.render_window(window).unwrap();
    let before_budget_failure = application.committed_snapshot(window).unwrap();
    assert!(
        application
            .reserve_resource_bytes(window, BudgetDomain::GpuCache, 5)
            .is_err()
    );
    assert_eq!(
        application.committed_snapshot(window).unwrap().as_ref(),
        before_budget_failure.as_ref()
    );

    let resize_revision = before_budget_failure.layout().revision();
    application
        .dispatch_event(
            window,
            CommittedHitTarget::miss_for_window(window, resize_revision),
            &UiEvent::WindowResized(Size::new(160.0, 90.0)),
        )
        .unwrap();
    application.render_window(window).unwrap();
    assert_eq!(
        application
            .committed_snapshot(window)
            .unwrap()
            .layout()
            .viewport(),
        Size::new(160.0, 90.0)
    );
}

#[test]
fn ordinary_widgets_reach_compiler_without_native_surface_commands() {
    let mut context = BuildContext::new();
    let root = Container::new()
        .with_child(Text::new("logical text").build(&mut context).unwrap())
        .with_child(Button::new("button").build(&mut context).unwrap())
        .build(&mut context)
        .unwrap();
    let mut harness = RenderHarness::new();
    harness.mount(root).unwrap();
    let frame = harness
        .render(Size::new(320.0, 200.0), DpiScale::ONE)
        .unwrap();
    assert!(frame.layout.node_count() >= 4);
    assert!(frame.scene.command_count() >= 2);
    assert!(frame.compiled.batches >= 2);
    assert!(
        harness
            .render_tree()
            .commands()
            .iter()
            .all(|command| !matches!(command, PaintCommand::NativeSurface { .. }))
    );
}

#[test]
fn image_widget_reaches_compiler_with_generation_stamped_draw_command() {
    let mut build = BuildContext::new();
    let image = Image::new(ImageHandle::from_parts(17, 3))
        .with_alt_text("preview")
        .build(&mut build)
        .unwrap();
    let mut harness = RenderHarness::new();
    harness.mount(image).unwrap();
    harness
        .render(Size::new(160.0, 100.0), DpiScale::ONE)
        .unwrap();
    assert!(harness.render_tree().commands().iter().any(|command| {
        matches!(
            command,
            PaintCommand::DrawImage { image, .. }
                if image.slot() == 17 && image.generation() == 3
        )
    }));
}

#[test]
fn reconciled_and_fresh_render_outputs_are_observably_equivalent() {
    fn declaration(label: &str, enabled: bool) -> WidgetNode {
        let mut context = BuildContext::new();
        Container::new()
            .with_child(
                Button::new(label)
                    .with_key("stable")
                    .with_enabled(enabled)
                    .build(&mut context)
                    .unwrap(),
            )
            .build(&mut context)
            .unwrap()
    }

    let viewport = Size::new(320.0, 200.0);
    let changed = declaration("changed", false);
    let mut reconciled = RenderHarness::new();
    reconciled.mount(declaration("base", true)).unwrap();
    reconciled.render(viewport, DpiScale::ONE).unwrap();
    reconciled.reconcile(changed.clone()).unwrap();
    let incremental = reconciled.render(viewport, DpiScale::ONE).unwrap();

    let mut rebuilt = RenderHarness::new();
    rebuilt.mount(changed).unwrap();
    let full = rebuilt.render(viewport, DpiScale::ONE).unwrap();

    assert_eq!(
        reconciled.render_tree().commands(),
        rebuilt.render_tree().commands()
    );
    assert_eq!(
        incremental.scene.command_count(),
        full.scene.command_count()
    );
    assert_eq!(incremental.scene.chunk_count(), full.scene.chunk_count());
    assert_eq!(incremental.scene.fingerprint(), full.scene.fingerprint());
    assert_eq!(
        incremental.compiled.paint_commands,
        full.compiled.paint_commands
    );
    assert_eq!(incremental.compiled.passes, full.compiled.passes);
    assert_eq!(incremental.compiled.batches, full.compiled.batches);
    assert_eq!(
        incremental.compiled.quad_instances,
        full.compiled.quad_instances
    );
    assert_eq!(incremental.compiled.fingerprint, full.compiled.fingerprint);
}

#[cfg(feature = "render")]
#[test]
fn wgpu_device_recovery_keeps_the_headless_path_available_when_supported() {
    let Ok(mut renderer) = pollster::block_on(tgui::render::wgpu::WgpuRenderer::new_headless())
    else {
        return;
    };
    renderer.inject_device_loss();
    assert!(renderer.is_device_lost());
    pollster::block_on(renderer.recover_device()).unwrap();
    assert!(!renderer.is_device_lost());
}
