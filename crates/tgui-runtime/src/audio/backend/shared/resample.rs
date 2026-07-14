use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::util::frame::audio::Audio as AudioFrame;
use ffmpeg_next as ffmpeg;

use crate::foundation::error::TguiError;

use super::{AudioOutput, AudioSampleBatch};

pub(crate) fn receive_audio_frames_with_buffer(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    reusable: &mut ReusableAudioFrame,
    compressed_bytes: u64,
) -> Result<(), TguiError> {
    let mut pending_compressed_bytes = compressed_bytes;
    receive_audio_frames_with_pending_and_buffer(
        decoder,
        resampler,
        audio_output,
        reusable,
        &mut pending_compressed_bytes,
    )
}

pub(crate) fn flush_audio_resampler_with_buffer(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    reusable: &mut ReusableAudioFrame,
) -> Result<(), TguiError> {
    let mut pending_compressed_bytes = 0;
    flush_audio_resampler_with_pending_and_buffer(
        resampler,
        audio_output,
        reusable,
        &mut pending_compressed_bytes,
    )
}

pub(crate) fn receive_audio_frames_with_pending_and_buffer(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    reusable: &mut ReusableAudioFrame,
    pending_compressed_bytes: &mut u64,
) -> Result<(), TguiError> {
    let mut decoded = AudioFrame::empty();
    let mut batch = AudioSampleBatch::new_for_channels(audio_output.channels());
    while decoder.receive_frame(&mut decoded).is_ok() {
        let resampled = reusable.prepare_resampled_frame(resampler, &decoded);
        resampler.run(&decoded, resampled).map_err(|error| {
            TguiError::Media(format!("failed to resample audio frame: {error}"))
        })?;
        if let Some(samples) = audio_frame_f32_samples(resampled) {
            batch.extend_from_slice(samples);
        }
    }
    queue_audio_batch(audio_output, batch, pending_compressed_bytes);
    Ok(())
}

pub(crate) fn flush_audio_resampler_with_pending_and_buffer(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    reusable: &mut ReusableAudioFrame,
    pending_compressed_bytes: &mut u64,
) -> Result<(), TguiError> {
    let mut batch = AudioSampleBatch::new_for_channels(audio_output.channels());
    loop {
        let resampled = reusable.prepare_flush_frame(resampler);
        match resampler
            .flush(resampled)
            .map_err(|error| TguiError::Media(format!("failed to flush resampler: {error}")))?
        {
            Some(_) => {
                if let Some(samples) = audio_frame_f32_samples(resampled) {
                    batch.extend_from_slice(samples);
                }
            }
            None => break,
        }
    }
    queue_audio_batch(audio_output, batch, pending_compressed_bytes);
    Ok(())
}

pub(crate) struct ReusableAudioFrame {
    frame: AudioFrame,
    allocated_samples: usize,
}

impl ReusableAudioFrame {
    pub(crate) fn new() -> Self {
        Self {
            frame: AudioFrame::empty(),
            allocated_samples: 0,
        }
    }

    fn prepare_resampled_frame(
        &mut self,
        resampler: &Resampler,
        decoded: &AudioFrame,
    ) -> &mut AudioFrame {
        let samples = resampled_sample_capacity(resampler, decoded);
        self.prepare_frame(samples, resampler);
        &mut self.frame
    }

    fn prepare_flush_frame(&mut self, resampler: &Resampler) -> &mut AudioFrame {
        let samples = flush_sample_capacity(resampler);
        self.prepare_frame(samples, resampler);
        &mut self.frame
    }

    fn prepare_frame(&mut self, samples: usize, resampler: &Resampler) {
        let output = resampler.output();
        let allocation_matches = !self.frame_is_empty()
            && self.frame.format() == output.format
            && self.frame.channel_layout() == output.channel_layout
            && self.allocated_samples >= samples;

        if allocation_matches {
            self.frame.set_samples(samples);
            self.frame.set_channel_layout(output.channel_layout);
            self.frame.set_format(output.format);
            return;
        }

        self.frame = AudioFrame::empty();
        // SAFETY: AudioFrame::alloc wraps av_frame_get_buffer. The empty frame is
        // initialized with the resampler output format and channel layout.
        unsafe {
            self.frame
                .alloc(output.format, samples, output.channel_layout);
        }
        self.allocated_samples = samples;
    }

    fn frame_is_empty(&self) -> bool {
        // SAFETY: This only checks whether the underlying data pointer is null.
        unsafe { self.frame.is_empty() }
    }
}

