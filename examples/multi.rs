//! Real-window showcase for every built-in tgui component.
//!
//! Run with the default desktop features:
//!
//! ```text
//! cargo run --example multi
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use tgui::accessibility::Semantics;
use tgui::core::{DpiScale, Error, ImageHandle, Point, Rect, ResourceId};
use tgui::event::{
    AccessibilityAction, AccessibilityActionEvent, EventHandler, EventPhase, PointerButton,
    PointerButtons, PointerEvent, PointerId, PointerKind, UiEvent,
};
use tgui::layout::{Dimension, FlexDirection, LayoutSize, LayoutStyle, LengthPercentage, Sides};
use tgui::media::{
    DecodedImage, ImageCompletion, ImageDecodeResult, ImagePresentation, ImageRegistry,
    ImageRequestKey, ImageSize, ImageSource,
};
use tgui::native::{
    MockNativeHostFactory, NativeHostComposition, NativeHostCreateContext, NativeHostLayout,
    NativeHostManager, NativeHostScheduler, NativeHostWidget,
};
use tgui::virtualization::VirtualList;
use tgui::virtualization::VirtualListDataSource;
use tgui::widget::{BuildContext, View, Widget, WidgetNode};
use tgui::widgets::{Button, Container, Image, Text};
use tgui::{Application, ElementId, ResourceCompletion, Size, State, WindowId, WindowSpec};

#[cfg(feature = "desktop")]
use tgui::platform::{WinitSurface, WinitSurfaceEvent};
#[cfg(feature = "desktop")]
use winit::application::ApplicationHandler;
#[cfg(feature = "desktop")]
use winit::event::{ElementState, MouseButton, WindowEvent};
#[cfg(feature = "desktop")]
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(feature = "desktop")]
use winit::window::WindowId as PlatformWindowId;

const BUTTON_KEY: &str = "multi-primary-button";
const IMAGE_KEY: &str = "multi-image";
const HOST_KEY: &str = "multi-native-host";

struct Showcase {
    application: Application,
    window: WindowId,
    spec: WindowSpec,
    _images: ImageRegistry,
    _hosts: NativeHostManager,
}

impl Showcase {
    fn new() -> tgui::Result<Self> {
        let (mut images, image_key, image_request) = prepare_image();
        let (rows, list_semantics, list_metrics) = prepare_virtual_rows()?;
        let model = ShowcaseModel::new(image_request.handle, rows, list_semantics.accessibility());

        let spec = WindowSpec::new("tgui all-components showcase")
            .with_inner_size(Size::new(720.0, 480.0))
            .with_min_inner_size(Some(Size::new(640.0, 420.0)));
        let mut application = Application::new();
        let window = application.create_window(spec.clone())?;
        application.set_view(window, ShowcaseView::new(model.clone()))?;
        application.render_window(window)?;

        let image = element_with_key(&application, window, IMAGE_KEY)?;
        complete_image(
            &mut application,
            window,
            image,
            &mut images,
            image_key,
            image_request.handle,
        )?;

        let host_element = element_with_key(&application, window, HOST_KEY)?;
        let host_factory = MockNativeHostFactory::default();
        let mut hosts = NativeHostManager::new();
        let host = hosts.create(
            &host_factory,
            NativeHostCreateContext::new(window, host_element),
        )?;
        hosts.mount(host)?;
        let host_layout =
            NativeHostLayout::new(Rect::from_xywh(0.0, 0.0, 240.0, 96.0), DpiScale::ONE);
        hosts.update_layout(host, host_layout)?;
        let schedule = NativeHostScheduler::schedule(
            hosts.capabilities(host).expect("the host is live"),
            hosts.cost(host).expect("the host is live"),
            host_layout,
        )?;
        let composition = hosts.compose(host, schedule.strategy)?;
        let mut transaction = tgui::UpdateTxn::new();
        model
            .native_composition
            .set(&mut transaction, Some(composition))?;
        application.apply_transaction(transaction)?;
        application.record_virtualization_metrics(window, list_metrics.into())?;

        let final_frame = application.render_window(window)?;
        let snapshot = application
            .committed_snapshot(window)
            .expect("a rendered window has a committed snapshot");
        let metrics = application
            .frame_metrics(window)
            .expect("a rendered window has frame metrics");

        assert_eq!(model.clicks.get()?, 0);
        assert!(model.event_phases.borrow().is_empty());
        assert!(matches!(
            images.presentation(image_request.handle),
            ImagePresentation::Texture(handle) if handle == image_request.handle
        ));
        assert!(model.native_composition.get()?.is_some());
        assert!(list_metrics.materialized_items < list_metrics.total_items);
        assert!(final_frame.scene.command_count() > 0);

        println!(
            "components=Container,Text,Button,Image,NativeHostWidget clicks={} phases={:?}",
            model.clicks.get()?,
            model.event_phases.borrow(),
        );
        println!(
            "image={:?} virtual_rows={}/{} native_calls={}",
            images.presentation(image_request.handle),
            list_metrics.materialized_items,
            list_metrics.total_items,
            host_factory.calls().len(),
        );
        println!(
            "layout_nodes={} semantics_nodes={} commands={} batches={} revisions={:?}",
            snapshot.layout().node_count(),
            snapshot.semantics().node_count(),
            final_frame.scene.command_count(),
            metrics.scene.batches,
            snapshot.revisions(),
        );
        Ok(Self {
            application,
            window,
            spec,
            _images: images,
            _hosts: hosts,
        })
    }

