use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use tgui::audio::bench_support as audio_bench;

fn make_pcm(frames: usize, channels: u16) -> Vec<f32> {
    let total = frames * channels as usize;
    let mut samples = Vec::with_capacity(total);
    for index in 0..total {
        let phase = (index as f32) * 0.011;
        samples.push(phase.sin() * 0.6);
    }
    samples
}

fn bench_write_audio_samples(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_write_samples");

    let cases = [
        ("playing_unmuted", false, 0.7_f32, true),
        ("playing_muted", true, 0.7_f32, true),
        ("paused", false, 0.7_f32, false),
    ];

    for (label, muted, volume, playing) in cases {
        for &frames in &[1024_usize, 4096] {
            for &channels in &[1_u16, 2] {
                let bench_id =
                    BenchmarkId::new(label, format!("frames={}_ch={}", frames, channels));
                group.throughput(Throughput::Elements((frames * channels as usize) as u64));
                group.bench_with_input(bench_id, &(frames, channels), |b, &(frames, channels)| {
                    let output = audio_bench::make_output(channels, volume, muted, playing);
                    let buffer_samples = frames * channels as usize;
                    if playing {
                        let big_chunk = make_pcm(buffer_samples * 4096, channels);
                        audio_bench::enqueue_chunk(&output, big_chunk, 0);
                    }
                    let mut buffer = vec![0.0_f32; buffer_samples];
                    b.iter(|| {
                        audio_bench::write_f32(black_box(&mut buffer), &output);
                        black_box(buffer[0])
                    });
                });
            }
        }
    }

    group.finish();
}

fn bench_write_audio_samples_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_write_samples_i16");

    for &frames in &[1024_usize, 4096] {
        let channels = 2_u16;
        let bench_id = BenchmarkId::new("playing_unmuted", format!("frames={}", frames));
        group.throughput(Throughput::Elements((frames * channels as usize) as u64));
        group.bench_with_input(bench_id, &frames, |b, &frames| {
            let output = audio_bench::make_output(channels, 0.7, false, true);
            let buffer_samples = frames * channels as usize;
            let big_chunk = make_pcm(buffer_samples * 4096, channels);
            audio_bench::enqueue_chunk(&output, big_chunk, 0);
            let mut buffer = vec![0_i16; buffer_samples];
            b.iter(|| {
                audio_bench::write_i16(black_box(&mut buffer), &output);
                black_box(buffer[0])
            });
        });
    }

    group.finish();
}

fn bench_push_samples(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_push_samples");

    for &frames in &[256_usize, 1024, 4096] {
        let channels = 2_u16;
        let bench_id = BenchmarkId::new("stereo_chunk", frames);
        group.throughput(Throughput::Elements((frames * channels as usize) as u64));
        group.bench_with_input(bench_id, &frames, |b, &frames| {
            let output = audio_bench::make_output(channels, 1.0, false, true);
            let chunk_template = make_pcm(frames, channels);
            b.iter_batched(
                || chunk_template.clone(),
                |chunk| {
                    audio_bench::enqueue_chunk(&output, chunk, 1024);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_http_options(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_http_options");

    let no_headers: Vec<(String, String)> = Vec::new();
    let three_headers: Vec<(String, String)> = vec![
        ("Authorization".to_string(), "Bearer token".to_string()),
        ("Referer".to_string(), "https://example.com/app".to_string()),
        ("Cookie".to_string(), "a=1; b=2".to_string()),
    ];
    let many_headers: Vec<(String, String)> = (0..8)
        .map(|index| {
            (
                format!("X-Header-{index}"),
                format!("value-{index}-payload"),
            )
        })
        .collect();

    for (label, headers) in [
        ("0", &no_headers),
        ("3", &three_headers),
        ("8", &many_headers),
    ] {
        group.bench_with_input(BenchmarkId::new("build", label), headers, |b, headers| {
            b.iter(|| {
                audio_bench::build_http_options(black_box(headers))
                    .expect("bench http options should build");
            });
        });
    }

    group.finish();
}

criterion_group!(
    audio_pipeline_benches,
    bench_write_audio_samples,
    bench_write_audio_samples_i16,
    bench_push_samples,
    bench_http_options,
);
criterion_main!(audio_pipeline_benches);
