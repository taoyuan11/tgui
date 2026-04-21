use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig};

use crate::TguiError;

#[derive(Clone)]
pub(super) struct SharedAudioClock {
    shared: Arc<SharedAudioOutput>,
    channels: u16,
    sample_rate: u32,
}

impl SharedAudioClock {
    pub(super) fn position(&self) -> Duration {
        let played_frames = self.shared.played_frames.load(Ordering::SeqCst);
        Duration::from_secs_f64(played_frames as f64 / self.sample_rate as f64)
    }

    pub(super) fn buffered_duration(&self) -> Duration {
        let buffered_samples = self
            .shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .len();
        let buffered_frames = buffered_samples / self.channels as usize;
        Duration::from_secs_f64(buffered_frames as f64 / self.sample_rate as f64)
    }

    pub(super) fn buffered_memory_bytes(&self) -> u64 {
        self.shared
            .compressed_chunks
            .lock()
            .expect("audio compressed queue lock poisoned")
            .iter()
            .map(|chunk| chunk.compressed_bytes)
            .sum()
    }

    pub(super) fn has_started_clock(&self) -> bool {
        self.shared.played_frames.load(Ordering::SeqCst) > 0
    }
}

pub(super) struct AudioOutput {
    shared: Arc<SharedAudioOutput>,
    _stream: Stream,
    channels: u16,
    sample_rate: u32,
}

struct SharedAudioOutput {
    queue: Mutex<VecDeque<f32>>,
    compressed_chunks: Mutex<VecDeque<CompressedAudioChunk>>,
    playing: AtomicBool,
    muted: AtomicBool,
    volume_bits: AtomicU32,
    played_frames: AtomicU64,
    channels: u16,
    underflowing: AtomicBool,
}

struct CompressedAudioChunk {
    sample_count: usize,
    compressed_bytes: u64,
}

impl AudioOutput {
    pub(super) fn new(volume: f32, muted: bool) -> Result<Self, TguiError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| TguiError::Media("audio output device not found".to_string()))?;
        let config = device.default_output_config().map_err(|error| {
            TguiError::Media(format!("failed to query audio output config: {error}"))
        })?;

        let shared = Arc::new(SharedAudioOutput {
            queue: Mutex::new(VecDeque::new()),
            compressed_chunks: Mutex::new(VecDeque::new()),
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(muted),
            volume_bits: AtomicU32::new(volume.to_bits()),
            played_frames: AtomicU64::new(0),
            channels: config.channels(),
            underflowing: AtomicBool::new(false),
        });

        let stream = build_output_stream(&device, &config, shared.clone())?;
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

    pub(super) fn clock_handle(&self) -> SharedAudioClock {
        SharedAudioClock {
            shared: self.shared.clone(),
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }

    pub(super) fn channels(&self) -> u16 {
        self.channels
    }

    pub(super) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(super) fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::SeqCst);
        if !playing {
            self.shared.underflowing.store(false, Ordering::SeqCst);
        }
    }

    pub(super) fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    pub(super) fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::SeqCst);
    }

    pub(super) fn push_samples(&self, samples: &[f32], compressed_bytes: u64) {
        let mut queue = self.shared.queue.lock().expect("audio queue lock poisoned");
        queue.extend(samples.iter().copied());
        drop(queue);

        if !samples.is_empty() {
            self.shared
                .compressed_chunks
                .lock()
                .expect("audio compressed queue lock poisoned")
                .push_back(CompressedAudioChunk {
                    sample_count: samples.len(),
                    compressed_bytes,
                });
        }

        self.shared.underflowing.store(false, Ordering::SeqCst);
    }

    pub(super) fn buffered_duration(&self) -> Duration {
        self.clock_handle().buffered_duration()
    }

    pub(super) fn buffered_memory_bytes(&self) -> u64 {
        self.clock_handle().buffered_memory_bytes()
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    shared: Arc<SharedAudioOutput>,
) -> Result<Stream, TguiError> {
    let error_callback = |error| eprintln!("tgui video audio stream error: {error}");
    let stream_config = config.config();

    match config.sample_format() {
        SampleFormat::I16 => {
            let shared = shared.clone();
            device
                .build_output_stream(
                    &stream_config,
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
                    &stream_config,
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
                &stream_config,
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

fn write_audio_samples<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    let playing = shared.playing.load(Ordering::SeqCst);
    let muted = shared.muted.load(Ordering::SeqCst);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::SeqCst));
    let mut queue = shared.queue.lock().expect("audio queue lock poisoned");
    let mut consumed_samples = 0usize;

    for sample in buffer.iter_mut() {
        let next = if playing {
            match queue.pop_front() {
                Some(sample) => {
                    consumed_samples += 1;
                    sample
                }
                None => {
                    shared.underflowing.store(true, Ordering::SeqCst);
                    0.0
                }
            }
        } else {
            0.0
        };
        let next = if muted { 0.0 } else { next * volume };
        *sample = T::from_sample(next);
    }

    drop(queue);
    consume_audio_compressed_bytes(shared, consumed_samples);

    if playing && consumed_samples > 0 {
        let consumed_frames = (consumed_samples / shared.channels as usize) as u64;
        shared
            .played_frames
            .fetch_add(consumed_frames, Ordering::SeqCst);
        if consumed_samples == buffer.len() {
            shared.underflowing.store(false, Ordering::SeqCst);
        }
    }
}

fn consume_audio_compressed_bytes(shared: &Arc<SharedAudioOutput>, consumed_samples: usize) {
    if consumed_samples == 0 {
        return;
    }

    let mut remaining_samples = consumed_samples;
    let mut chunks = shared
        .compressed_chunks
        .lock()
        .expect("audio compressed queue lock poisoned");

    while remaining_samples > 0 {
        let Some(front) = chunks.front_mut() else {
            break;
        };

        if front.sample_count <= remaining_samples {
            remaining_samples -= front.sample_count;
            chunks.pop_front();
            continue;
        }

        let bytes_to_consume =
            ((front.compressed_bytes as u128 * remaining_samples as u128) / front.sample_count as u128)
                as u64;
        front.sample_count -= remaining_samples;
        front.compressed_bytes = front.compressed_bytes.saturating_sub(bytes_to_consume);
        remaining_samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_audio_still_advances_clock() {
        let shared = Arc::new(SharedAudioOutput {
            queue: Mutex::new(VecDeque::from(vec![0.25, -0.25, 0.5, -0.5])),
            compressed_chunks: Mutex::new(VecDeque::new()),
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
}