    #[cfg(feature = "desktop")]
    fn dispatch(&mut self, hit_at: Point, event: UiEvent) -> tgui::Result<()> {
        let hit = self.application.hit_test(self.window, hit_at)?;
        self.application.dispatch_event(self.window, hit, &event)?;
        Ok(())
    }

    #[cfg(feature = "desktop")]
    fn redraw(&mut self, surface: &mut WinitSurface) -> tgui::Result<()> {
        self.application.render_window(self.window)?;
        let scene = self
            .application
            .compiled_scene(self.window)
            .expect("a successful frame commits a compiled scene");
        surface.render(&scene)
    }
}

#[cfg(feature = "desktop")]
struct DesktopShowcase {
    showcase: Showcase,
    surface: Option<WinitSurface>,
    cursor: Point,
    primary_pressed: bool,
}

#[cfg(feature = "desktop")]
impl DesktopShowcase {
    fn new() -> tgui::Result<Self> {
        Ok(Self {
            showcase: Showcase::new()?,
            surface: None,
            cursor: Point::ZERO,
            primary_pressed: false,
        })
    }

    fn dispatch_window_state(&mut self, event: UiEvent) -> tgui::Result<()> {
        self.showcase.dispatch(Point::ZERO, event)
    }

    fn dispatch_pointer(&mut self, event: UiEvent) -> tgui::Result<()> {
        self.showcase.dispatch(self.cursor, event)
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.request_redraw();
        }
    }

    fn recover_after_render_error(&mut self, event_loop: &ActiveEventLoop, error: tgui::Error) {
        eprintln!("desktop frame failed: {error}");
        let Some(surface) = self.surface.as_mut() else {
            event_loop.exit();
            return;
        };
        match pollster::block_on(surface.recover_device()) {
            Ok(()) => surface.request_redraw(),
            Err(recovery_error) => {
                eprintln!("GPU recovery failed: {recovery_error}");
                event_loop.exit();
            }
        }
    }
}

