use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig};
use parking_lot::Mutex;

use crate::foundation::error::TguiError;
use crate::log::Log;

const AUDIO_QUEUE_CHUNK_TARGET_SAMPLES: usize = 2048;
const MIN_PLAYBACK_RATE: f32 = 0.25;
const MAX_PLAYBACK_RATE: f32 = 4.0;

#[cfg(feature = "bench-support")]
static AUDIO_OUTPUT_CALLBACKS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static AUDIO_OUTPUT_LOCK_MISSES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static AUDIO_OUTPUT_UNDERFLOWS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static AUDIO_OUTPUT_WRITTEN_SAMPLES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioOutputDiagnostics {
    pub callbacks: u64,
    pub lock_misses: u64,
    pub underflows: u64,
    pub written_samples: u64,
}

pub(crate) fn normalize_playback_rate(rate: f32) -> f32 {
    if rate.is_finite() {
        rate.clamp(MIN_PLAYBACK_RATE, MAX_PLAYBACK_RATE)
    } else {
        1.0
    }
}

#[cfg(feature = "bench-support")]
fn record_audio_output_callback() {
    AUDIO_OUTPUT_CALLBACKS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_audio_output_callback() {}

#[cfg(feature = "bench-support")]
fn record_audio_output_lock_miss() {
    AUDIO_OUTPUT_LOCK_MISSES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_audio_output_lock_miss() {}

#[cfg(feature = "bench-support")]
fn record_audio_output_underflow() {
    AUDIO_OUTPUT_UNDERFLOWS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_audio_output_underflow() {}

#[cfg(feature = "bench-support")]
fn record_audio_output_written_samples(samples: usize) {
    AUDIO_OUTPUT_WRITTEN_SAMPLES.fetch_add(samples as u64, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_audio_output_written_samples(_samples: usize) {}

#[cfg(feature = "bench-support")]
fn reset_audio_output_diagnostics() {
    AUDIO_OUTPUT_CALLBACKS.store(0, Ordering::Relaxed);
    AUDIO_OUTPUT_LOCK_MISSES.store(0, Ordering::Relaxed);
    AUDIO_OUTPUT_UNDERFLOWS.store(0, Ordering::Relaxed);
    AUDIO_OUTPUT_WRITTEN_SAMPLES.store(0, Ordering::Relaxed);
}

#[cfg(feature = "bench-support")]
fn audio_output_diagnostics() -> AudioOutputDiagnostics {
    AudioOutputDiagnostics {
        callbacks: AUDIO_OUTPUT_CALLBACKS.load(Ordering::Relaxed),
        lock_misses: AUDIO_OUTPUT_LOCK_MISSES.load(Ordering::Relaxed),
        underflows: AUDIO_OUTPUT_UNDERFLOWS.load(Ordering::Relaxed),
        written_samples: AUDIO_OUTPUT_WRITTEN_SAMPLES.load(Ordering::Relaxed),
    }
}

#[derive(Clone)]
pub(crate) struct SharedAudioClock {
    shared: Arc<SharedAudioOutput>,
    channels: u16,
    sample_rate: u32,
}

impl SharedAudioClock {
    pub(crate) fn position(&self) -> Duration {
        let played_frames = self.shared.played_frames.load(Ordering::Relaxed);
        Duration::from_secs_f64(played_frames as f64 / self.sample_rate as f64)
    }

    pub(crate) fn buffered_duration(&self) -> Duration {
        let buffered_samples = self.shared.queued_samples.load(Ordering::Relaxed) as usize;
        let buffered_frames = buffered_samples / self.channels as usize;
        Duration::from_secs_f64(buffered_frames as f64 / self.sample_rate as f64)
    }

    pub(crate) fn buffered_memory_bytes(&self) -> u64 {
        self.shared.queued_decoded_bytes.load(Ordering::Relaxed)
    }

    #[cfg(feature = "video")]
    pub(crate) fn has_started_clock(&self) -> bool {
        self.shared.played_frames.load(Ordering::Relaxed) > 0
    }
}

pub(crate) struct AudioOutput {
    shared: Arc<SharedAudioOutput>,
    _stream: Stream,
    channels: u16,
    sample_rate: u32,
}

pub(crate) struct AudioSampleBatch {
    chunks: Vec<Vec<f32>>,
    pending: Vec<f32>,
    sample_count: u64,
    target_samples: usize,
}

pub(crate) struct SharedAudioOutput {
    pub(super) queue: Mutex<VecDeque<AudioSampleChunk>>,
    pub(super) queued_samples: AtomicU64,
    pub(super) queued_compressed_bytes: AtomicU64,
    pub(super) queued_decoded_bytes: AtomicU64,
    pub(super) playing: AtomicBool,
    pub(super) muted: AtomicBool,
    pub(super) volume_bits: AtomicU32,
    pub(super) playback_rate_bits: AtomicU32,
    pub(super) source_frame_fraction_bits: AtomicU32,
    pub(super) played_frames: AtomicU64,
    pub(super) channels: u16,
    pub(super) underflowing: AtomicBool,
}

pub(crate) struct AudioSampleChunk {
    pub(super) samples: Vec<f32>,
    pub(super) offset: usize,
    pub(super) compressed_bytes: u64,
    pub(super) decoded_bytes: u64,
}

impl AudioSampleBatch {
    pub(crate) fn new() -> Self {
        Self::with_target_samples(AUDIO_QUEUE_CHUNK_TARGET_SAMPLES)
    }

    pub(crate) fn new_for_channels(channels: u16) -> Self {
        Self::with_target_samples(channel_aligned_chunk_target_samples(channels))
    }

    fn with_target_samples(target_samples: usize) -> Self {
        Self {
            chunks: Vec::new(),
            pending: Vec::with_capacity(target_samples),
            sample_count: 0,
            target_samples,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sample_count == 0
    }

    pub(crate) fn extend_from_slice(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        self.sample_count = self.sample_count.saturating_add(samples.len() as u64);
        let mut remaining = samples;
        while !remaining.is_empty() {
            if self.pending.is_empty() && remaining.len() >= self.target_samples {
                self.chunks.push(remaining[..self.target_samples].to_vec());
                remaining = &remaining[self.target_samples..];
                continue;
            }

            let room = self.target_samples.saturating_sub(self.pending.len());
            let take = room.min(remaining.len());
            self.pending.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];

            if self.pending.len() >= self.target_samples {
                self.chunks.push(std::mem::replace(
                    &mut self.pending,
                    Vec::with_capacity(self.target_samples),
                ));
            }
        }
    }

    pub(crate) fn into_chunks(mut self) -> Vec<Vec<f32>> {
        if !self.pending.is_empty() {
            self.chunks.push(self.pending);
        }
        self.chunks
    }
}

impl Default for AudioSampleBatch {
    fn default() -> Self {
        Self::new()
    }
}

fn channel_aligned_chunk_target_samples(channels: u16) -> usize {
    let channels = usize::from(channels.max(1));
    let aligned = AUDIO_QUEUE_CHUNK_TARGET_SAMPLES / channels * channels;
    aligned.max(channels)
}

impl AudioOutput {
    pub(crate) fn new(volume: f32, muted: bool, log_tag: &'static str) -> Result<Self, TguiError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| TguiError::Media("audio output device not found".to_string()))?;
        let config = device.default_output_config().map_err(|error| {
            TguiError::Media(format!("failed to query audio output config: {error}"))
        })?;

        let shared = Arc::new(SharedAudioOutput {
            queue: Mutex::new(VecDeque::new()),
            queued_samples: AtomicU64::new(0),
            queued_compressed_bytes: AtomicU64::new(0),
            queued_decoded_bytes: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(muted),
            volume_bits: AtomicU32::new(volume.to_bits()),
            playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
            source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
            played_frames: AtomicU64::new(0),
            channels: config.channels(),
            underflowing: AtomicBool::new(false),
        });

        let stream = build_output_stream(&device, &config, shared.clone(), log_tag)?;
        stream.play().map_err(|error| {
            TguiError::Media(format!("failed to start audio output stream: {error}"))
        })?;

        Ok(Self {
            shared,
            _stream: stream,
            channels: config.channels(),
            sample_rate: config.sample_rate(),
        })
    }

    pub(crate) fn clock_handle(&self) -> SharedAudioClock {
        SharedAudioClock {
            shared: self.shared.clone(),
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }

    pub(crate) fn channels(&self) -> u16 {
        self.channels
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::Release);
        if !playing {
            self.shared.underflowing.store(false, Ordering::Relaxed);
        }
    }

    pub(crate) fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::Relaxed);
    }

    pub(crate) fn set_playback_rate(&self, rate: f32) {
        self.shared
            .playback_rate_bits
            .store(normalize_playback_rate(rate).to_bits(), Ordering::Relaxed);
        self.shared
            .source_frame_fraction_bits
            .store(0.0f32.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn push_sample_batch(&self, batch: AudioSampleBatch, compressed_bytes: u64) {
        if batch.is_empty() {
            return;
        }
        let chunks =
            build_audio_sample_chunks_from_coalesced(batch.into_chunks(), compressed_bytes);
        self.push_built_sample_chunks(chunks);
    }

    fn push_built_sample_chunks(&self, chunks: Vec<AudioSampleChunk>) {
        if chunks.is_empty() {
            return;
        }
        let sample_len = chunks
            .iter()
            .map(|chunk| chunk.samples.len() as u64)
            .sum::<u64>();
        let compressed_bytes = chunks
            .iter()
            .map(|chunk| chunk.compressed_bytes)
            .sum::<u64>();
        let decoded_bytes = chunks.iter().map(|chunk| chunk.decoded_bytes).sum::<u64>();

        self.shared
            .queued_samples
            .fetch_add(sample_len, Ordering::Relaxed);
        self.shared
            .queued_compressed_bytes
            .fetch_add(compressed_bytes, Ordering::Relaxed);
        self.shared
            .queued_decoded_bytes
            .fetch_add(decoded_bytes, Ordering::Relaxed);

        self.shared.queue.lock().extend(chunks);
        self.shared.underflowing.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(super) fn build_audio_sample_chunks(
    chunks: Vec<Vec<f32>>,
    compressed_bytes: u64,
) -> Vec<AudioSampleChunk> {
    let chunks = coalesce_audio_chunks(chunks);
    build_audio_sample_chunks_from_coalesced(chunks, compressed_bytes)
}

fn build_audio_sample_chunks_from_coalesced(
    chunks: Vec<Vec<f32>>,
    compressed_bytes: u64,
) -> Vec<AudioSampleChunk> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let total_samples = chunks
        .iter()
        .map(|samples| samples.len() as u64)
        .sum::<u64>()
        .max(1);
    let mut remaining_bytes = compressed_bytes;
    let mut remaining_samples = total_samples;

    chunks
        .into_iter()
        .map(|samples| {
            let sample_count = samples.len() as u64;
            let chunk_bytes = if remaining_samples == sample_count {
                remaining_bytes
            } else {
                compressed_bytes.saturating_mul(sample_count) / total_samples
            };
            remaining_bytes = remaining_bytes.saturating_sub(chunk_bytes);
            remaining_samples = remaining_samples.saturating_sub(sample_count);
            let decoded_bytes = decoded_audio_bytes(&samples);
            AudioSampleChunk {
                samples,
                offset: 0,
                compressed_bytes: chunk_bytes,
                decoded_bytes,
            }
        })
        .collect()
}

#[cfg(test)]
fn coalesce_audio_chunks(chunks: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    let mut batch = AudioSampleBatch::new();
    for samples in chunks {
        batch.extend_from_slice(&samples);
    }
    batch.into_chunks()
}

impl AudioOutput {
    #[cfg(feature = "video")]
    pub(crate) fn buffered_duration(&self) -> Duration {
        self.clock_handle().buffered_duration()
    }

    #[cfg(feature = "video")]
    pub(crate) fn buffered_memory_bytes(&self) -> u64 {
        self.clock_handle().buffered_memory_bytes()
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    shared: Arc<SharedAudioOutput>,
    log_tag: &'static str,
) -> Result<Stream, TguiError> {
    let error_callback =
        move |error| Log::with_tag(log_tag).error(format_args!("audio stream error: {error}"));
    let stream_config = config.config();

    match config.sample_format() {
        SampleFormat::I16 => {
            let shared = shared.clone();
            device
                .build_output_stream(
                    stream_config.clone(),
                    move |buffer: &mut [i16], _| write_audio_samples(buffer, &shared),
                    error_callback,
                    None,
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to build i16 audio stream: {error}"))
                })
        }
        SampleFormat::U16 => {
            let shared = shared.clone();
            device
                .build_output_stream(
                    stream_config.clone(),
                    move |buffer: &mut [u16], _| write_audio_samples(buffer, &shared),
                    error_callback,
                    None,
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to build u16 audio stream: {error}"))
                })
        }
        SampleFormat::F32 => device
            .build_output_stream(
                stream_config,
                move |buffer: &mut [f32], _| write_audio_samples(buffer, &shared),
                error_callback,
                None,
            )
            .map_err(|error| {
                TguiError::Media(format!("failed to build f32 audio stream: {error}"))
            }),
        other => Err(TguiError::Media(format!(
            "unsupported audio sample format: {other:?}"
        ))),
    }
}

pub(super) fn write_audio_samples<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    record_audio_output_callback();
    let playing = shared.playing.load(Ordering::Acquire);
    if !playing {
        fill_silence(buffer);
        return;
    }

    let playback_rate = normalize_playback_rate(f32::from_bits(
        shared.playback_rate_bits.load(Ordering::Relaxed),
    ));
    if (playback_rate - 1.0).abs() > f32::EPSILON {
        write_audio_samples_at_rate(buffer, shared, playback_rate);
        return;
    }

    shared
        .source_frame_fraction_bits
        .store(0.0f32.to_bits(), Ordering::Relaxed);
    write_audio_samples_at_normal_rate(buffer, shared);
}

fn write_audio_samples_at_normal_rate<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    let muted = shared.muted.load(Ordering::Relaxed);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::Relaxed));
    let Some(mut queue) = shared.queue.try_lock() else {
        record_audio_output_lock_miss();
        fill_audio_underflow(buffer, shared);
        return;
    };
    let mut write_index = 0usize;
    let mut consumed_samples = 0usize;
    let mut consumed_compressed_bytes = 0u64;
    let mut consumed_decoded_bytes = 0u64;

    while write_index < buffer.len() {
        let Some(chunk) = queue.front_mut() else {
            break;
        };
        let remaining_samples = chunk.samples.len() - chunk.offset;
        if remaining_samples == 0 {
            if let Some(chunk) = queue.pop_front() {
                consumed_decoded_bytes = consumed_decoded_bytes.saturating_add(chunk.decoded_bytes);
            }
            continue;
        }

        let write_count = remaining_samples.min(buffer.len() - write_index);
        let start = chunk.offset;
        let end = start + write_count;
        let src = &chunk.samples[start..end];
        let dst = &mut buffer[write_index..write_index + write_count];
        if muted {
            for out in dst.iter_mut() {
                *out = T::from_sample(0.0);
            }
        } else if volume == 1.0 {
            for (out, &sample) in dst.iter_mut().zip(src.iter()) {
                *out = T::from_sample(sample);
            }
        } else {
            for (out, &sample) in dst.iter_mut().zip(src.iter()) {
                *out = T::from_sample(sample * volume);
            }
        }

        let bytes = if write_count == remaining_samples {
            chunk.compressed_bytes
        } else {
            ((chunk.compressed_bytes as u128 * write_count as u128) / remaining_samples as u128)
                as u64
        };
        chunk.compressed_bytes = chunk.compressed_bytes.saturating_sub(bytes);
        chunk.offset = end;
        consumed_compressed_bytes = consumed_compressed_bytes.saturating_add(bytes);

        write_index += write_count;
        consumed_samples += write_count;

        if chunk.offset >= chunk.samples.len() {
            if let Some(chunk) = queue.pop_front() {
                consumed_decoded_bytes = consumed_decoded_bytes.saturating_add(chunk.decoded_bytes);
            }
        }
    }

    drop(queue);

    fill_silence(&mut buffer[write_index..]);
    record_audio_output_written_samples(write_index);

    if consumed_samples > 0 {
        shared
            .queued_samples
            .fetch_sub(consumed_samples as u64, Ordering::Relaxed);
        let consumed_frames = (consumed_samples / shared.channels as usize) as u64;
        shared
            .played_frames
            .fetch_add(consumed_frames, Ordering::Relaxed);
    }
    if consumed_compressed_bytes > 0 {
        shared
            .queued_compressed_bytes
            .fetch_sub(consumed_compressed_bytes, Ordering::Relaxed);
    }
    if consumed_decoded_bytes > 0 {
        shared
            .queued_decoded_bytes
            .fetch_sub(consumed_decoded_bytes, Ordering::Relaxed);
    }

    if write_index < buffer.len() {
        record_audio_output_underflow();
        shared.underflowing.store(true, Ordering::Relaxed);
    } else if consumed_samples > 0 {
        shared.underflowing.store(false, Ordering::Relaxed);
    }
}

fn write_audio_samples_at_rate<T>(
    buffer: &mut [T],
    shared: &Arc<SharedAudioOutput>,
    playback_rate: f32,
) where
    T: Sample + FromSample<f32>,
{
    let channels = shared.channels.max(1) as usize;
    let muted = shared.muted.load(Ordering::Relaxed);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::Relaxed));
    let Some(mut queue) = shared.queue.try_lock() else {
        record_audio_output_lock_miss();
        fill_audio_underflow(buffer, shared);
        return;
    };
    let mut write_index = 0usize;
    let mut consumed = ConsumedAudio::default();
    let mut frame_fraction =
        f32::from_bits(shared.source_frame_fraction_bits.load(Ordering::Relaxed))
            .clamp(0.0, 0.999_999);

    while write_index + channels <= buffer.len() {
        let copied = copy_front_audio_frame(
            &mut queue,
            channels,
            &mut buffer[write_index..write_index + channels],
            muted,
            volume,
            &mut consumed,
        );
        if !copied {
            break;
        }

        write_index += channels;
        frame_fraction += playback_rate;
        let frames_to_consume = frame_fraction.floor() as usize;
        frame_fraction -= frames_to_consume as f32;
        if frames_to_consume > 0 {
            let source_consumed =
                consume_audio_source_frames(&mut queue, channels, frames_to_consume);
            let consumed_frames = source_consumed.frames;
            consumed.add(source_consumed);
            if consumed_frames < frames_to_consume {
                frame_fraction = 0.0;
                break;
            }
        }
    }

    drop(queue);

    fill_silence(&mut buffer[write_index..]);
    record_audio_output_written_samples(write_index);

    shared
        .source_frame_fraction_bits
        .store(frame_fraction.to_bits(), Ordering::Relaxed);
    if consumed.samples > 0 {
        shared
            .queued_samples
            .fetch_sub(consumed.samples as u64, Ordering::Relaxed);
    }
    if consumed.frames > 0 {
        shared
            .played_frames
            .fetch_add(consumed.frames as u64, Ordering::Relaxed);
    }
    if consumed.compressed_bytes > 0 {
        shared
            .queued_compressed_bytes
            .fetch_sub(consumed.compressed_bytes, Ordering::Relaxed);
    }
    if consumed.decoded_bytes > 0 {
        shared
            .queued_decoded_bytes
            .fetch_sub(consumed.decoded_bytes, Ordering::Relaxed);
    }

    if write_index < buffer.len() {
        record_audio_output_underflow();
        shared.underflowing.store(true, Ordering::Relaxed);
    } else if consumed.samples > 0 {
        shared.underflowing.store(false, Ordering::Relaxed);
    }
}

