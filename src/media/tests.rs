use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::ui::widget::Rect;

use super::loader::load_media_document;
use super::types::ImageSnapshot;
use super::{MediaManager, MediaSource, RasterRequest, TextureFrame};

const ONE_BY_ONE_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x4C,
    0x00, 0x3B,
];
const SIMPLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#22c55e"/></svg>"##;

#[test]
fn svg_rasterizes_per_requested_size_and_reuses_cached_texture() {
    let mut document =
        load_media_document(&MediaSource::bytes(SIMPLE_SVG)).expect("embedded SVG should decode");
    let invalidation = InvalidationSignal::new();
    let budget = ResourceBudget::DEFAULT;

    let first = document
        .texture_for(
            RasterRequest {
                width: 20,
                height: 40,
            },
            &invalidation,
            &budget,
        )
        .expect("SVG rasterization should work")
        .expect("SVG should produce a texture");
    let second = document
        .texture_for(
            RasterRequest {
                width: 20,
                height: 40,
            },
            &invalidation,
            &budget,
        )
        .expect("SVG rasterization should work")
        .expect("SVG should produce a texture");
    let third = document
        .texture_for(
            RasterRequest {
                width: 40,
                height: 80,
            },
            &invalidation,
            &budget,
        )
        .expect("SVG rasterization should work")
        .expect("SVG should produce a texture");

    assert_eq!(first.size(), (20, 40));
    assert_eq!(third.size(), (40, 80));
    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), third.id());
}

#[test]
fn svg_raster_request_is_clamped_to_max_dimension() {
    let mut document = load_media_document(&MediaSource::bytes(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="2048"><rect width="4096" height="2048" fill="#2563eb"/></svg>"##,
    ))
    .expect("large SVG should decode");
    let invalidation = InvalidationSignal::new();

    let texture = document
        .texture_for(
            RasterRequest {
                width: 4096,
                height: 2048,
            },
            &invalidation,
            &ResourceBudget::DEFAULT,
        )
        .expect("SVG rasterization should work")
        .expect("SVG should produce a texture");

    assert_eq!(texture.size(), (2048, 1024));
}

#[test]
fn raster_request_uses_physical_pixels() {
    let frame = Rect::new(0.0, 0.0, 120.0, 80.0);

    let request_1x = RasterRequest::from_frame(frame, 1.0).expect("logical frame should rasterize");
    let request_2x = RasterRequest::from_frame(frame, 2.0).expect("scaled frame should rasterize");

    assert_eq!(request_1x.width(), 120);
    assert_eq!(request_1x.height(), 80);
    assert_eq!(request_2x.width(), 240);
    assert_eq!(request_2x.height(), 160);
}

#[test]
fn canvas_shadow_cache_reuses_matching_texture() {
    let media = MediaManager::new(InvalidationSignal::new());
    let first = media
        .canvas_shadow_texture(42, 16, 16, || {
            Ok(TextureFrame::new(16, 16, vec![0; 16 * 16 * 4]))
        })
        .expect("shadow rasterization should succeed")
        .expect("shadow texture should be cached");
    let second = media
        .canvas_shadow_texture(42, 16, 16, || {
            Ok(TextureFrame::new(16, 16, vec![255; 16 * 16 * 4]))
        })
        .expect("shadow cache lookup should succeed")
        .expect("shadow texture should be cached");
    let third = media
        .canvas_shadow_texture(43, 16, 16, || {
            Ok(TextureFrame::new(16, 16, vec![255; 16 * 16 * 4]))
        })
        .expect("new shadow cache entry should succeed")
        .expect("shadow texture should be cached");

    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), third.id());
}

