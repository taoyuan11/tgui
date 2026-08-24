//! Gallery model, retained view, resources, and headless verification path.

use tgui::accessibility::Semantics;
use tgui::core::{DpiScale, ElementId, Error, ImageHandle, ItemKey, Rect, ResourceId};
use tgui::event::UiEvent;
#[cfg(not(feature = "desktop"))]
use tgui::event::{CommittedHitTarget, PointerEvent, PointerId, PointerKind};
#[cfg(not(feature = "desktop"))]
use tgui::media::ImagePresentation;
use tgui::media::{
    DecodedImage, ImageCompletion, ImageDecodeResult, ImageRegistry, ImageRequestKey, ImageSize,
    ImageSource,
};
use tgui::native::{
    HostHandle, MockNativeHostFactory, NativeHostComposition, NativeHostCreateContext,
    NativeHostLayout, NativeHostManager, NativeHostScheduler,
};
use tgui::virtualization::{
    CollectionSemantics, VirtualList, VirtualListDataSource, VirtualListMetrics,
};
use tgui::widget::{BuildContext, View, Widget, WidgetNode};
use tgui::widgets::{Container, Text};
use tgui::{Application, ResourceCompletion, Size, State, WindowId, WindowSpec};

use crate::layout::row;
use crate::navigation::{self, Page};
use crate::pages;

pub const WINDOW_SIZE: Size = Size::new(960.0, 720.0);

#[derive(Clone)]
pub struct GalleryModel {
    pub selected_page: State<Page>,
    pub clicks: State<u32>,
    pub image: ImageHandle,
    pub virtual_rows: State<Vec<WidgetNode>>,
    pub list_semantics: Semantics,
    pub list_metrics: VirtualListMetrics,
    pub native_composition: State<Option<NativeHostComposition>>,
}

struct GalleryView {
    model: GalleryModel,
}

impl View for GalleryView {
    fn build_view(&self, context: &mut BuildContext) -> tgui::Result<WidgetNode> {
        let selected = context.read_state(&self.model.selected_page)?;
        let sidebar = navigation::sidebar(context, selected, &self.model.selected_page)?;
        let page = pages::page(context, selected, &self.model)?;

        Container::new()
            .with_key("component-gallery")
            .with_children([sidebar, page])
            .build(context)
            .map(|node| {
                node.with_layout_style(row(WINDOW_SIZE.width, WINDOW_SIZE.height, 0.0, 0.0))
            })
    }
}

pub struct Gallery {
    pub application: Application,
    pub window: WindowId,
    #[cfg(feature = "desktop")]
    pub spec: WindowSpec,
    #[cfg(not(feature = "desktop"))]
    pub model: GalleryModel,
    _images: ImageRegistry,
    hosts: NativeHostManager,
    host: HostHandle,
}

impl Gallery {
    pub fn new() -> tgui::Result<Self> {
        let (mut images, image_key, image_handle) = prepare_image();
        let (virtual_rows, list_semantics, list_metrics) = prepare_virtual_rows()?;
        let model = GalleryModel {
            selected_page: State::new(Page::Basics),
            clicks: State::new(0),
            image: image_handle,
            virtual_rows: State::new(virtual_rows),
            list_semantics: list_semantics.accessibility(),
            list_metrics,
            native_composition: State::new(None),
        };
        let spec = WindowSpec::new("tgui component gallery")
            .with_inner_size(WINDOW_SIZE)
            .with_min_inner_size(Some(Size::new(760.0, 560.0)));
        let mut application = Application::new();
        let window = application.create_window(spec.clone())?;
        application.set_view(
            window,
            GalleryView {
                model: model.clone(),
            },
        )?;
        application.render_window(window)?;

        let image_element = element_with_key(&application, window, "gallery-image")?;
        complete_image(
            &mut application,
            window,
            image_element,
            &mut images,
            image_key,
            image_handle,
        )?;

        let host_element = element_with_key(&application, window, "gallery-native-host")?;
        let host_factory = MockNativeHostFactory::default();
        let mut hosts = NativeHostManager::new();
        let host = hosts.create(
            &host_factory,
            NativeHostCreateContext::new(window, host_element),
        )?;
        hosts.mount(host)?;
        let host_layout =
            NativeHostLayout::new(Rect::from_xywh(0.0, 0.0, 320.0, 110.0), DpiScale::ONE);
        hosts.update_layout(host, host_layout)?;
        let schedule = NativeHostScheduler::schedule(
            hosts.capabilities(host).expect("the gallery host is live"),
            hosts.cost(host).expect("the gallery host is live"),
            host_layout,
        )?;
        let composition = hosts.compose(host, schedule.strategy)?;
        let mut transaction = tgui::UpdateTxn::new();
        model
            .native_composition
            .set(&mut transaction, Some(composition))?;
        application.apply_transaction(transaction)?;
        application.record_virtualization_metrics(window, list_metrics.into())?;
        application.render_window(window)?;

        Ok(Self {
            application,
            window,
            #[cfg(feature = "desktop")]
            spec,
            #[cfg(not(feature = "desktop"))]
            model,
            _images: images,
            hosts,
            host,
        })
    }