fn fill_audio_underflow<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    fill_silence(buffer);
    record_audio_output_underflow();
    shared.underflowing.store(true, Ordering::Relaxed);
}

fn fill_silence<T>(buffer: &mut [T])
where
    T: Sample + FromSample<f32>,
{
    for sample in buffer.iter_mut() {
        *sample = T::from_sample(0.0);
    }
}

fn copy_front_audio_frame<T>(
    queue: &mut VecDeque<AudioSampleChunk>,
    channels: usize,
    output: &mut [T],
    muted: bool,
    volume: f32,
    consumed: &mut ConsumedAudio,
) -> bool
where
    T: Sample + FromSample<f32>,
{
    loop {
        let Some(chunk) = queue.front_mut() else {
            return false;
        };
        if chunk.offset + channels <= chunk.samples.len() {
            let src = &chunk.samples[chunk.offset..chunk.offset + channels];
            if muted {
                for out in output.iter_mut() {
                    *out = T::from_sample(0.0);
                }
            } else if volume == 1.0 {
                for (out, &sample) in output.iter_mut().zip(src.iter()) {
                    *out = T::from_sample(sample);
                }
            } else {
                for (out, &sample) in output.iter_mut().zip(src.iter()) {
                    *out = T::from_sample(sample * volume);
                }
            }
            return true;
        }

        drain_front_audio_chunk(queue, consumed);
    }
}