#[cfg(feature = "desktop")]
impl ApplicationHandler for DesktopShowcase {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        match pollster::block_on(WinitSurface::new(event_loop, &self.showcase.spec)) {
            Ok(surface) => {
                let logical_size = surface.logical_size();
                let dpi_scale = surface.dpi_scale();
                self.surface = Some(surface);
                let synchronized = self
                    .dispatch_window_state(UiEvent::WindowResized(logical_size))
                    .and_then(|()| {
                        self.dispatch_window_state(UiEvent::WindowDpiChanged(dpi_scale))
                    });
                if let Err(error) = synchronized {
                    eprintln!("window initialization failed: {error}");
                    event_loop.exit();
                    return;
                }
                self.request_redraw();
            }
            Err(error) => {
                eprintln!("desktop initialization failed: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: PlatformWindowId,
        event: WindowEvent,
    ) {
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        if surface.id() != window_id {
            return;
        }
        let translated = match surface.handle_event(&event) {
            Ok(event) => event,
            Err(error) => {
                eprintln!("window event failed: {error}");
                return;
            }
        };

        let result = match translated {
            WinitSurfaceEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WinitSurfaceEvent::RedrawRequested => {
                let surface = self.surface.as_mut().expect("surface remains installed");
                self.showcase.redraw(surface)
            }
            WinitSurfaceEvent::Resized {
                logical_size,
                dpi_scale,
            } => self
                .dispatch_window_state(UiEvent::WindowResized(logical_size))
                .and_then(|()| self.dispatch_window_state(UiEvent::WindowDpiChanged(dpi_scale)))
                .map(|()| self.request_redraw()),
            WinitSurfaceEvent::Ignored => match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let scale = self
                        .surface
                        .as_ref()
                        .expect("surface remains installed")
                        .dpi_scale()
                        .get();
                    let logical = position.to_logical::<f64>(scale);
                    self.cursor = Point::new(logical.x as f32, logical.y as f32);
                    let buttons = if self.primary_pressed {
                        PointerButtons::PRIMARY
                    } else {
                        PointerButtons::NONE
                    };
                    self.dispatch_pointer(UiEvent::PointerMove(
                        PointerEvent::new(PointerId::MOUSE, PointerKind::Mouse, self.cursor)
                            .with_buttons(buttons),
                    ))
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.primary_pressed = state == ElementState::Pressed;
                    let buttons = if self.primary_pressed {
                        PointerButtons::PRIMARY
                    } else {
                        PointerButtons::NONE
                    };
                    let pointer =
                        PointerEvent::new(PointerId::MOUSE, PointerKind::Mouse, self.cursor)
                            .with_button(Some(PointerButton::Primary))
                            .with_buttons(buttons);
                    let event = if self.primary_pressed {
                        UiEvent::PointerDown(pointer)
                    } else {
                        UiEvent::PointerUp(pointer)
                    };
                    self.dispatch_pointer(event).map(|()| self.request_redraw())
                }
                WindowEvent::Focused(active) => self
                    .dispatch_window_state(UiEvent::WindowActivated(active))
                    .map(|()| self.request_redraw()),
                _ => Ok(()),
            },
        };

        if let Err(error) = result {
            if translated == WinitSurfaceEvent::RedrawRequested {
                self.recover_after_render_error(event_loop, error);
            } else {
                eprintln!("input dispatch failed: {error}");
            }
        }
    }
}

#[cfg(feature = "desktop")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut showcase = DesktopShowcase::new()?;
    event_loop.run_app(&mut showcase)?;
    Ok(())
}

#[cfg(not(feature = "desktop"))]
fn main() -> tgui::Result<()> {
    let _showcase = Showcase::new()?;
    println!("the multi example requires the `desktop` feature to open a window");
    Ok(())
}

fn prepare_image() -> (ImageRegistry, ImageRequestKey, tgui::media::ImageRequest) {
    let mut images = ImageRegistry::new();
    let key = ImageRequestKey::new(ImageSource::bytes(b"multi-example-image".as_slice()));
    let request = images.request(key.clone());
    (images, key, request)
}

