use std::sync::Arc;
use tgui::core::{ImageHandle, ResourceRevision};
#[cfg(feature = "image")]
use tgui::core::{RevisionSet, WindowId};
use tgui::media::{
    CpuImageCache, DecodedImage, GpuTextureCache, ImageCompletion, ImageLoadError, ImagePayload,
    ImagePresentation, ImageRegistry, ImageRequestKey, ImageSize, ImageSource, ImageSourceResolver,
    ImageTextureUploader, LocalImageSourceResolver,
};
#[cfg(feature = "image")]
use tgui::media::{ImageDecodeRequest, spawn_image_decode};
#[cfg(feature = "image")]
use tgui::ui_channel;
use tgui::{Error, Result};

const RED_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

fn image(rgba: [u8; 4]) -> DecodedImage {
    DecodedImage::new(ImageSize::new(1, 1).unwrap(), rgba).unwrap()
}

#[test]
fn exact_request_identity_deduplicates_and_rebinding_advances_generation() {
    let first_key = ImageRequestKey::new(ImageSource::bytes([1_u8, 2, 3].as_slice()));
    let next_key = ImageRequestKey::new(ImageSource::bytes([4_u8, 5, 6].as_slice()));
    let mut registry = ImageRegistry::new();

    let first = registry.request(first_key.clone());
    assert!(first.needs_decode);
    assert_eq!(registry.request(first_key.clone()).handle, first.handle);
    let ready = tgui::media::ImageDecodeResult {
        handle: first.handle,
        key: first_key.clone(),
        decoded: Ok(image([255, 0, 0, 255])),
    };
    assert!(matches!(
        registry.complete(first.handle.stamp(), &ready),
        ImageCompletion::Ready { .. }
    ));

    let next = registry.replace(first.handle, next_key.clone()).unwrap();
    assert_eq!(next.handle.slot(), first.handle.slot());
    assert_ne!(next.handle.generation(), first.handle.generation());
    assert_eq!(
        registry.presentation(next.handle),
        ImagePresentation::Texture(first.handle)
    );

    let stale = tgui::media::ImageDecodeResult {
        handle: first.handle,
        key: first_key,
        decoded: Ok(image([255, 0, 0, 255])),
    };
    assert_eq!(
        registry.complete(first.handle.stamp(), &stale),
        ImageCompletion::Stale
    );

    let current = tgui::media::ImageDecodeResult {
        handle: next.handle,
        key: next_key,
        decoded: Ok(image([0, 255, 0, 255])),
    };
    assert_eq!(
        registry.complete(next.handle.stamp(), &current),
        ImageCompletion::Ready {
            handle: next.handle,
            intrinsic_size_changed: false,
        }
    );
    assert_eq!(
        registry.presentation(next.handle),
        ImagePresentation::Texture(next.handle)
    );
}

#[cfg(feature = "image")]
#[test]
fn worker_results_validate_source_generation_and_resource_revision() {
    let target = WindowId::from_parts(1, 1);
    let key = ImageRequestKey::new(ImageSource::bytes(RED_PNG));
    let mut registry = ImageRegistry::new();
    let old = registry.request(key.clone());
    let (dispatcher, inbox) = ui_channel();

    let stale_revision =
        ImageDecodeRequest::new(target, old.handle, key.clone(), RevisionSet::ZERO);
    spawn_image_decode(
        stale_revision,
        Arc::new(LocalImageSourceResolver),
        dispatcher.clone(),
    )
    .join()
    .unwrap();

    let current = registry.replace(old.handle, key.clone()).unwrap();
    let revisions = RevisionSet::new(
        tgui::core::LayoutRevision::ZERO,
        tgui::core::SceneRevision::ZERO,
        ResourceRevision::new(1),
        tgui::core::SemanticRevision::ZERO,
    );
    let valid = ImageDecodeRequest::new(target, current.handle, key, revisions);
    spawn_image_decode(valid, Arc::new(LocalImageSourceResolver), dispatcher)
        .join()
        .unwrap();

    let mut cpu = CpuImageCache::new(4, 8).unwrap();
    let batch = registry
        .drain_decode_results(&inbox, target, revisions, &mut cpu)
        .unwrap();
    assert_eq!(batch.stale, 1);
    assert_eq!(
        batch.completions,
        [ImageCompletion::Ready {
            handle: current.handle,
            intrinsic_size_changed: true,
        }]
    );
    assert_eq!(cpu.stats().resident_bytes, 4);
}