#[test]
fn svg_from_path_resolves_relative_local_images() {
    let temp_dir = unique_temp_dir();
    fs::create_dir_all(&temp_dir).expect("temporary directory should exist");
    let image_path = temp_dir.join("pixel.gif");
    let svg_path = temp_dir.join("doc.svg");
    fs::write(&image_path, ONE_BY_ONE_GIF).expect("image fixture should be written");
    fs::write(
        &svg_path,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#,
    )
    .expect("svg fixture should be written");

    let document = load_media_document(&MediaSource::path(&svg_path))
        .expect("SVG with relative local image should decode");
    assert_eq!(document.intrinsic_size.width, 8.0);
    assert_eq!(document.intrinsic_size.height, 8.0);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn svg_from_url_resolves_relative_http_images() {
    let server = TestServer::new(HashMap::from([
        (
            "/doc.svg".to_string(),
            TestResponse::new(
                "image/svg+xml",
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#
                    .to_vec(),
            ),
        ),
        (
            "/pixel.gif".to_string(),
            TestResponse::new("image/gif", ONE_BY_ONE_GIF.to_vec()),
        ),
    ]));

    let document = load_media_document(&MediaSource::url(format!("{}/doc.svg", server.base_url)))
        .expect("SVG with relative HTTP image should decode");
    assert_eq!(document.intrinsic_size.width, 8.0);
    assert_eq!(document.intrinsic_size.height, 8.0);
}

#[test]
fn embedded_svg_rejects_relative_local_image_references() {
    let error = match load_media_document(&MediaSource::bytes(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="pixel.gif" width="8" height="8"/></svg>"#,
    )) {
        Ok(_) => panic!("embedded SVG should reject relative local image references"),
        Err(error) => error,
    };

    assert!(error
        .to_string()
        .contains("unsupported SVG image reference"));
}

#[test]
fn svg_external_image_failures_surface_as_media_errors() {
    let server = TestServer::new(HashMap::from([(
        "/doc.svg".to_string(),
        TestResponse::new(
            "image/svg+xml",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="missing.gif" width="8" height="8"/></svg>"#
                .to_vec(),
        ),
    )]));

    let error = match load_media_document(&MediaSource::url(format!("{}/doc.svg", server.base_url)))
    {
        Ok(_) => panic!("missing external image should fail the SVG"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("failed to fetch SVG image reference"));
}

#[test]
fn media_manager_uses_intrinsic_size_before_svg_rasterization() {
    let media = MediaManager::new(InvalidationSignal::new());
    let source = MediaSource::bytes(SIMPLE_SVG);

    let metadata = wait_for_snapshot(&media, &source, None);
    assert_eq!(metadata.intrinsic_size.width, 10.0);
    assert_eq!(metadata.intrinsic_size.height, 20.0);
    assert!(metadata.texture.is_none());

    let rasterized = wait_for_snapshot(
        &media,
        &source,
        Some(RasterRequest {
            width: 20,
            height: 40,
        }),
    );
    assert_eq!(
        rasterized
            .texture
            .expect("SVG should rasterize once a target size is requested")
            .size(),
        (20, 40)
    );
}

#[test]
fn embedded_raster_bytes_are_available_without_background_loader_delay() {
    let media = MediaManager::new(InvalidationSignal::new());
    let source = MediaSource::bytes(ONE_BY_ONE_GIF);

    let snapshot = media.image_snapshot(&source, None);

    assert!(!snapshot.loading);
    assert!(snapshot.error.is_none());
    assert_eq!(snapshot.intrinsic_size.width, 1.0);
    assert_eq!(snapshot.intrinsic_size.height, 1.0);
}

#[test]
fn raster_document_rasterizes_requested_size_and_reuses_cached_texture() {
    let media = MediaManager::new(InvalidationSignal::new());
    let source = MediaSource::bytes(ONE_BY_ONE_GIF);

    let first = wait_for_snapshot(
        &media,
        &source,
        Some(RasterRequest {
            width: 64,
            height: 64,
        }),
    );
    let second = wait_for_snapshot(
        &media,
        &source,
        Some(RasterRequest {
            width: 64,
            height: 64,
        }),
    );

    let first_texture = first.texture.expect("raster image should decode");
    let second_texture = second.texture.expect("raster image should decode");
    assert_eq!(first_texture.size(), (64, 64));
    assert_eq!(first_texture.id(), second_texture.id());
}

#[test]
fn raster_document_keeps_previous_texture_while_new_size_is_loading() {
    let media = MediaManager::new(InvalidationSignal::new());
    let source = MediaSource::bytes(ONE_BY_ONE_GIF);

    let first = wait_for_snapshot(
        &media,
        &source,
        Some(RasterRequest {
            width: 64,
            height: 64,
        }),
    );
    let first_texture = first.texture.expect("raster image should decode");

    let resized = media.image_snapshot(
        &source,
        Some(RasterRequest {
            width: 192,
            height: 192,
        }),
    );

    assert!(resized.loading);
    let fallback_texture = resized
        .texture
        .expect("previous raster texture should remain available while resizing");
    assert_eq!(fallback_texture.id(), first_texture.id());
}

fn wait_for_snapshot(
    media: &MediaManager,
    source: &MediaSource,
    raster_request: Option<RasterRequest>,
) -> ImageSnapshot {
    for _ in 0..150 {
        let snapshot = media.image_snapshot(source, raster_request);
        if !snapshot.loading {
            return snapshot;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for media snapshot");
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tests")
        .as_nanos();
    std::env::temp_dir().join(format!("tgui-svg-test-{nanos}"))
}

struct TestResponse {
    content_type: &'static str,
    body: Vec<u8>,
    status_line: &'static str,
}

impl TestResponse {
    fn new(content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            content_type,
            body,
            status_line: "HTTP/1.1 200 OK",
        }
    }
}

struct TestServer {
    base_url: String,
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn new(routes: HashMap<String, TestResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        listener
            .set_nonblocking(true)
            .expect("test server should be non-blocking");
        let address = listener
            .local_addr()
            .expect("test server should expose an address");
        let base_url = format!("http://{address}");
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = thread::spawn(move || loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 4096];
                    let bytes_read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");

                    let response = routes.get(path);
                    let (status_line, content_type, body) = if let Some(response) = response {
                        (
                            response.status_line,
                            response.content_type,
                            response.body.clone(),
                        )
                    } else {
                        ("HTTP/1.1 404 Not Found", "text/plain", b"missing".to_vec())
                    };

                    let header = format!(
                        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("test server accept failed: {error}"),
            }
        });

        Self {
            base_url,
            shutdown_tx,
            handle: Some(handle),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
