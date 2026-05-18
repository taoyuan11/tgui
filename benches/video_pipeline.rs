use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tgui::video::bench_support as video_bench;

fn fill_queue(queue: &video_bench::BenchVideoQueue, frame_count: usize, stale_count: usize) {
    queue.replace_generation(2);
    if stale_count > 0 {
        let stale_positions: Vec<u64> = (0..stale_count as u64)
            .map(|index| index.saturating_mul(33))
            .collect();
        queue.push_frames(1, &stale_positions, 4096);
    }
    let positions: Vec<u64> = (0..frame_count as u64)
        .map(|index| index.saturating_mul(33))
        .collect();
    queue.push_frames(2, &positions, 4096);
}

fn bench_shared_queue_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_scan");

    for &frames in &[8_usize, 64, 256] {
        let queue = video_bench::BenchVideoQueue::new();
        fill_queue(&queue, frames, frames / 4);

        group.bench_with_input(
            BenchmarkId::new("ready_frame_count", frames),
            &frames,
            |b, _| {
                b.iter(|| black_box(queue.ready_frame_count(black_box(2))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ready_memory_bytes", frames),
            &frames,
            |b, _| {
                b.iter(|| black_box(queue.ready_memory_bytes(black_box(2))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("tail_end_position", frames),
            &frames,
            |b, _| {
                b.iter(|| black_box(queue.tail_end_position(black_box(2))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("head_frame_memory_bytes", frames),
            &frames,
            |b, _| {
                b.iter(|| black_box(queue.head_frame_memory_bytes(black_box(2))));
            },
        );
    }

    group.finish();
}

fn bench_shared_queue_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_round_trip");

    for &frames in &[16_usize, 128] {
        group.throughput(Throughput::Elements(frames as u64));
        group.bench_with_input(BenchmarkId::new("push_then_pop", frames), &frames, |b, &frames| {
            let queue = video_bench::BenchVideoQueue::new();
            queue.replace_generation(1);
            let positions: Vec<u64> = (0..frames as u64)
                .map(|index| index.saturating_mul(33))
                .collect();
            b.iter(|| {
                queue.push_frames(1, black_box(&positions), 4096);
                while queue.has_frames(1) {
                    queue.pop_front_matching(1);
                }
            });
        });
    }

    group.finish();
}

fn bench_buffering_decisions(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_buffering_decisions");

    let throttle_inputs: [(bool, bool, bool, bool); 8] = [
        (false, false, false, false),
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
        (true, true, false, false),
        (false, true, true, false),
        (true, true, true, true),
    ];
    group.bench_function("should_throttle_demux_8x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &(a, c0, c1, c2) in throttle_inputs.iter() {
                if video_bench::bench_should_throttle_demux(
                    black_box(a),
                    black_box(c0),
                    black_box(c1),
                    black_box(c2),
                ) {
                    acc = acc.wrapping_add(1);
                }
            }
            black_box(acc)
        });
    });

    let memory_inputs: [(u64, u64, u64); 8] = [
        (0, 100 * 1024 * 1024, 0),
        (50 * 1024 * 1024, 100 * 1024 * 1024, 4 * 1024 * 1024),
        (90 * 1024 * 1024, 100 * 1024 * 1024, 4 * 1024 * 1024),
        (95 * 1024 * 1024, 100 * 1024 * 1024, 4 * 1024 * 1024),
        (100 * 1024 * 1024, 100 * 1024 * 1024, 0),
        (10 * 1024 * 1024, 200 * 1024 * 1024, 1 * 1024 * 1024),
        (199 * 1024 * 1024, 200 * 1024 * 1024, 2 * 1024 * 1024),
        (1, 1, 0),
    ];
    group.bench_function("buffering_constrained_by_memory_limit_8x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &(total, limit, next) in memory_inputs.iter() {
                if video_bench::bench_buffering_constrained_by_memory_limit(
                    black_box(total),
                    black_box(limit),
                    black_box(next),
                ) {
                    acc = acc.wrapping_add(1);
                }
            }
            black_box(acc)
        });
    });

    let target_inputs: [(u64, u64, Option<u64>, bool); 6] = [
        (0, 1500, None, false),
        (500, 1500, Some(2000), false),
        (1500, 1500, None, false),
        (1499, 1500, Some(1500), false),
        (200, 1500, Some(180), false),
        (1000, 1500, None, true),
    ];
    group.bench_function("video_buffer_target_satisfied_6x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &(buffered, target, remaining, cap) in target_inputs.iter() {
                if video_bench::bench_video_buffer_target_satisfied(
                    black_box(buffered),
                    black_box(target),
                    black_box(remaining),
                    black_box(cap),
                ) {
                    acc = acc.wrapping_add(1);
                }
            }
            black_box(acc)
        });
    });

    let buffer_inputs: [(u64, u64, Option<u64>); 6] = [
        (0, 2000, None),
        (1000, 2000, Some(5000)),
        (1999, 2000, None),
        (2000, 2000, None),
        (200, 2000, Some(180)),
        (3000, 2000, Some(10_000)),
    ];
    group.bench_function("should_buffer_video_6x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &(buffered, threshold, remaining) in buffer_inputs.iter() {
                if video_bench::bench_should_buffer_video(
                    black_box(buffered),
                    black_box(threshold),
                    black_box(remaining),
                ) {
                    acc = acc.wrapping_add(1);
                }
            }
            black_box(acc)
        });
    });

    group.finish();
}

fn bench_distribute_compressed_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_distribute_compressed_bytes");

    for &frames in &[1_usize, 4, 16, 64] {
        group.bench_with_input(BenchmarkId::new("frames", frames), &frames, |b, &frames| {
            b.iter(|| {
                black_box(video_bench::bench_distribute_video_compressed_bytes(
                    black_box(frames),
                    black_box(10_240),
                ))
            });
        });
    }

    group.finish();
}

fn bench_pts_to_duration(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_pts_to_duration");

    let timestamps: [i64; 16] = [
        0, 1, 33, 66, 99, 132, 200, 333, 500, 1_000, 5_000, 10_000, 33_333, 50_000, 100_000,
        1_000_000,
    ];
    let bases: [(i32, i32); 4] = [(1, 1000), (1, 90_000), (1001, 24_000), (1, 48_000)];

    group.bench_function("batch_16x4", |b| {
        b.iter(|| {
            let mut total = 0u128;
            for &(num, den) in bases.iter() {
                for &timestamp in timestamps.iter() {
                    let duration = video_bench::bench_pts_to_duration(
                        black_box(Some(timestamp)),
                        black_box(num),
                        black_box(den),
                    );
                    if let Some(d) = duration {
                        total = total.wrapping_add(d.as_nanos());
                    }
                }
            }
            black_box(total)
        });
    });

    group.finish();
}

fn bench_playback_clock_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_playback_clock");

    group.bench_function("set_then_read_1024", |b| {
        let clock = video_bench::BenchPlaybackClock::new();
        let mut counter: u64 = 0;
        b.iter(|| {
            for _ in 0..1024 {
                counter = counter.wrapping_add(33_000_000);
                clock.set_position(Duration::from_nanos(counter));
                black_box(clock.position());
            }
        });
    });

    group.finish();
}

criterion_group!(
    video_pipeline_benches,
    bench_shared_queue_scan,
    bench_shared_queue_round_trip,
    bench_buffering_decisions,
    bench_distribute_compressed_bytes,
    bench_pts_to_duration,
    bench_playback_clock_round_trip,
);
criterion_main!(video_pipeline_benches);
