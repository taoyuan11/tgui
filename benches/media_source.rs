use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tgui::media::{MediaBytes, MediaSource};

fn hash_source(source: &MediaSource) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn bench_media_bytes_from_vec(c: &mut Criterion) {
    let mut group = c.benchmark_group("media_bytes_from_vec");

    for size in [1024_usize, 256 * 1024, 4 * 1024 * 1024] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let bytes = MediaBytes::from(vec![7_u8; black_box(size)]);
                black_box(bytes);
            });
        });
    }

    group.finish();
}

fn bench_media_bytes_clone_and_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("media_bytes_clone_and_hash");

    for size in [1024_usize, 256 * 1024, 4 * 1024 * 1024] {
        let bytes = MediaBytes::from_shared(Arc::<[u8]>::from(vec![11_u8; size]));
        let source = MediaSource::bytes(bytes);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let cloned = source.clone();
                let hash = hash_source(&cloned);
                black_box(hash);
            });
        });
    }

    group.finish();
}

fn bench_media_source_construction(c: &mut Criterion) {
    c.bench_function("media_source_path_url_bytes_construction", |b| {
        b.iter(|| {
            let path = MediaSource::path(black_box("assets/image.png"));
            let url = MediaSource::url(black_box("https://example.test/image.png"));
            let bytes = MediaSource::bytes(MediaBytes::from_static(black_box(b"tiny")));
            black_box((path, url, bytes));
        });
    });
}

criterion_group!(
    benches,
    bench_media_bytes_from_vec,
    bench_media_bytes_clone_and_hash,
    bench_media_source_construction,
);
criterion_main!(benches);