#[derive(Default)]
struct ConsumedAudio {
    samples: usize,
    frames: usize,
    compressed_bytes: u64,
    decoded_bytes: u64,
}

impl ConsumedAudio {
    fn add(&mut self, other: Self) {
        self.samples = self.samples.saturating_add(other.samples);
        self.frames = self.frames.saturating_add(other.frames);
        self.compressed_bytes = self.compressed_bytes.saturating_add(other.compressed_bytes);
        self.decoded_bytes = self.decoded_bytes.saturating_add(other.decoded_bytes);
    }
}

fn consume_audio_source_frames(
    queue: &mut VecDeque<AudioSampleChunk>,
    channels: usize,
    frames: usize,
) -> ConsumedAudio {
    let mut remaining_frames = frames;
    let mut consumed = ConsumedAudio::default();

    while remaining_frames > 0 {
        let Some(chunk) = queue.front_mut() else {
            break;
        };
        let remaining_samples = chunk.samples.len().saturating_sub(chunk.offset);
        let remaining_chunk_frames = remaining_samples / channels;
        if remaining_chunk_frames == 0 {
            drain_front_audio_chunk(queue, &mut consumed);
            continue;
        }

        let take_frames = remaining_frames.min(remaining_chunk_frames);
        let take_samples = take_frames * channels;
        let bytes = if take_samples >= remaining_samples {
            chunk.compressed_bytes
        } else {
            ((chunk.compressed_bytes as u128 * take_samples as u128) / remaining_samples as u128)
                as u64
        };

        chunk.compressed_bytes = chunk.compressed_bytes.saturating_sub(bytes);
        chunk.offset = chunk.offset.saturating_add(take_samples);
        consumed.samples = consumed.samples.saturating_add(take_samples);
        consumed.frames = consumed.frames.saturating_add(take_frames);
        consumed.compressed_bytes = consumed.compressed_bytes.saturating_add(bytes);
        remaining_frames -= take_frames;

        if chunk.offset >= chunk.samples.len() {
            drain_front_audio_chunk(queue, &mut consumed);
        }
    }

    consumed
}

