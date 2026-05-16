use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig};

use crate::foundation::error::TguiError;
use crate::log::Log;

#[derive(Clone)]
pub(crate) struct SharedAudioClock {
    shared: Arc<SharedAudioOutput>,
    channels: u16,
    sample_rate: u32,
}

impl SharedAudioClock {
    pub(crate) fn position(&self) -> Duration {
        let played_frames = self.shared.played_frames.load(Ordering::SeqCst);
        Duration::from_secs_f64(played_frames as f64 / self.sample_rate as f64)
    }

    pub(crate) fn buffered_duration(&self) -> Duration {
        let buffered_samples = self.shared.queued_samples.load(Ordering::SeqCst) as usize;
        let buffered_frames = buffered_samples / self.channels as usize;
        Duration::from_secs_f64(buffered_frames as f64 / self.sample_rate as f64)
    }

    pub(crate) fn buffered_memory_bytes(&self) -> u64 {
        self.shared.queued_compressed_bytes.load(Ordering::SeqCst)
    }

    #[cfg(feature = "video")]
    pub(crate) fn has_started_clock(&self) -> bool {
        self.shared.played_frames.load(Ordering::SeqCst) > 0
    }
}

pub(crate) struct AudioOutput {
    shared: Arc<SharedAudioOutput>,
    _stream: Stream,
    channels: u16,
    sample_rate: u32,
}

pub(super) struct SharedAudioOutput {
    pub(super) queue: Mutex<VecDeque<AudioSampleChunk>>,
    pub(super) queued_samples: AtomicU64,
    pub(super) queued_compressed_bytes: AtomicU64,
    pub(super) playing: AtomicBool,
    pub(super) muted: AtomicBool,
    pub(super) volume_bits: AtomicU32,
    pub(super) played_frames: AtomicU64,
    pub(super) channels: u16,
    pub(super) underflowing: AtomicBool,
}

pub(super) struct AudioSampleChunk {
    pub(super) samples: Vec<f32>,
    pub(super) offset: usize,
    pub(super) compressed_bytes: u64,
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
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(muted),
            volume_bits: AtomicU32::new(volume.to_bits()),
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
        self.shared.playing.store(playing, Ordering::SeqCst);
        if !playing {
            self.shared.underflowing.store(false, Ordering::SeqCst);
        }
    }

    pub(crate) fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::SeqCst);
    }

    pub(crate) fn push_samples(&self, samples: Vec<f32>, compressed_bytes: u64) {
        if samples.is_empty() {
            return;
        }
        let sample_len = samples.len();

        self.shared
            .queued_samples
            .fetch_add(sample_len as u64, Ordering::SeqCst);
        self.shared
            .queued_compressed_bytes
            .fetch_add(compressed_bytes, Ordering::SeqCst);

        self.shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .push_back(AudioSampleChunk {
                samples,
                offset: 0,
                compressed_bytes,
            });

        self.shared.underflowing.store(false, Ordering::SeqCst);
    }

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

pub(super) fn write_audio_samples<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    let playing = shared.playing.load(Ordering::SeqCst);
    if !playing {
        for sample in buffer.iter_mut() {
            *sample = T::from_sample(0.0);
        }
        return;
    }

    let muted = shared.muted.load(Ordering::SeqCst);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::SeqCst));
    let mut queue = shared.queue.lock().expect("audio queue lock poisoned");
    let mut write_index = 0usize;
    let mut consumed_samples = 0usize;
    let mut consumed_compressed_bytes = 0u64;

    while write_index < buffer.len() {
        let wrote = {
            let Some(chunk) = queue.front_mut() else {
                break;
            };
            let remaining_samples = chunk.samples.len().saturating_sub(chunk.offset);
            if remaining_samples == 0 {
                0usize
            } else {
                let write_count = remaining_samples.min(buffer.len() - write_index);
                let start = chunk.offset;
                let end = start + write_count;
                for (out, sample) in buffer[write_index..write_index + write_count]
                    .iter_mut()
                    .zip(chunk.samples[start..end].iter().copied())
                {
                    let next = if muted { 0.0 } else { sample * volume };
                    *out = T::from_sample(next);
                }

                let bytes = if write_count == remaining_samples {
                    chunk.compressed_bytes
                } else {
                    ((chunk.compressed_bytes as u128 * write_count as u128)
                        / remaining_samples as u128) as u64
                };
                chunk.compressed_bytes = chunk.compressed_bytes.saturating_sub(bytes);
                chunk.offset = end;
                consumed_compressed_bytes = consumed_compressed_bytes.saturating_add(bytes);
                write_count
            }
        };

        if queue
            .front()
            .is_some_and(|chunk| chunk.offset >= chunk.samples.len())
        {
            queue.pop_front();
        }
        if wrote == 0 {
            continue;
        }
        write_index += wrote;
        consumed_samples += wrote;
    }

    for sample in buffer[write_index..].iter_mut() {
        *sample = T::from_sample(0.0);
    }
    if write_index < buffer.len() {
        shared.underflowing.store(true, Ordering::SeqCst);
    }

    drop(queue);

    if consumed_samples > 0 {
        shared
            .queued_samples
            .fetch_sub(consumed_samples as u64, Ordering::SeqCst);
    }
    if consumed_compressed_bytes > 0 {
        shared
            .queued_compressed_bytes
            .fetch_sub(consumed_compressed_bytes, Ordering::SeqCst);
    }

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