    #[cfg(feature = "desktop")]
    pub fn dispatch(&mut self, point: tgui::Point, event: UiEvent) -> tgui::Result<()> {
        let hit = self.application.hit_test(self.window, point)?;
        self.application.dispatch_event(self.window, hit, &event)?;
        Ok(())
    }

    #[cfg(not(feature = "desktop"))]
    pub fn select_page(&mut self, page: Page) -> tgui::Result<()> {
        let key = format!("nav-{}", page.key());
        let target = element_with_key(&self.application, self.window, &key)?;
        let revision = self
            .application
            .committed_snapshot(self.window)
            .expect("the gallery has rendered")
            .layout()
            .revision();
        let hit = CommittedHitTarget::for_window(self.window, revision, Some(target));
        let event = UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            tgui::Point::ZERO,
        ));
        self.application.dispatch_event(self.window, hit, &event)?;
        self.application.render_window(self.window)?;
        Ok(())
    }

    #[cfg(feature = "desktop")]
    pub fn redraw(&mut self) -> tgui::Result<std::sync::Arc<tgui::render::CompiledScene>> {
        self.application.render_window(self.window)?;
        self.application
            .compiled_scene(self.window)
            .ok_or_else(|| Error::compile("gallery", "a rendered gallery has no compiled scene"))
    }
}

impl Drop for Gallery {
    fn drop(&mut self) {
        let _ = self.hosts.destroy(self.host);
    }
}

#[cfg(not(feature = "desktop"))]
pub fn run_headless() -> tgui::Result<()> {
    let mut gallery = Gallery::new()?;
    for page in Page::ALL {
        gallery.select_page(page)?;
        assert_eq!(gallery.model.selected_page.get()?, page);
    }

    let snapshot = gallery
        .application
        .committed_snapshot(gallery.window)
        .expect("the headless gallery has a committed snapshot");
    assert!(gallery.model.list_metrics.materialized_items < gallery.model.list_metrics.total_items);
    assert!(matches!(
        gallery._images.presentation(gallery.model.image),
        ImagePresentation::Texture(handle) if handle == gallery.model.image
    ));
    println!(
        "pages={} selected={:?} components=Container,Text,Button,Image,VirtualList,NativeHostWidget layout_nodes={} semantics_nodes={} virtual_rows={}/{}",
        Page::ALL.len(),
        gallery.model.selected_page.get()?,
        snapshot.layout().node_count(),
        snapshot.semantics().node_count(),
        gallery.model.list_metrics.materialized_items,
        gallery.model.list_metrics.total_items,
    );
    Ok(())
}

fn prepare_image() -> (ImageRegistry, ImageRequestKey, ImageHandle) {
    let mut images = ImageRegistry::new();
    let key = ImageRequestKey::new(ImageSource::bytes(b"multi-module-gallery".as_slice()));
    let request = images.request(key.clone());
    (images, key, request.handle)
}

fn complete_image(
    application: &mut Application,
    window: WindowId,
    element: ElementId,
    images: &mut ImageRegistry,
    key: ImageRequestKey,
    handle: ImageHandle,
) -> tgui::Result<()> {
    let decoded = DecodedImage::new(
        ImageSize::new(2, 2)?,
        [
            31, 41, 55, 255, 16, 185, 129, 255, 239, 68, 68, 255, 245, 158, 11, 255,
        ],
    )?;
    let decoded_bytes = decoded.byte_len();
    let result = ImageDecodeResult {
        handle,
        key,
        decoded: Ok(decoded),
    };
    assert!(matches!(
        images.complete(handle.stamp(), &result),
        ImageCompletion::Ready { .. }
    ));
    let ticket = application.begin_resource_request(window, element, handle.stamp())?;
    application.complete_resource_request(
        ResourceCompletion::new(
            ticket,
            [ResourceId::from_parts(handle.slot(), handle.generation())],
            0x7467_7569,
        )
        .with_intrinsic_size_changed(true)
        .with_upload_bytes(decoded_bytes),
    )?;
    Ok(())
}

fn prepare_virtual_rows() -> tgui::Result<(Vec<WidgetNode>, CollectionSemantics, VirtualListMetrics)>
{
    let mut list = VirtualList::new(DemoRows(10_000), 28.0)?;
    list.set_overscan(56.0)?;
    list.set_viewport(112.0, 28_000.0)?;
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

struct DemoRows(usize);

impl VirtualListDataSource for DemoRows {
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
        context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Text::new(format!("Virtual row {index:05}"))
            .build(context)
            .map(|node| crate::layout::fixed(node, 620.0, 28.0))
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
                format!("component with key {key:?} is not mounted"),
            )
        })
}
