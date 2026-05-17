use super::*;

fn log_video_debug(arguments: std::fmt::Arguments<'_>) {
    if crate::video::backend::ffmpeg::video_debug_enabled() {
        crate::log::Log::with_tag("tgui-video").debug(arguments);
    }
}

pub(super) fn buffering_profile_for_source(source: &VideoSource) -> BufferingProfile {
    match source {
        VideoSource::File(_) => LOCAL_BUFFERING_PROFILE,
        VideoSource::Url { .. } => NETWORK_BUFFERING_PROFILE,
    }
}

pub(super) fn stream_frame_duration(stream: &format::stream::Stream<'_>) -> Option<Duration> {
    rational_frame_duration(stream.avg_frame_rate())
        .or_else(|| rational_frame_duration(stream.rate()))
}

pub(super) fn rational_frame_duration(rate: ffmpeg::Rational) -> Option<Duration> {
    let numerator = rate.numerator();
    let denominator = rate.denominator();
    if numerator <= 0 || denominator <= 0 {
        return None;
    }

    Some(Duration::from_secs_f64(
        denominator as f64 / numerator as f64,
    ))
}

pub(super) fn validate_video_source(source: &VideoSource) -> Result<(), TguiError> {
    let VideoSource::Url { headers, .. } = source else {
        return Ok(());
    };

    validate_ffmpeg_headers("video", headers)
}

#[cfg(test)]
pub(super) fn http_input_options(
    source: &VideoSource,
) -> Result<ffmpeg::Dictionary<'static>, TguiError> {
    match source {
        VideoSource::File(_) => Ok(ffmpeg::Dictionary::new()),
        VideoSource::Url { headers, .. } => ffmpeg_http_input_options("video", headers),
    }
}

pub(super) fn open_input(
    source: &VideoSource,
    source_url: &str,
) -> Result<format::context::Input, TguiError> {
    match source {
        VideoSource::File(_) => open_ffmpeg_input("video", source_url, None),
        VideoSource::Url { headers, .. } => open_ffmpeg_input("video", source_url, Some(headers)),
    }
}

pub(super) fn open_video_decoder(
    stream: &format::stream::Stream<'_>,
) -> Result<OpenedVideoDecoder, TguiError> {
    let parameters = stream.parameters();
    let codec_id = parameters.id();

    if codec_id == codec::Id::AV1 {
        for decoder_name in ["libdav1d", "libaom-av1", "av1"] {
            let Some(codec) = codec::decoder::find_by_name(decoder_name) else {
                continue;
            };
            if !codec.is_video() || codec.id() != codec_id {
                continue;
            }

            match codec::context::Context::from_parameters(parameters.clone())
                .and_then(|context| context.decoder().open_as(codec))
                .and_then(|opened| opened.video())
            {
                Ok(decoder) => {
                    log_video_debug(format_args!(
                        "selected AV1 decoder name={} description={}",
                        codec.name(),
                        codec.description()
                    ));
                    return Ok(OpenedVideoDecoder {
                        decoder,
                        codec_id,
                        decoder_name: codec.name().to_string(),
                    });
                }
                Err(error) => {
                    log_video_debug(format_args!(
                        "failed to open AV1 decoder name={} error={}",
                        codec.name(),
                        error
                    ));
                }
            }
        }
    }

    let video_context = codec::context::Context::from_parameters(parameters)
        .map_err(|error| TguiError::Media(format!("failed to open video codec: {error}")))?;
    let video_decoder = video_context
        .decoder()
        .video()
        .map_err(|error| TguiError::Media(format!("failed to create video decoder: {error}")))?;

    if let Some(codec) = video_decoder.codec() {
        log_video_debug(format_args!(
            "selected video decoder name={} description={}",
            codec.name(),
            codec.description()
        ));
        return Ok(OpenedVideoDecoder {
            decoder: video_decoder,
            codec_id,
            decoder_name: codec.name().to_string(),
        });
    }

    Ok(OpenedVideoDecoder {
        decoder: video_decoder,
        codec_id,
        decoder_name: codec_id.name().to_string(),
    })
}

pub(super) fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    pending_compressed_bytes: &mut u64,
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
    queue_audio_chunks(audio_output, chunks, pending_compressed_bytes);
    Ok(())
}

pub(super) fn flush_audio_resampler(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    pending_compressed_bytes: &mut u64,
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
    queue_audio_chunks(audio_output, chunks, pending_compressed_bytes);
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
    unsafe {
        frame.alloc(
            resampler.output().format,
            samples,
            resampler.output().channel_layout,
        );
    }
    frame
}

