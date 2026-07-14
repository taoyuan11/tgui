//! Benchmark hooks for the ffmpeg video backend.
//!
//! Only compiled when the `bench-support` feature is enabled. Re-exported up
//! the module chain into `tgui::video::bench_support` so external Criterion
//! benches can drive the queue / decision functions directly without spinning
//! up a full decoder.

use std::sync::Arc;
use std::time::Duration;

use ffmpeg_next as ffmpeg;

use crate::media::{RasterRequest, TextureFrame};
use crate::video::backend::VideoRenderFrame;

use super::helpers::{
    buffering_constrained_by_memory_limit, distribute_video_compressed_bytes, pts_to_duration,
    should_buffer_for_rebuffer, should_buffer_video, should_throttle_demux,
    total_buffered_memory_bytes, video_buffer_target_satisfied, VideoFrameConverter,
};
use super::queue::{QueuedVideoFrame, SharedPlaybackClock, SharedVideoQueue};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::video::Video as VideoFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchVideoFrameKind {
    Rgba,
    Rgb24,
    Nv12,
    Yuv420p,
}

pub struct BenchConvertedVideoFrame {
    pub is_yuv: bool,
    pub width: u32,
    pub height: u32,
    pub decoded_bytes: u64,
    pub revision: u64,
}

pub struct BenchVideoFrameConverter {
    converter: VideoFrameConverter,
    decoded: VideoFrame,
    target_raster: Option<RasterRequest>,
    texture_id: u64,
    revision: u64,
}

impl BenchVideoFrameConverter {
    pub fn new(
        kind: BenchVideoFrameKind,
        width: u32,
        height: u32,
        target_size: Option<(u32, u32)>,
    ) -> Self {
        Self {
            converter: VideoFrameConverter::new(),
            decoded: bench_video_frame(kind, width, height),
            target_raster: target_size
                .map(|(width, height)| RasterRequest::new_clamped(width, height)),
            texture_id: TextureFrame::allocate_id(),
            revision: 0,
        }
    }

    pub fn convert(&mut self) -> BenchConvertedVideoFrame {
        self.revision = self.revision.saturating_add(1);
        let converted = self
            .converter
            .convert_render_frame(
                &self.decoded,
                self.target_raster,
                self.texture_id,
                self.revision,
            )
            .expect("benchmark video frame should convert");

        match converted {
            VideoRenderFrame::Rgba(texture) => BenchConvertedVideoFrame {
                is_yuv: false,
                width: texture.size().0,
                height: texture.size().1,
                decoded_bytes: texture.pixels().len() as u64,
                revision: texture.revision(),
            },
            VideoRenderFrame::Yuv(frame) => BenchConvertedVideoFrame {
                is_yuv: true,
                width: frame.size().0,
                height: frame.size().1,
                decoded_bytes: frame.decoded_bytes(),
                revision: frame.revision(),
            },
        }
    }
}

fn bench_video_frame(kind: BenchVideoFrameKind, width: u32, height: u32) -> VideoFrame {
    let pixel = match kind {
        BenchVideoFrameKind::Rgba => Pixel::RGBA,
        BenchVideoFrameKind::Rgb24 => Pixel::RGB24,
        BenchVideoFrameKind::Nv12 => Pixel::NV12,
        BenchVideoFrameKind::Yuv420p => Pixel::YUV420P,
    };
    let mut frame = VideoFrame::new(pixel, width.max(1), height.max(1));

    match kind {
        BenchVideoFrameKind::Rgba => fill_packed_video_plane(&mut frame, 4),
        BenchVideoFrameKind::Rgb24 => fill_packed_video_plane(&mut frame, 3),
        BenchVideoFrameKind::Nv12 => {
            fill_video_plane(&mut frame, 0, height.max(1), 0x30);
            fill_video_plane(&mut frame, 1, height.max(1).div_ceil(2), 0x80);
        }
        BenchVideoFrameKind::Yuv420p => {
            let chroma_height = height.max(1).div_ceil(2);
            fill_video_plane(&mut frame, 0, height.max(1), 0x30);
            fill_video_plane(&mut frame, 1, chroma_height, 0x70);
            fill_video_plane(&mut frame, 2, chroma_height, 0x90);
        }
    }

    frame
}