#[test]
fn url_transport_is_explicit_and_does_not_change_request_identity() {
    let source = ImageSource::url("https://example.invalid/image.png").unwrap();
    let key = ImageRequestKey::new(source.clone());
    assert_eq!(key, ImageRequestKey::new(source.clone()));
    assert_eq!(
        LocalImageSourceResolver.resolve(&source),
        Err(ImageLoadError::NetworkResolverRequired)
    );
    assert!(ImageSource::url("file:///tmp/image.png").is_err());
}

#[test]
fn cpu_cache_is_bounded_and_reports_hits_failures_and_evictions() {
    let one = ImageRequestKey::new(ImageSource::bytes([1_u8].as_slice()));
    let two = ImageRequestKey::new(ImageSource::bytes([2_u8].as_slice()));
    let mut cache = CpuImageCache::new(4, 8).unwrap();

    cache.insert(one.clone(), image([1, 2, 3, 4])).unwrap();
    assert!(cache.get(&one).is_some());
    cache.insert(two.clone(), image([5, 6, 7, 8])).unwrap();
    assert!(cache.peek(&one).is_none());
    assert!(cache.get(&two).is_some());
    assert!(cache.get(&one).is_none());
    let oversized = DecodedImage::new(ImageSize::new(3, 1).unwrap(), [0_u8; 12]).unwrap();
    assert!(cache.insert(one, oversized).is_err());

    let stats = cache.stats();
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.hits, 2);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.failures, 1);
    assert_eq!(stats.evictions, 1);
    assert_eq!(stats.resident_bytes, 4);
    assert_eq!(stats.peak_bytes, 4);
}

struct FakeUploader;

impl ImageTextureUploader for FakeUploader {
    type Texture = Arc<[u8]>;

    fn upload_image(
        &mut self,
        _handle: ImageHandle,
        image: &DecodedImage,
    ) -> Result<Self::Texture> {
        Ok(Arc::from(image.rgba8()))
    }
}

struct FailingUploader;

impl ImageTextureUploader for FailingUploader {
    type Texture = Arc<[u8]>;

    fn upload_image(
        &mut self,
        _handle: ImageHandle,
        _image: &DecodedImage,
    ) -> Result<Self::Texture> {
        Err(Error::resource(None, "test upload failure", true))
    }
}

#[test]
fn gpu_cache_owns_texture_objects_and_is_generation_exact_and_bounded() {
    let old = ImageHandle::from_parts(7, 1);
    let current = ImageHandle::from_parts(7, 2);
    let mut cache = GpuTextureCache::new(4, 8).unwrap();

    cache
        .upload(
            &mut FakeUploader,
            old,
            &image([1, 2, 3, 4]),
            ResourceRevision::new(1),
        )
        .unwrap();
    cache
        .upload(
            &mut FakeUploader,
            current,
            &image([5, 6, 7, 8]),
            ResourceRevision::new(2),
        )
        .unwrap();
    assert!(cache.peek(old).is_none());
    let texture = cache.get(current).unwrap();
    assert_eq!(&*texture.texture, &[5, 6, 7, 8]);
    assert_eq!(texture.resource_revision, ResourceRevision::new(2));

    let stats = cache.stats();
    assert_eq!(stats.uploads, 2);
    assert_eq!(stats.upload_bytes, 8);
    assert_eq!(stats.evictions, 1);

    assert!(
        cache
            .upload(
                &mut FailingUploader,
                ImageHandle::from_parts(8, 1),
                &image([0, 0, 0, 0]),
                ResourceRevision::new(3),
            )
            .is_err()
    );
    assert_eq!(cache.stats().failures, 1);
}

#[cfg(feature = "svg")]
#[test]
fn svg_raster_size_is_part_of_identity_and_output() {
    let svg = ImageSource::svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="red"/></svg>"#,
    );
    let small = ImageRequestKey::new(svg.clone());
    let large = ImageRequestKey::new(svg).with_raster_size(ImageSize::new(4, 6).unwrap());
    assert_ne!(small, large);
    let decoded = tgui::media::decode_image(&large, &LocalImageSourceResolver).unwrap();
    assert_eq!(decoded.size(), ImageSize::new(4, 6).unwrap());
    assert_eq!(decoded.byte_len(), 96);
}

#[test]
fn custom_url_resolver_can_supply_encoded_data() {
    struct Resolver;
    impl ImageSourceResolver for Resolver {
        fn resolve(
            &self,
            _source: &ImageSource,
        ) -> std::result::Result<ImagePayload, ImageLoadError> {
            Ok(ImagePayload::Encoded(Arc::from(RED_PNG)))
        }
    }

    let source = ImageSource::url("https://example.invalid/image.png").unwrap();
    let payload = Resolver.resolve(&source).unwrap();
    assert!(matches!(payload, ImagePayload::Encoded(_)));
}
