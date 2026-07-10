//! Benchmark hooks for the ffmpeg video backend.
//!
//! Only compiled when the `bench-support` feature is enabled. Re-exported up
//! the module chain into `tgui::video::bench_support` so external Criterion
//! benches can drive the queue / decision functions directly without spinning
//! up a full decoder.

use std::sync::Arc;
use std::time::Duration;

use ffmpeg_next as ffmpeg;

use crate::media::TextureFrame;

use super::helpers::{
    buffering_constrained_by_memory_limit, distribute_video_compressed_bytes, pts_to_duration,
    should_buffer_for_rebuffer, should_buffer_video, should_throttle_demux,
    total_buffered_memory_bytes, video_buffer_target_satisfied,
};
use super::queue::{QueuedVideoFrame, SharedPlaybackClock, SharedVideoQueue};

pub struct BenchVideoQueue {
    inner: Arc<SharedVideoQueue>,
}

impl BenchVideoQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SharedVideoQueue::new()),
        }
    }

    pub fn replace_generation(&self, generation: u64) {
        self.inner.replace_generation(generation);
    }

    pub fn push_frames(
        &self,
        generation: u64,
        positions_ms: &[u64],
        decoded_bytes_per_frame: u64,
    ) {
        let texture = Arc::new(TextureFrame::new(1, 1, vec![0u8; 4]));
        let frames: Vec<QueuedVideoFrame> = positions_ms
            .iter()
            .map(|&ms| QueuedVideoFrame {
                generation,
                position: Duration::from_millis(ms),
                end_position: Duration::from_millis(ms + 33),
                texture: texture.clone(),
                decoded_bytes: decoded_bytes_per_frame,
                compressed_bytes: 0,
            })
            .collect();
        self.inner.push_frames(frames);
    }

    pub fn ready_frame_count(&self, generation: u64) -> usize {
        self.inner.ready_frame_count(generation)
    }

    pub fn ready_memory_bytes(&self, generation: u64) -> u64 {
        self.inner.ready_memory_bytes(generation)
    }

    pub fn tail_end_position(&self, generation: u64) -> Option<Duration> {
        self.inner.tail_end_position(generation)
    }

    pub fn head_frame_memory_bytes(&self, generation: u64) -> Option<u64> {
        self.inner.head_frame_memory_bytes(generation)
    }

    pub fn pop_front_matching(&self, generation: u64) -> bool {
        self.inner.pop_front_matching(generation).is_some()
    }

    pub fn has_frames(&self, generation: u64) -> bool {
        self.inner.has_frames(generation)
    }
}

impl Default for BenchVideoQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
pub struct BenchPlaybackClock {
    inner: SharedPlaybackClock,
}

impl BenchPlaybackClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_position(&self, position: Duration) {
        self.inner.set_position(position);
    }

    pub fn position(&self) -> Duration {
        self.inner.position()
    }
}

pub fn bench_should_throttle_demux(
    compressed_buffer_limit_reached: bool,
    audio_hard_full: bool,
    decoded_video_hard_full: bool,
    video_packet_fuse_tripped: bool,
) -> bool {
    should_throttle_demux(
        compressed_buffer_limit_reached,
        audio_hard_full,
        decoded_video_hard_full,
        video_packet_fuse_tripped,
    )
}

pub fn bench_buffering_constrained_by_memory_limit(
    total_buffered_memory_bytes: u64,
    buffer_memory_limit_bytes: u64,
    next_video_frame_memory_bytes: u64,
) -> bool {
    buffering_constrained_by_memory_limit(
        total_buffered_memory_bytes,
        buffer_memory_limit_bytes,
        next_video_frame_memory_bytes,
    )
}

pub fn bench_total_buffered_memory_bytes(
    pending_video_packet_bytes: u64,
    ready_video_frame_bytes: u64,
    audio_buffered_bytes: u64,
) -> u64 {
    total_buffered_memory_bytes(
        pending_video_packet_bytes,
        ready_video_frame_bytes,
        audio_buffered_bytes,
    )
}

pub fn bench_video_buffer_target_satisfied(
    buffered_ms: u64,
    target_ms: u64,
    remaining_ms: Option<u64>,
    frame_cap_reached: bool,
) -> bool {
    video_buffer_target_satisfied(
        Duration::from_millis(buffered_ms),
        Duration::from_millis(target_ms),
        remaining_ms.map(Duration::from_millis),
        frame_cap_reached,
    )
}

pub fn bench_should_buffer_video(
    buffered_ms: u64,
    threshold_ms: u64,
    remaining_ms: Option<u64>,
) -> bool {
    should_buffer_video(
        Duration::from_millis(buffered_ms),
        Duration::from_millis(threshold_ms),
        remaining_ms.map(Duration::from_millis),
    )
}

pub fn bench_should_buffer_for_rebuffer(
    audio_starving: bool,
    video_starving: bool,
    buffering_constrained_by_memory_limit_flag: bool,
) -> bool {
    should_buffer_for_rebuffer(
        audio_starving,
        video_starving,
        buffering_constrained_by_memory_limit_flag,
    )
}

pub fn bench_pts_to_duration(timestamp: Option<i64>, num: i32, den: i32) -> Option<Duration> {
    pts_to_duration(timestamp, ffmpeg::Rational(num, den))
}

pub fn bench_distribute_video_compressed_bytes(frame_count: usize, compressed_bytes: u64) -> u64 {
    let texture = Arc::new(TextureFrame::new(1, 1, vec![0u8; 4]));
    let mut frames: Vec<QueuedVideoFrame> = (0..frame_count)
        .map(|index| QueuedVideoFrame {
            generation: 1,
            position: Duration::from_millis(index as u64 * 33),
            end_position: Duration::from_millis(index as u64 * 33 + 33),
            texture: texture.clone(),
            decoded_bytes: 4,
            compressed_bytes: 0,
        })
        .collect();
    distribute_video_compressed_bytes(&mut frames, compressed_bytes);
    frames.iter().map(|frame| frame.compressed_bytes).sum()
}