fn resampled_sample_capacity(resampler: &Resampler, decoded: &AudioFrame) -> usize {
    let delay = resampler
        .delay()
        .map(|delay| delay.output.max(0) as usize)
        .unwrap_or(0);
    let input_rate = decoded.rate().max(1) as u64;
    let output_rate = resampler.output().rate.max(1) as u64;
    let scaled_samples =
        ((decoded.samples() as u64 * output_rate) + input_rate.saturating_sub(1)) / input_rate;
    let samples = delay
        .saturating_add(scaled_samples as usize)
        .saturating_add(32)
        .max(1);
    samples
}

fn flush_sample_capacity(resampler: &Resampler) -> usize {
    resampler
        .delay()
        .map(|delay| delay.output.max(0) as usize)
        .unwrap_or(0)
        .saturating_add(32)
        .max(1)
}

fn queue_audio_batch(
    audio_output: &AudioOutput,
    batch: AudioSampleBatch,
    pending_compressed_bytes: &mut u64,
) {
    if batch.is_empty() {
        return;
    }

    let total_compressed_bytes = std::mem::take(pending_compressed_bytes);
    audio_output.push_sample_batch(batch, total_compressed_bytes);
}

fn audio_frame_f32_samples(frame: &AudioFrame) -> Option<&[f32]> {
    if frame.samples() == 0 || !frame.is_packed() {
        return None;
    }

    // SAFETY: packed F32 output stores all interleaved samples in data[0].
    // The returned slice is consumed before the frame is reused or dropped.
    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        Some(std::slice::from_raw_parts(
            (*frame.as_ptr()).data[0] as *const f32,
            len,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::audio::backend::shared::ensure_ffmpeg_initialized;

    use super::*;

    fn test_resampler() -> Resampler {
        ensure_ffmpeg_initialized().expect("FFmpeg should initialize for resampler tests");
        Resampler::get(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::STEREO,
            48_000,
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::STEREO,
            48_000,
        )
        .expect("test resampler should initialize")
    }

    fn decoded_frame(samples: usize) -> AudioFrame {
        let mut frame = AudioFrame::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            samples,
            ffmpeg::ChannelLayout::STEREO,
        );
        frame.set_rate(48_000);
        frame
    }

    fn audio_data_ptr(frame: &AudioFrame) -> usize {
        // SAFETY: The test only reads the frame data pointer after allocation.
        unsafe { (*frame.as_ptr()).data[0] as usize }
    }

    #[test]
    fn reusable_audio_frame_reuses_matching_allocation() {
        let resampler = test_resampler();
        let mut reusable = ReusableAudioFrame::new();
        let decoded = decoded_frame(128);
        let first_allocated = {
            let frame = reusable.prepare_resampled_frame(&resampler, &decoded);
            assert_ne!(audio_data_ptr(frame), 0);
            audio_data_ptr(frame)
        };
        let allocated_samples = reusable.allocated_samples;

        let shorter = decoded_frame(64);
        let second_allocated = {
            let frame = reusable.prepare_resampled_frame(&resampler, &shorter);
            audio_data_ptr(frame)
        };

        assert_eq!(second_allocated, first_allocated);
        assert_eq!(reusable.allocated_samples, allocated_samples);
    }

    #[test]
    fn reusable_audio_frame_expands_when_capacity_is_insufficient() {
        let resampler = test_resampler();
        let mut reusable = ReusableAudioFrame::new();
        let small = decoded_frame(16);
        reusable.prepare_resampled_frame(&resampler, &small);
        let small_capacity = reusable.allocated_samples;

        let large = decoded_frame(2048);
        reusable.prepare_resampled_frame(&resampler, &large);

        assert!(reusable.allocated_samples > small_capacity);
        assert!(reusable.allocated_samples >= resampled_sample_capacity(&resampler, &large));
    }

    #[test]
    fn reusable_audio_frame_reuses_for_flush_frame() {
        let resampler = test_resampler();
        let mut reusable = ReusableAudioFrame::new();
        let decoded = decoded_frame(128);
        let first_allocated = {
            let frame = reusable.prepare_resampled_frame(&resampler, &decoded);
            audio_data_ptr(frame)
        };
        let allocated_samples = reusable.allocated_samples;

        let flush_allocated = {
            let frame = reusable.prepare_flush_frame(&resampler);
            audio_data_ptr(frame)
        };

        assert_eq!(flush_allocated, first_allocated);
        assert_eq!(reusable.allocated_samples, allocated_samples);
        assert_eq!(reusable.frame.samples(), flush_sample_capacity(&resampler));
    }
}