fn fill_packed_video_plane(frame: &mut VideoFrame, bytes_per_pixel: usize) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let row_len = width.saturating_mul(bytes_per_pixel);
    let stride = frame.stride(0);
    let data = frame.data_mut(0);

    for y in 0..height {
        let row_start = y.saturating_mul(stride);
        for x in 0..row_len {
            data[row_start + x] = ((x + y * 31) % 251) as u8;
        }
        for byte in &mut data[row_start + row_len..row_start + stride] {
            *byte = 0xEE;
        }
    }
}

fn fill_video_plane(frame: &mut VideoFrame, plane: usize, rows: u32, seed: u8) {
    let stride = frame.stride(plane);
    let data = frame.data_mut(plane);

    for y in 0..rows as usize {
        let row_start = y.saturating_mul(stride);
        let row_end = row_start.saturating_add(stride);
        for (offset, byte) in data[row_start..row_end].iter_mut().enumerate() {
            *byte = seed.wrapping_add(((offset + y) % 29) as u8);
        }
    }
}

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
        compressed_bytes_per_frame: u64,
    ) {
        let texture = Arc::new(TextureFrame::new(1, 1, vec![0u8; 4]));
        let frames: Vec<QueuedVideoFrame> = positions_ms
            .iter()
            .map(|&ms| QueuedVideoFrame {
                generation,
                position: Duration::from_millis(ms),
                end_position: Duration::from_millis(ms + 33),
                frame: VideoRenderFrame::rgba(texture.clone()),
                compressed_bytes: compressed_bytes_per_frame,
                decoded_bytes: texture.pixels().len() as u64,
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
            frame: VideoRenderFrame::rgba(texture.clone()),
            compressed_bytes: 0,
            decoded_bytes: texture.pixels().len() as u64,
        })
        .collect();
    distribute_video_compressed_bytes(&mut frames, compressed_bytes);
    frames.iter().map(|frame| frame.compressed_bytes).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_video_frame_converter_reports_direct_yuv_frames() {
        let mut converter = BenchVideoFrameConverter::new(BenchVideoFrameKind::Nv12, 16, 8, None);

        let converted = converter.convert();

        assert!(converted.is_yuv);
        assert_eq!((converted.width, converted.height), (16, 8));
        assert!(converted.decoded_bytes > 0);
        assert_eq!(converted.revision, 1);
    }

    #[test]
    fn bench_video_frame_converter_reports_downscaled_rgba_frames() {
        let mut converter =
            BenchVideoFrameConverter::new(BenchVideoFrameKind::Nv12, 16, 8, Some((8, 4)));

        let converted = converter.convert();

        assert!(!converted.is_yuv);
        assert_eq!((converted.width, converted.height), (8, 4));
        assert_eq!(converted.decoded_bytes, 8 * 4 * 4);
        assert_eq!(converted.revision, 1);
    }

    #[test]
    fn bench_video_frame_converter_sequence_keeps_direct_yuv_path() {
        let mut converter = BenchVideoFrameConverter::new(BenchVideoFrameKind::Nv12, 16, 8, None);

        for revision in 1..=8 {
            let converted = converter.convert();

            assert!(converted.is_yuv);
            assert_eq!((converted.width, converted.height), (16, 8));
            assert_eq!(converted.revision, revision);
        }
    }

    #[test]
    fn bench_video_frame_converter_sequence_keeps_downscaled_rgba_path() {
        let mut converter =
            BenchVideoFrameConverter::new(BenchVideoFrameKind::Nv12, 16, 8, Some((8, 4)));

        for revision in 1..=8 {
            let converted = converter.convert();

            assert!(!converted.is_yuv);
            assert_eq!((converted.width, converted.height), (8, 4));
            assert_eq!(converted.decoded_bytes, 8 * 4 * 4);
            assert_eq!(converted.revision, revision);
        }
    }
}