fn drain_front_audio_chunk(queue: &mut VecDeque<AudioSampleChunk>, consumed: &mut ConsumedAudio) {
    let Some(chunk) = queue.pop_front() else {
        return;
    };

    consumed.samples = consumed
        .samples
        .saturating_add(chunk.samples.len().saturating_sub(chunk.offset));
    consumed.compressed_bytes = consumed
        .compressed_bytes
        .saturating_add(chunk.compressed_bytes);
    consumed.decoded_bytes = consumed.decoded_bytes.saturating_add(chunk.decoded_bytes);
}

fn decoded_audio_bytes(samples: &Vec<f32>) -> u64 {
    samples
        .capacity()
        .saturating_mul(size_of::<f32>())
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(feature = "bench-support")]
pub(crate) mod bench_support {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::sync::Arc;

    use parking_lot::Mutex;

    pub use super::AudioOutputDiagnostics;
    use super::{
        audio_output_diagnostics, reset_audio_output_diagnostics, write_audio_samples,
        AudioSampleChunk, SharedAudioOutput,
    };

    /// Opaque wrapper around `SharedAudioOutput` for external benches.
    pub struct BenchAudioOutput {
        pub(super) inner: Arc<SharedAudioOutput>,
    }

    pub fn make_output(channels: u16, volume: f32, muted: bool, playing: bool) -> BenchAudioOutput {
        BenchAudioOutput {
            inner: Arc::new(SharedAudioOutput {
                queue: Mutex::new(VecDeque::new()),
                queued_samples: AtomicU64::new(0),
                queued_compressed_bytes: AtomicU64::new(0),
                queued_decoded_bytes: AtomicU64::new(0),
                playing: AtomicBool::new(playing),
                muted: AtomicBool::new(muted),
                volume_bits: AtomicU32::new(volume.to_bits()),
                playback_rate_bits: AtomicU32::new(1.0f32.to_bits()),
                source_frame_fraction_bits: AtomicU32::new(0.0f32.to_bits()),
                played_frames: AtomicU64::new(0),
                channels,
                underflowing: AtomicBool::new(false),
            }),
        }
    }

