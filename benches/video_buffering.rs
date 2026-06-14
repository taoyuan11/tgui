use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(all(feature = "video", feature = "bench-support"))]
use criterion::{BatchSize, BenchmarkId};
#[cfg(all(feature = "video", feature = "bench-support"))]
use std::hint::black_box;
#[cfg(all(feature = "video", feature = "bench-support"))]
use tgui::video::bench_support as video_bench;

#[cfg(all(feature = "video", feature = "bench-support"))]
fn positions(frame_count: usize) -> Vec<u64> {
    (0..frame_count).map(|index| index as u64 * 33).collect()
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn positions_with_interval(frame_count: usize, frame_interval_ms: u64) -> Vec<u64> {
    (0..frame_count)
        .map(|index| index as u64 * frame_interval_ms)
        .collect()
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_buffer_decisions(c: &mut Criterion) {
    c.bench_function("video_buffer_decision_helpers", |b| {
        b.iter(|| {
            let throttle = video_bench::bench_should_throttle_demux(
                black_box(false),
                black_box(false),
                black_box(true),
                black_box(false),
            );
            let constrained = video_bench::bench_buffering_constrained_by_memory_limit(
                black_box(48 * 1024 * 1024),
                black_box(64 * 1024 * 1024),
                black_box(8 * 1024 * 1024),
            );
            let total = video_bench::bench_total_buffered_memory_bytes(
                black_box(8 * 1024 * 1024),
                black_box(24 * 1024 * 1024),
                black_box(6 * 1024 * 1024),
            );
            let target = video_bench::bench_video_buffer_target_satisfied(
                black_box(700),
                black_box(500),
                black_box(Some(2_000)),
                black_box(false),
            );
            let should_buffer =
                video_bench::bench_should_buffer_video(black_box(180), black_box(500), None);
            let rebuffer = video_bench::bench_should_buffer_for_rebuffer(
                black_box(false),
                black_box(true),
                black_box(constrained),
            );
            black_box((
                throttle,
                constrained,
                total,
                target,
                should_buffer,
                rebuffer,
            ));
        });
    });
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_queue_accounting(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_accounting");

    for frame_count in [30_usize, 300, 1200] {
        let positions = positions(frame_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(frame_count),
            &positions,
            |b, positions| {
                b.iter_batched(
                    || {
                        let queue = video_bench::BenchVideoQueue::new();
                        queue.replace_generation(1);
                        queue.push_frames(1, positions, 32 * 1024);
                        queue
                    },
                    |queue| {
                        let ready = queue.ready_frame_count(1);
                        let memory = queue.ready_memory_bytes(1);
                        let tail = queue.tail_end_position(1);
                        let head = queue.head_frame_memory_bytes(1);
                        let popped = queue.pop_front_matching(1);
                        black_box((ready, memory, tail, head, popped));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_high_fps_queue_accounting(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_high_fps_queue_accounting");

    let cases = [
        ("1080p_60fps_2s", 120_usize, 16_u64, 1920_u64 * 1080 * 4),
        ("4k_60fps_2s", 120, 16, 3840_u64 * 2160 * 4),
        ("4k_120fps_2s", 240, 8, 3840_u64 * 2160 * 4),
    ];

    for (name, frame_count, interval_ms, frame_bytes) in cases {
        let positions = positions_with_interval(frame_count, interval_ms);
        group.bench_with_input(
            BenchmarkId::new(name, frame_count),
            &positions,
            |b, positions| {
                b.iter_batched(
                    || {
                        let queue = video_bench::BenchVideoQueue::new();
                        queue.replace_generation(1);
                        queue.push_frames(1, positions, frame_bytes);
                        queue
                    },
                    |queue| {
                        let ready = queue.ready_frame_count(1);
                        let memory = queue.ready_memory_bytes(1);
                        let tail = queue.tail_end_position(1);
                        let constrained = video_bench::bench_buffering_constrained_by_memory_limit(
                            memory,
                            512 * 1024 * 1024,
                            frame_bytes,
                        );
                        black_box((ready, memory, tail, constrained));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_pts_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_pts_to_duration");

    for time_base in [(1_i32, 1_000_i32), (1, 90_000), (1001, 30_000)] {
        group.bench_with_input(
            BenchmarkId::new("time_base", format!("{}/{}", time_base.0, time_base.1)),
            &time_base,
            |b, &(num, den)| {
                b.iter(|| {
                    let duration =
                        video_bench::bench_pts_to_duration(black_box(Some(12_345)), num, den);
                    black_box(duration);
                });
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "video", feature = "bench-support"))]
fn bench_video_compressed_byte_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_compressed_byte_distribution");

    for frame_count in [30_usize, 300, 1200] {
        group.bench_with_input(
            BenchmarkId::from_parameter(frame_count),
            &frame_count,
            |b, &frame_count| {
                b.iter(|| {
                    let total = video_bench::bench_distribute_video_compressed_bytes(
                        black_box(frame_count),
                        black_box(24 * 1024 * 1024),
                    );
                    black_box(total);
                });
            },
        );
    }

    group.finish();
}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_buffer_decisions(_c: &mut Criterion) {
    eprintln!("Skipping video_buffering benchmarks: video + bench-support features not enabled");
}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_queue_accounting(_c: &mut Criterion) {}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_high_fps_queue_accounting(_c: &mut Criterion) {}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_pts_conversion(_c: &mut Criterion) {}

#[cfg(not(all(feature = "video", feature = "bench-support")))]
fn bench_video_compressed_byte_distribution(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_video_buffer_decisions,
    bench_video_queue_accounting,
    bench_video_high_fps_queue_accounting,
    bench_video_pts_conversion,
    bench_video_compressed_byte_distribution,
);
criterion_main!(benches);
