use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(all(feature = "video", feature = "bench-support"))]
use criterion::{BatchSize, BenchmarkId};
#[cfg(all(feature = "video", feature = "bench-support"))]
use std::hint::black_box;
#[cfg(all(feature = "video", feature = "bench-support"))]
use tgui::video::bench_support::{
    BenchConvertedVideoFrame, BenchVideoFrameConverter, BenchVideoFrameKind,
};

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_frame_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_frame_conversion");
    let cases = [
        (
            "rgba_passthrough_1080p",
            BenchVideoFrameKind::Rgba,
            1920,
            1080,
            None,
        ),
        (
            "rgb24_expand_1080p",
            BenchVideoFrameKind::Rgb24,
            1920,
            1080,
            None,
        ),
        (
            "nv12_direct_yuv_1080p",
            BenchVideoFrameKind::Nv12,
            1920,
            1080,
            None,
        ),
        (
            "nv12_downscale_rgba_1080p",
            BenchVideoFrameKind::Nv12,
            1920,
            1080,
            Some((960, 540)),
        ),
        (
            "yuv420p_direct_yuv_4k",
            BenchVideoFrameKind::Yuv420p,
            3840,
            2160,
            None,
        ),
    ];

    for (name, kind, width, height, target_size) in cases {
        group.bench_with_input(BenchmarkId::new(name, width), &kind, |b, &kind| {
            b.iter_batched(
                || BenchVideoFrameConverter::new(kind, width, height, target_size),
                |mut converter| {
                    let converted: BenchConvertedVideoFrame = converter.convert();
                    black_box((
                        converted.is_yuv,
                        converted.width,
                        converted.height,
                        converted.decoded_bytes,
                        converted.revision,
                    ));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_frame_sequence_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_frame_sequence_conversion");
    let cases = [
        (
            "rgba_passthrough_1080p_120f",
            BenchVideoFrameKind::Rgba,
            1920,
            1080,
            None,
        ),
        (
            "nv12_direct_yuv_1080p_120f",
            BenchVideoFrameKind::Nv12,
            1920,
            1080,
            None,
        ),
        (
            "nv12_downscale_rgba_1080p_120f",
            BenchVideoFrameKind::Nv12,
            1920,
            1080,
            Some((960, 540)),
        ),
    ];

    for (name, kind, width, height, target_size) in cases {
        group.bench_with_input(BenchmarkId::new(name, width), &120_usize, |b, &frames| {
            b.iter_batched(
                || BenchVideoFrameConverter::new(kind, width, height, target_size),
                |mut converter| {
                    let mut yuv_frames = 0_usize;
                    let mut decoded_bytes = 0_u64;
                    let mut last_revision = 0_u64;

                    for _ in 0..frames {
                        let converted: BenchConvertedVideoFrame = converter.convert();
                        yuv_frames += usize::from(converted.is_yuv);
                        decoded_bytes = decoded_bytes.saturating_add(converted.decoded_bytes);
                        last_revision = converted.revision;
                    }

                    black_box((yuv_frames, decoded_bytes, last_revision));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_frame_conversion(_c: &mut Criterion) {
    eprintln!(
        "Skipping video_frame_conversion benchmarks: video + bench-support features not enabled"
    );
}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_frame_sequence_conversion(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_video_frame_conversion,
    bench_video_frame_sequence_conversion
);
criterion_main!(benches);
