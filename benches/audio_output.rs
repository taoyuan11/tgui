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
fn enqueue_fragmented_samples(
    output: &audio_bench::BenchAudioOutput,
    total_samples: usize,
    chunk_samples: usize,
) {
    let mut remaining = total_samples;
    while remaining > 0 {
        let take = remaining.min(chunk_samples.max(1));
        audio_bench::enqueue_chunk(output, make_samples(take), take as u64 * 4);
        remaining -= take;
    }
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
                        audio_bench::reset_diagnostics();
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
                        black_box((
                            buffer,
                            audio_bench::played_frames(&output),
                            audio_bench::diagnostics(),
                        ));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_rate_adjusted_write_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_output_rate_adjusted_write_f32");

    for rate in [0.5_f32, 1.5, 2.0] {
        group.bench_with_input(BenchmarkId::from_parameter(rate), &rate, |b, &rate| {
            let frames = 512_usize;
            let sample_count = frames * 2;
            b.iter_batched(
                || {
                    audio_bench::reset_diagnostics();
                    let output = audio_bench::make_output(2, 0.75, false, true);
                    audio_bench::set_playback_rate(&output, rate);
                    audio_bench::enqueue_chunk(&output, make_samples(sample_count * 8), 32 * 1024);
                    (output, vec![0.0_f32; sample_count])
                },
                |(output, mut buffer)| {
                    audio_bench::write_f32(&mut buffer, &output);
                    black_box((
                        buffer,
                        audio_bench::played_frames(&output),
                        audio_bench::queued_samples(&output),
                        audio_bench::diagnostics(),
                    ));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_fragmented_queue_write_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_output_fragmented_queue_write_f32");

    for chunk_frames in [1_usize, 4, 16, 128] {
        group.bench_with_input(
            BenchmarkId::from_parameter(chunk_frames),
            &chunk_frames,
            |b, &chunk_frames| {
                let frames = 512_usize;
                let sample_count = frames * 2;
                let chunk_samples = chunk_frames * 2;
                b.iter_batched(
                    || {
                        audio_bench::reset_diagnostics();
                        let output = audio_bench::make_output(2, 1.0, false, true);
                        enqueue_fragmented_samples(&output, sample_count * 2, chunk_samples);
                        (output, vec![0.0_f32; sample_count])
                    },
                    |(output, mut buffer)| {
                        audio_bench::write_f32(&mut buffer, &output);
                        black_box((
                            buffer,
                            audio_bench::played_frames(&output),
                            audio_bench::queued_samples(&output),
                            audio_bench::underflowing(&output),
                            audio_bench::diagnostics(),
                        ));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn run_sustained_callbacks(
    callback_frames: usize,
    callback_count: usize,
    producer_chunk_frames: usize,
    playback_rate: f32,
) -> audio_bench::AudioOutputDiagnostics {
    audio_bench::reset_diagnostics();
    let channels = 2_usize;
    let callback_samples = callback_frames * channels;
    let producer_chunk_samples = producer_chunk_frames * channels;
    let source_samples_per_callback =
        ((callback_samples as f32 * playback_rate.max(1.0)).ceil() as usize).div_ceil(channels)
            * channels;
    let output = audio_bench::make_output(channels as u16, 1.0, false, true);
    if (playback_rate - 1.0).abs() > f32::EPSILON {
        audio_bench::set_playback_rate(&output, playback_rate);
    }

    enqueue_fragmented_samples(
        &output,
        source_samples_per_callback * 4,
        producer_chunk_samples,
    );
    let low_water_samples = source_samples_per_callback * 2;
    let mut buffer = vec![0.0_f32; callback_samples];

    for _ in 0..callback_count {
        if audio_bench::queued_samples(&output) < low_water_samples as u64 {
            enqueue_fragmented_samples(
                &output,
                source_samples_per_callback * 2,
                producer_chunk_samples,
            );
        }
        audio_bench::write_f32(&mut buffer, &output);
        black_box(buffer[0]);
    }

    audio_bench::diagnostics()
}

#[cfg(all(feature = "audio", feature = "bench-support"))]
fn bench_audio_sustained_callback_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_output_sustained_callback_simulation");
    let cases = [
        ("normal_chunk_64f", 1.0_f32, 64_usize),
        ("normal_chunk_512f", 1.0_f32, 512),
        ("rate_1_5_chunk_512f", 1.5_f32, 512),
    ];

    for (name, playback_rate, producer_chunk_frames) in cases {
        group.bench_with_input(
            BenchmarkId::new(name, producer_chunk_frames),
            &producer_chunk_frames,
            |b, &producer_chunk_frames| {
                b.iter(|| {
                    let diagnostics = run_sustained_callbacks(
                        black_box(512),
                        black_box(96),
                        black_box(producer_chunk_frames),
                        black_box(playback_rate),
                    );
                    black_box(diagnostics);
                });
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
                        audio_bench::reset_diagnostics();
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
                        black_box((
                            buffer,
                            audio_bench::played_frames(&output),
                            audio_bench::diagnostics(),
                        ));
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
fn bench_audio_rate_adjusted_write_f32(_c: &mut Criterion) {}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_fragmented_queue_write_f32(_c: &mut Criterion) {}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_sustained_callback_simulation(_c: &mut Criterion) {}

#[cfg(not(all(feature = "audio", feature = "bench-support")))]
fn bench_audio_http_options(_c: &mut Criterion) {}

criterion_group!(
    benches,
    bench_audio_write_f32,
    bench_audio_write_i16,
    bench_audio_rate_adjusted_write_f32,
    bench_audio_fragmented_queue_write_f32,
    bench_audio_sustained_callback_simulation,
    bench_audio_http_options,
);
criterion_main!(benches);
