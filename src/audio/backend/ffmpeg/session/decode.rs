use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::util::frame::audio::Audio as AudioFrame;
use ffmpeg_next as ffmpeg;

use crate::audio::backend::shared::AudioOutput;
use crate::foundation::error::TguiError;

pub(super) fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    compressed_bytes: u64,
) -> Result<(), TguiError> {
    let mut decoded = AudioFrame::empty();
    let mut chunks = Vec::new();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut resampled = allocate_resampled_audio_frame(resampler, &decoded);
        resampler.run(&decoded, &mut resampled).map_err(|error| {
            TguiError::Media(format!("failed to resample audio frame: {error}"))
        })?;
        if let Some(samples) = audio_frame_to_f32_if_any(&resampled) {
            chunks.push(samples);
        }
    }
    queue_audio_chunks(audio_output, chunks, compressed_bytes);
    Ok(())
}

pub(super) fn flush_audio_resampler(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
) -> Result<(), TguiError> {
    let mut chunks = Vec::new();
    loop {
        let mut resampled = allocate_flush_audio_frame(resampler);
        match resampler
            .flush(&mut resampled)
            .map_err(|error| TguiError::Media(format!("failed to flush resampler: {error}")))?
        {
            Some(_) => {
                if let Some(samples) = audio_frame_to_f32_if_any(&resampled) {
                    chunks.push(samples);
                }
            }
            None => break,
        }
    }
    queue_audio_chunks(audio_output, chunks, 0);
    Ok(())
}

fn allocate_resampled_audio_frame(resampler: &Resampler, decoded: &AudioFrame) -> AudioFrame {
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
    let mut frame = AudioFrame::empty();
    // SAFETY: `AudioFrame::alloc` 是 ffmpeg-next 暴露在 `Audio` 上的 FFI 包装，
    // 它在内部调用 `av_frame_get_buffer`。`frame` 由 `AudioFrame::empty()` 创建，
    // 是合法的零初始化 frame；`format` / `samples` / `channel_layout` 都来自
    // resampler 自己声明的输出参数，组合保证 ffmpeg 能成功分配。
    unsafe {
        frame.alloc(
            resampler.output().format,
            samples,
            resampler.output().channel_layout,
        );
    }
    frame
}

fn allocate_flush_audio_frame(resampler: &Resampler) -> AudioFrame {
    let samples = resampler
        .delay()
        .map(|delay| delay.output.max(0) as usize)
        .unwrap_or(0)
        .saturating_add(32)
        .max(1);
    let mut frame = AudioFrame::empty();
    // SAFETY: 同 `allocate_resampled_audio_frame`：参数都来自 resampler 自身的
    // 输出描述，`frame` 是新创建的空 frame。
    unsafe {
        frame.alloc(
            resampler.output().format,
            samples,
            resampler.output().channel_layout,
        );
    }
    frame
}

fn queue_audio_chunks(audio_output: &AudioOutput, chunks: Vec<Vec<f32>>, compressed_bytes: u64) {
    if chunks.is_empty() {
        return;
    }

    let total_samples = chunks
        .iter()
        .map(|samples| samples.len() as u64)
        .sum::<u64>()
        .max(1);
    let mut remaining_bytes = compressed_bytes;
    let mut remaining_samples = total_samples;

    for samples in chunks {
        let sample_count = samples.len() as u64;
        let chunk_bytes = if remaining_samples == sample_count {
            remaining_bytes
        } else {
            compressed_bytes.saturating_mul(sample_count) / total_samples
        };
        remaining_bytes = remaining_bytes.saturating_sub(chunk_bytes);
        remaining_samples = remaining_samples.saturating_sub(sample_count);
        audio_output.push_samples(samples, chunk_bytes);
    }
}

fn audio_frame_to_f32_if_any(frame: &AudioFrame) -> Option<Vec<f32>> {
    if frame.samples() == 0 || !frame.is_packed() {
        return None;
    }

    // SAFETY: 进入这里之前已确认 frame 是 packed（即所有声道交错存放在
    // `data[0]` 中），`samples * channels` 即采样总数，且 ffmpeg-next 在
    // packed FLT 输出下保证 `data[0]` 指向 `len * sizeof(f32)` 的连续缓冲。
    // 切片只在调用期间存在，立即拷贝出 `Vec`，不会越过 frame 的生命周期。
    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        Some(slice.to_vec())
    }
}
