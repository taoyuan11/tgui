use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use super::network::{ffmpeg_http_input_options, validate_ffmpeg_headers};
use super::output::{
    build_audio_sample_chunks, write_audio_samples, AudioSampleBatch, AudioSampleChunk,
    SharedAudioOutput,
};
use crate::foundation::error::TguiError;

#[test]
fn muted_audio_still_advances_clock() {
    let samples = vec![0.25, -0.25, 0.5, -0.5];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 0,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(4),
        queued_compressed_bytes: AtomicU64::new(0),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(true),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = [1.0f32; 4];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(buffer, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 2);
    assert_eq!(shared.queued_decoded_bytes.load(Ordering::SeqCst), 0);
    assert!(!shared.underflowing.load(Ordering::SeqCst));
}

#[test]
fn decoded_audio_memory_stays_until_chunk_is_drained() {
    let samples = vec![0.25, -0.25, 0.5, -0.5];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 2,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(4),
        queued_compressed_bytes: AtomicU64::new(2),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut first_half = [0.0f32; 2];
    write_audio_samples(&mut first_half, &shared);

    assert_eq!(
        shared.queued_decoded_bytes.load(Ordering::SeqCst),
        decoded_bytes
    );
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 2);

    let mut second_half = [0.0f32; 2];
    write_audio_samples(&mut second_half, &shared);

    assert_eq!(shared.queued_decoded_bytes.load(Ordering::SeqCst), 0);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 0);
}

#[test]
fn audio_callback_does_not_block_when_queue_lock_is_contended() {
    let samples = vec![0.25, -0.25, 0.5, -0.5];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 4,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(4),
        queued_compressed_bytes: AtomicU64::new(4),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let guard = shared.queue.lock();
    let mut buffer = [1.0f32; 4];
    write_audio_samples(&mut buffer, &shared);
    drop(guard);

    assert_eq!(buffer, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 4);
    assert_eq!(shared.queued_compressed_bytes.load(Ordering::SeqCst), 4);
    assert_eq!(
        shared.queued_decoded_bytes.load(Ordering::SeqCst),
        decoded_bytes
    );
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 0);
    assert!(shared.underflowing.load(Ordering::SeqCst));
}

#[test]
fn playback_rate_above_one_consumes_source_frames_faster() {
    let samples = vec![1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 4.0, 40.0];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 8,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(8),
        queued_compressed_bytes: AtomicU64::new(8),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(2.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = [0.0f32; 4];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(buffer, [1.0, 10.0, 3.0, 30.0]);
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 4);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 0);
}

#[test]
fn rate_adjusted_audio_callback_does_not_block_when_queue_lock_is_contended() {
    let samples = vec![1.0, 10.0, 2.0, 20.0];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 4,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(4),
        queued_compressed_bytes: AtomicU64::new(4),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(2.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let guard = shared.queue.lock();
    let mut buffer = [1.0f32; 4];
    write_audio_samples(&mut buffer, &shared);
    drop(guard);

    assert_eq!(buffer, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 4);
    assert_eq!(shared.queued_compressed_bytes.load(Ordering::SeqCst), 4);
    assert_eq!(
        shared.queued_decoded_bytes.load(Ordering::SeqCst),
        decoded_bytes
    );
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 0);
    assert!(shared.underflowing.load(Ordering::SeqCst));
}

#[test]
fn playback_rate_below_one_consumes_source_frames_slower() {
    let samples = vec![1.0, 10.0, 2.0, 20.0, 3.0, 30.0, 4.0, 40.0];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 8,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(8),
        queued_compressed_bytes: AtomicU64::new(8),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(0.5f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = [0.0f32; 8];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(buffer, [1.0, 10.0, 1.0, 10.0, 2.0, 20.0, 2.0, 20.0]);
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 2);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 4);
}

#[test]
fn rate_adjusted_audio_drops_partial_frame_tail_without_leaking_queue_counters() {
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let decoded_bytes = (samples.capacity() * size_of::<f32>()) as u64;
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes: 8,
            decoded_bytes,
        }])),
        queued_samples: AtomicU64::new(8),
        queued_compressed_bytes: AtomicU64::new(8),
        queued_decoded_bytes: AtomicU64::new(decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(2.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 6,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = [0.0f32; 12];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(&buffer[..6], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(&buffer[6..], &[0.0; 6]);
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 1);
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 0);
    assert_eq!(shared.queued_compressed_bytes.load(Ordering::SeqCst), 0);
    assert_eq!(shared.queued_decoded_bytes.load(Ordering::SeqCst), 0);
    assert!(shared.underflowing.load(Ordering::SeqCst));
}