fn queue_audio_chunks(
    audio_output: &AudioOutput,
    chunks: Vec<Vec<f32>>,
    pending_compressed_bytes: &mut u64,
) {
    if chunks.is_empty() {
        return;
    }

    let total_compressed_bytes = std::mem::take(pending_compressed_bytes);
    let total_samples = chunks
        .iter()
        .map(|samples| samples.len() as u64)
        .sum::<u64>()
        .max(1);
    let mut remaining_bytes = total_compressed_bytes;
    let mut remaining_samples = total_samples;

    for samples in chunks {
        let sample_count = samples.len() as u64;
        let chunk_bytes = if remaining_samples == sample_count {
            remaining_bytes
        } else {
            total_compressed_bytes.saturating_mul(sample_count) / total_samples
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

    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        Some(slice.to_vec())
    }
}

pub(super) fn video_frame_to_texture(
    scaler: &mut Scaler,
    decoded: &VideoFrame,
) -> Result<TextureFrame, TguiError> {
    let mut rgba_frame = VideoFrame::empty();
    scaler
        .run(decoded, &mut rgba_frame)
        .map_err(|error| TguiError::Media(format!("failed to convert video frame: {error}")))?;

    let width = rgba_frame.width();
    let height = rgba_frame.height();
    let stride = rgba_frame.stride(0);
    let data = rgba_frame.data(0);
    let row_len = width as usize * 4;
    let mut pixels = vec![0u8; row_len * height as usize];
    for row in 0..height as usize {
        let src_offset = row * stride;
        let dst_offset = row * row_len;
        pixels[dst_offset..dst_offset + row_len]
            .copy_from_slice(&data[src_offset..src_offset + row_len]);
    }

    Ok(TextureFrame::new(width, height, pixels))
}

pub(super) fn pts_to_duration(
    timestamp: Option<i64>,
    time_base: ffmpeg::Rational,
) -> Option<Duration> {
    let timestamp = timestamp?;
    let numerator = time_base.numerator() as f64;
    let denominator = time_base.denominator() as f64;
    if denominator <= 0.0 {
        return None;
    }
    let seconds = timestamp as f64 * numerator / denominator;
    Some(Duration::from_secs_f64(seconds.max(0.0)))
}

pub(super) fn packet_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

pub(super) fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

pub(super) fn should_throttle_demux(
    compressed_buffer_limit_reached: bool,
    audio_hard_full: bool,
    decoded_video_hard_full: bool,
    video_packet_fuse_tripped: bool,
) -> bool {
    compressed_buffer_limit_reached
        || audio_hard_full
        || decoded_video_hard_full
        || video_packet_fuse_tripped
}

pub(super) fn total_buffered_memory_bytes(
    pending_video_packet_bytes: u64,
    ready_video_frame_bytes: u64,
    audio_buffered_bytes: u64,
) -> u64 {
    pending_video_packet_bytes
        .saturating_add(ready_video_frame_bytes)
        .saturating_add(audio_buffered_bytes)
}

pub(super) fn startup_playback_blocked_by_memory_limit(
    buffering_constrained_by_memory_limit: bool,
    has_ready_video_frames: bool,
    has_audio_output: bool,
    audio_buffered_duration: Duration,
) -> bool {
    buffering_constrained_by_memory_limit
        && has_ready_video_frames
        && (!has_audio_output || !audio_buffered_duration.is_zero())
}

pub(super) fn should_buffer_for_rebuffer(
    audio_starving: bool,
    video_starving: bool,
    buffering_constrained_by_memory_limit: bool,
) -> bool {
    audio_starving || (video_starving && !buffering_constrained_by_memory_limit)
}

pub(super) fn buffering_constrained_by_memory_limit(
    total_buffered_memory_bytes: u64,
    buffer_memory_limit_bytes: u64,
    next_video_frame_memory_bytes: u64,
) -> bool {
    total_buffered_memory_bytes.saturating_add(next_video_frame_memory_bytes)
        > buffer_memory_limit_bytes
}

pub(super) fn distribute_video_compressed_bytes(
    frames: &mut [QueuedVideoFrame],
    compressed_bytes: u64,
) {
    if frames.is_empty() {
        return;
    }
    let base = compressed_bytes / frames.len() as u64;
    let remainder = compressed_bytes % frames.len() as u64;
    for (index, frame) in frames.iter_mut().enumerate() {
        frame.compressed_bytes = base + u64::from(index < remainder as usize);
    }
}

pub(super) fn video_buffer_target_satisfied(
    buffered: Duration,
    target: Duration,
    remaining: Option<Duration>,
    frame_cap_reached: bool,
) -> bool {
    buffered >= target
        || frame_cap_reached
        || remaining
            .map(|remaining| buffered.saturating_add(VIDEO_PRESENT_TOLERANCE) >= remaining)
            .unwrap_or(false)
}

pub(super) fn should_buffer_video(
    buffered: Duration,
    threshold: Duration,
    remaining: Option<Duration>,
) -> bool {
    buffered < threshold
        && !remaining
            .map(|remaining| buffered.saturating_add(VIDEO_PRESENT_TOLERANCE) >= remaining)
            .unwrap_or(false)
}