    pub fn enqueue_chunk(output: &BenchAudioOutput, samples: Vec<f32>, compressed_bytes: u64) {
        if samples.is_empty() {
            return;
        }
        let sample_len = samples.len() as u64;
        let decoded_bytes = super::decoded_audio_bytes(&samples);
        output
            .inner
            .queued_samples
            .fetch_add(sample_len, std::sync::atomic::Ordering::Relaxed);
        output
            .inner
            .queued_compressed_bytes
            .fetch_add(compressed_bytes, std::sync::atomic::Ordering::Relaxed);
        output
            .inner
            .queued_decoded_bytes
            .fetch_add(decoded_bytes, std::sync::atomic::Ordering::Relaxed);
        output.inner.queue.lock().push_back(AudioSampleChunk {
            samples,
            offset: 0,
            compressed_bytes,
            decoded_bytes,
        });
    }

    pub fn write_f32(buffer: &mut [f32], output: &BenchAudioOutput) {
        write_audio_samples(buffer, &output.inner)
    }

    pub fn write_i16(buffer: &mut [i16], output: &BenchAudioOutput) {
        write_audio_samples(buffer, &output.inner)
    }

    pub fn played_frames(output: &BenchAudioOutput) -> u64 {
        output.inner.played_frames.load(Ordering::Relaxed)
    }