fn prepare_virtual_rows() -> tgui::Result<(
    Vec<tgui::widget::WidgetNode>,
    tgui::virtualization::CollectionSemantics,
    tgui::virtualization::VirtualListMetrics,
)> {
    let mut list = VirtualList::new(DemoRows::new(10_000), 28.0)?;
    list.set_overscan(56.0)?;
    list.set_viewport(140.0, 56_000.0)?;

    let rows = list
        .materialized_items()
        .map(|item| {
            let semantics = list
                .item_semantics(item.key())
                .expect("a materialized row has item semantics")
                .accessibility();
            item.node().clone().with_semantics(semantics)
        })
        .collect();
    Ok((rows, list.collection_semantics(), list.metrics()))
}

fn complete_image(
    application: &mut Application,
    window: tgui::WindowId,
    element: tgui::ElementId,
    images: &mut ImageRegistry,
    key: ImageRequestKey,
    handle: tgui::core::ImageHandle,
) -> tgui::Result<()> {
    let decoded = DecodedImage::new(
        ImageSize::new(2, 2)?,
        [
            32, 104, 180, 255, 245, 158, 11, 255, 28, 138, 92, 255, 220, 38, 38, 255,
        ],
    )?;
    let decoded_bytes = decoded.byte_len();
    let result = ImageDecodeResult {
        handle,
        key: key.clone(),
        decoded: Ok(decoded),
    };
    assert!(matches!(
        images.complete(handle.stamp(), &result),
        ImageCompletion::Ready { .. }
    ));

    let ticket = application.begin_resource_request(window, element, handle.stamp())?;
    let receipt = application.complete_resource_request(
        ResourceCompletion::new(
            ticket,
            [ResourceId::from_parts(handle.slot(), handle.generation())],
            0x006d_756c_7469,
        )
        .with_intrinsic_size_changed(true)
        .with_upload_bytes(decoded_bytes),
    )?;
    assert!(receipt.accepted && !receipt.stale);
    Ok(())
}

#[derive(Clone)]
struct ShowcaseModel {
    clicks: State<u32>,
    event_phases: Rc<RefCell<Vec<EventPhase>>>,
    native_composition: State<Option<NativeHostComposition>>,
    image: ImageHandle,
    rows: State<Vec<WidgetNode>>,
    list_semantics: Semantics,
}

impl ShowcaseModel {
    fn new(image: ImageHandle, rows: Vec<WidgetNode>, list_semantics: Semantics) -> Self {
        Self {
            clicks: State::new(0),
            event_phases: Rc::new(RefCell::new(Vec::new())),
            native_composition: State::new(None),
            image,
            rows: State::new(rows),
            list_semantics,
        }
    }
}

struct ShowcaseView {
    model: ShowcaseModel,
}

impl ShowcaseView {
    fn new(model: ShowcaseModel) -> Self {
        Self { model }
    }
}