#[test]
fn small_audio_chunks_are_coalesced_before_queueing() {
    let chunks = build_audio_sample_chunks(
        vec![
            vec![1.0, 1.0],
            vec![2.0, 2.0, 2.0],
            vec![3.0; 4096],
            vec![4.0],
        ],
        4102,
    );

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].samples.len(), 2048);
    assert_eq!(&chunks[0].samples[..5], &[1.0, 1.0, 2.0, 2.0, 2.0]);
    assert!(chunks[0].samples[5..].iter().all(|sample| *sample == 3.0));
    assert_eq!(chunks[0].compressed_bytes, 2048);
    assert_eq!(chunks[1].samples.len(), 2048);
    assert!(chunks[1].samples.iter().all(|sample| *sample == 3.0));
    assert_eq!(chunks[1].compressed_bytes, 2048);
    assert_eq!(chunks[2].samples, vec![3.0, 3.0, 3.0, 3.0, 3.0, 4.0]);
    assert_eq!(chunks[2].compressed_bytes, 6);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.compressed_bytes)
            .sum::<u64>(),
        4102
    );
}

#[test]
fn audio_sample_batch_aligns_chunks_to_channel_frames() {
    let mut batch = AudioSampleBatch::new_for_channels(6);
    let samples = vec![1.0; 4098];

    batch.extend_from_slice(&samples);

    let chunks = batch.into_chunks();
    assert_eq!(
        chunks.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2046, 2046, 6]
    );
    assert!(chunks.iter().all(|chunk| chunk.len() % 6 == 0));
}

#[test]
fn audio_sample_batch_appends_slices_into_target_chunks() {
    let mut batch = AudioSampleBatch::new();
    let first = [1.0, 1.0];
    let second = vec![2.0; 2046];
    let third = [3.0];

    batch.extend_from_slice(&first);
    batch.extend_from_slice(&second);
    batch.extend_from_slice(&third);

    let chunks = build_audio_sample_chunks(batch.into_chunks(), 2049);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].samples.len(), 2048);
    assert_eq!(&chunks[0].samples[..2], &[1.0, 1.0]);
    assert_eq!(chunks[0].samples[2047], 2.0);
    assert_eq!(chunks[0].compressed_bytes, 2048);
    assert_eq!(chunks[1].samples, vec![3.0]);
    assert_eq!(chunks[1].compressed_bytes, 1);
}

#[test]
fn audio_sample_batch_splits_large_slices_into_target_chunks() {
    let mut batch = AudioSampleBatch::new();
    let samples = vec![1.0; 4097];

    batch.extend_from_slice(&samples);

    let chunks = build_audio_sample_chunks(batch.into_chunks(), 4097);

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].samples.len(), 2048);
    assert_eq!(chunks[0].compressed_bytes, 2048);
    assert_eq!(chunks[1].samples.len(), 2048);
    assert_eq!(chunks[1].compressed_bytes, 2048);
    assert_eq!(chunks[2].samples, vec![1.0]);
    assert_eq!(chunks[2].compressed_bytes, 1);
}

#[test]
fn large_audio_chunks_release_decoded_memory_incrementally() {
    let mut batch = AudioSampleBatch::new();
    let samples = vec![1.0; 4096];
    batch.extend_from_slice(&samples);

    let chunks = build_audio_sample_chunks(batch.into_chunks(), 4096);
    let remaining_decoded_bytes = chunks[1..]
        .iter()
        .map(|chunk| chunk.decoded_bytes)
        .sum::<u64>();
    let total_decoded_bytes = chunks.iter().map(|chunk| chunk.decoded_bytes).sum::<u64>();
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from(chunks)),
        queued_samples: AtomicU64::new(4096),
        queued_compressed_bytes: AtomicU64::new(4096),
        queued_decoded_bytes: AtomicU64::new(total_decoded_bytes),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(false),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
        source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = vec![0.0f32; 2048];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(
        shared.queued_decoded_bytes.load(Ordering::SeqCst),
        remaining_decoded_bytes
    );
    assert_eq!(shared.queued_samples.load(Ordering::SeqCst), 2048);
}

#[test]
fn invalid_headers_are_rejected() {
    let invalid_name = vec![("Bad\nHeader".to_string(), "value".to_string())];
    let invalid_value = vec![("Authorization".to_string(), "Bearer\ntoken".to_string())];

    assert!(matches!(
        validate_ffmpeg_headers("audio", &invalid_name),
        Err(TguiError::Media(message)) if message.contains("invalid line break")
    ));
    assert!(matches!(
        validate_ffmpeg_headers("audio", &invalid_value),
        Err(TguiError::Media(message)) if message.contains("invalid line break")
    ));
}

#[test]
fn http_sources_serialize_custom_headers() {
    let options = ffmpeg_http_input_options(
        "audio",
        &[
            ("Authorization".to_string(), "Bearer token".to_string()),
            ("Referer".to_string(), "https://example.com/app".to_string()),
        ],
    )
    .expect("http options should build");

    assert_eq!(
        options.get("headers"),
        Some("Authorization: Bearer token\r\nReferer: https://example.com/app\r\n")
    );
}

#[test]
fn http_sources_set_open_and_read_timeouts() {
    let options = ffmpeg_http_input_options("audio", &[]).expect("http options should build");

    assert_eq!(options.get("timeout"), Some("15000000"));
    assert_eq!(options.get("rw_timeout"), Some("15000000"));
}
