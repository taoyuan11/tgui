use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use super::network::{ffmpeg_http_input_options, validate_ffmpeg_headers};
use super::output::{write_audio_samples, AudioSampleChunk, SharedAudioOutput};
use crate::foundation::error::TguiError;

#[test]
fn muted_audio_still_advances_clock() {
    let shared = Arc::new(SharedAudioOutput {
        queue: Mutex::new(VecDeque::from([AudioSampleChunk {
            samples: vec![0.25, -0.25, 0.5, -0.5],
            offset: 0,
            compressed_bytes: 0,
        }])),
        queued_samples: AtomicU64::new(4),
        queued_compressed_bytes: AtomicU64::new(0),
        playing: AtomicBool::new(true),
        muted: AtomicBool::new(true),
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        played_frames: AtomicU64::new(0),
        channels: 2,
        underflowing: AtomicBool::new(false),
    });

    let mut buffer = [1.0f32; 4];
    write_audio_samples(&mut buffer, &shared);

    assert_eq!(buffer, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(shared.played_frames.load(Ordering::SeqCst), 2);
    assert!(!shared.underflowing.load(Ordering::SeqCst));
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