impl View for ShowcaseView {
    fn build_view(&self, context: &mut BuildContext) -> tgui::Result<WidgetNode> {
        let clicks = context.read_state(&self.model.clicks)?;
        let composition = context.read_state(&self.model.native_composition)?;
        let rows = context.read_state(&self.model.rows)?;

        let title = fixed_size(
            Text::new("tgui components: English / 中文 / مرحبا")
                .with_key("multi-title")
                .build(context)?,
            688.0,
            36.0,
        );
        let status = fixed_size(
            Text::new(format!(
                "The enabled button has been activated {clicks} time(s)."
            ))
            .with_key("multi-status")
            .build(context)?,
            688.0,
            28.0,
        );

        let clicks_state = self.model.clicks.clone();
        let phases = self.model.event_phases.clone();
        let enabled_button = fixed_size(
            Button::new("Activate")
                .with_key(BUTTON_KEY)
                .with_event_handler(EventHandler::new(1, move |event, context| {
                    let activated = matches!(
                        event,
                        UiEvent::PointerDown(_)
                            | UiEvent::AccessibilityAction(AccessibilityActionEvent {
                                action: AccessibilityAction::Activate,
                                ..
                            })
                    );
                    if activated && context.phase() == EventPhase::Target {
                        println!("按钮被按下了");
                        phases.borrow_mut().push(context.phase());
                        clicks_state.update(context.transaction(), |value| *value += 1)?;
                    }
                    Ok(())
                }))
                .build(context)?,
            200.0,
            44.0,
        );
        let disabled_button = fixed_size(
            Button::new("Disabled")
                .with_key("multi-disabled-button")
                .with_enabled(false)
                .build(context)?,
            200.0,
            44.0,
        );
        let button_row = Container::new()
            .with_key("multi-button-row")
            .with_children([enabled_button, disabled_button])
            .build(context)?
            .with_layout_style(row_style(688.0, 44.0, 12.0));

        let image = Image::new(self.model.image)
            .with_key(IMAGE_KEY)
            .with_alt_text("A decoded two-by-two color preview")
            .with_size(Size::new(128.0, 96.0))
            .build(context)?;
        let native_host = NativeHostWidget::new()
            .with_key(HOST_KEY)
            .with_layout_style(sized_style(240.0, 96.0))
            .with_z_order(1);
        let native_host = composition.map_or(native_host.clone(), |composition| {
            native_host.with_composition(composition)
        });
        let media_row = Container::new()
            .with_key("multi-media-row")
            .with_children([image, native_host.build(context)?])
            .build(context)?
            .with_layout_style(row_style(688.0, 96.0, 12.0));

        let list = Container::new()
            .with_key("multi-virtual-list")
            .with_children(rows)
            .build(context)?
            .with_layout_style(column_style(688.0, 160.0, 0.0, 4.0))
            .with_semantics(self.model.list_semantics.clone());

        Container::new()
            .with_key("multi-root")
            .with_children([title, status, button_row, media_row, list])
            .build(context)
            .map(|node| node.with_layout_style(column_style(720.0, 480.0, 12.0, 16.0)))
    }
}

fn element_with_key(
    application: &Application,
    window: WindowId,
    key: &str,
) -> tgui::Result<ElementId> {
    application
        .element_diagnostics(window)
        .ok_or_else(|| Error::invalid_input(Some("window".to_owned()), "window is stale"))?
        .into_iter()
        .find(|node| node.key.as_ref().and_then(|key| key.as_str()) == Some(key))
        .map(|node| node.id)
        .ok_or_else(|| {
            Error::invalid_input(
                Some("widget_key".to_owned()),
                format!("component with key {key:?} was not mounted"),
            )
        })
}

fn fixed_size(node: WidgetNode, width: f32, height: f32) -> WidgetNode {
    node.with_layout_style(sized_style(width, height))
}

fn sized_style(width: f32, height: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Dimension::Length(width), Dimension::Length(height))
}

fn row_style(width: f32, height: f32, gap: f32) -> LayoutStyle {
    let mut style = sized_style(width, height);
    style.flex_direction = FlexDirection::Row;
    style.gap = LayoutSize::new(LengthPercentage::Length(gap), LengthPercentage::ZERO);
    style
}

fn column_style(width: f32, height: f32, gap: f32, padding: f32) -> LayoutStyle {
    let mut style = sized_style(width, height);
    style.flex_direction = FlexDirection::Column;
    style.gap = LayoutSize::new(LengthPercentage::ZERO, LengthPercentage::Length(gap));
    style.padding = Sides::all(LengthPercentage::Length(padding));
    style
}

struct DemoRows {
    len: usize,
}

impl DemoRows {
    const fn new(len: usize) -> Self {
        Self { len }
    }
}

impl VirtualListDataSource for DemoRows {
    fn len(&self) -> usize {
        self.len
    }

    fn item_key(&self, index: usize) -> tgui::core::ItemKey {
        tgui::core::ItemKey::numeric(index as u64)
    }

    fn build_item(
        &self,
        index: usize,
        _key: &tgui::core::ItemKey,
        context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Text::new(format!("Virtual row {index:05}"))
            .build(context)
            .map(|node| {
                node.with_layout_style(
                    LayoutStyle::default()
                        .with_size(Dimension::Percent(1.0), Dimension::Length(28.0)),
                )
            })
    }
}
