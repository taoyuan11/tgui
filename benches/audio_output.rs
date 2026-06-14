use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(all(feature = "audio", feature = "bench-support"))]
use criterion::{BatchSize, BenchmarkId};
#[cfg(all(feature = "audio", feature = "bench-support"))]
use std::hint::black_box;
#[cfg(all(feature = "audio", feature = "bench-support"))]
use tgui::audio::bench_support as audio_bench;

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn make_samples(sample_count: usize) -> Vec<f32> {
    (0..sample_count)
        .map(|index| ((index % 97) as f32 / 96.0) * 2.0 - 1.0)
        .collect()
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_write_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_output_write_f32");

    for frames in [128_usize, 512, 2048] {
        group.bench_with_input(
            BenchmarkId::from_parameter(frames),
            &frames,
            |b, &frames| {
                let sample_count = frames * 2;
                b.iter_batched(
                    || {
                        let output = audio_bench::make_output(2, 0.75, false, true);
                        audio_bench::enqueue_chunk(
                            &output,
                            make_samples(sample_count * 4),
                            16 * 1024,
                        );
                        (output, vec![0.0_f32; sample_count])
                    },
                    |(output, mut buffer)| {
                        audio_bench::write_f32(&mut buffer, &output);
                        black_box((buffer, audio_bench::played_frames(&output)));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_write_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_output_write_i16");

    for frames in [128_usize, 512, 2048] {
        group.bench_with_input(
            BenchmarkId::from_parameter(frames),
            &frames,
            |b, &frames| {
                let sample_count = frames * 2;
                b.iter_batched(
                    || {
                        let output = audio_bench::make_output(2, 0.75, false, true);
                        audio_bench::enqueue_chunk(
                            &output,
                            make_samples(sample_count * 4),
                            16 * 1024,
                        );
                        (output, vec![0_i16; sample_count])
                    },
                    |(output, mut buffer)| {
                        audio_bench::write_i16(&mut buffer, &output);
                        black_box((buffer, audio_bench::played_frames(&output)));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_http_options(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_ffmpeg_http_options");

    for header_count in [0_usize, 4, 16] {
        let headers: Vec<(String, String)> = (0..header_count)
            .map(|index| (format!("X-Test-{index}"), format!("value-{index}")))
            .collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(header_count),
            &headers,
            |b, headers| {
                b.iter(|| {
                    let result = audio_bench::build_http_options(black_box(headers));
                    let _ = black_box(result);
                });
            },
        );
    }

    group.finish();
}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_write_f32(_c: &mut Criterion) {
    eprintln!("Skipping audio_output benchmarks: audio + bench-support features not enabled");
}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_write_i16(_c: &mut Criterion) {}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_http_options(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_audio_write_f32,
    bench_audio_write_i16,
    bench_audio_http_options,
);
criterion_main!(benches);