    pub fn queued_samples(output: &BenchAudioOutput) -> u64 {
        output.inner.queued_samples.load(Ordering::Relaxed)
    }

    pub fn underflowing(output: &BenchAudioOutput) -> bool {
        output.inner.underflowing.load(Ordering::Relaxed)
    }

    pub fn set_playback_rate(output: &BenchAudioOutput, rate: f32) {
        output.inner.playback_rate_bits.store(
            super::normalize_playback_rate(rate).to_bits(),
            Ordering::Relaxed,
        );
        output
            .inner
            .source_frame_fraction_bits
            .store(0.0f32.to_bits(), Ordering::Relaxed);
    }

    pub fn reset_diagnostics() {
        reset_audio_output_diagnostics();
    }

    pub fn diagnostics() -> AudioOutputDiagnostics {
        audio_output_diagnostics()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn bench_output_playback_rate_updates_rate_and_resets_fraction() {
            let output = make_output(2, 1.0, false, true);
            output
                .inner
                .source_frame_fraction_bits
                .store(0.5f32.to_bits(), Ordering::Relaxed);

            set_playback_rate(&output, 8.0);

            assert_eq!(
                f32::from_bits(output.inner.playback_rate_bits.load(Ordering::Relaxed)),
                4.0
            );
            assert_eq!(
                f32::from_bits(
                    output
                        .inner
                        .source_frame_fraction_bits
                        .load(Ordering::Relaxed)
                ),
                0.0
            );
        }

        #[test]
        fn bench_output_diagnostics_count_callbacks_and_written_samples() {
            reset_diagnostics();
            let output = make_output(2, 1.0, false, true);
            enqueue_chunk(&output, vec![0.25, -0.25, 0.5, -0.5], 4);

            let mut buffer = [0.0f32; 4];
            write_f32(&mut buffer, &output);

            assert_eq!(buffer, [0.25, -0.25, 0.5, -0.5]);
            assert_eq!(
                diagnostics(),
                AudioOutputDiagnostics {
                    callbacks: 1,
                    lock_misses: 0,
                    underflows: 0,
                    written_samples: 4,
                }
            );
        }

        #[test]
        fn bench_output_diagnostics_count_lock_miss_underflow() {
            reset_diagnostics();
            let output = make_output(2, 1.0, false, true);
            enqueue_chunk(&output, vec![0.25, -0.25, 0.5, -0.5], 4);
            let guard = output.inner.queue.lock();

            let mut buffer = [1.0f32; 4];
            write_f32(&mut buffer, &output);
            drop(guard);

            assert_eq!(buffer, [0.0; 4]);
            assert_eq!(
                diagnostics(),
                AudioOutputDiagnostics {
                    callbacks: 1,
                    lock_misses: 1,
                    underflows: 1,
                    written_samples: 0,
                }
            );
        }

        #[test]
        fn bench_output_sustained_callbacks_stay_buffered_without_lock_misses() {
            reset_diagnostics();
            let output = make_output(2, 1.0, false, true);
            let callback_samples = 512 * 2;
            let make_samples = |samples: usize| {
                (0..samples)
                    .map(|index| ((index % 97) as f32 / 96.0) * 2.0 - 1.0)
                    .collect::<Vec<_>>()
            };
            enqueue_chunk(&output, make_samples(callback_samples * 4), 16 * 1024);

            let mut buffer = vec![0.0_f32; callback_samples];
            for _ in 0..96 {
                if queued_samples(&output) < (callback_samples * 2) as u64 {
                    enqueue_chunk(&output, make_samples(callback_samples * 2), 8 * 1024);
                }
                write_f32(&mut buffer, &output);
            }

            assert_eq!(
                diagnostics(),
                AudioOutputDiagnostics {
                    callbacks: 96,
                    lock_misses: 0,
                    underflows: 0,
                    written_samples: (callback_samples * 96) as u64,
                }
            );
        }
    }
}
