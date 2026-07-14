use super::*;

fn log_video_debug(arguments: std::fmt::Arguments<'_>) {
    if crate::video::backend::ffmpeg::video_debug_enabled() {
        crate::log::Log::with_tag("tgui-video").debug(arguments);
    }
}

pub(super) fn buffering_profile_for_source(source: &VideoSource) -> BufferingProfile {
    match source {
        VideoSource::File(_) | VideoSource::Bytes { .. } => LOCAL_BUFFERING_PROFILE,
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
    match source {
        VideoSource::File(_) => Ok(()),
        VideoSource::Url { headers, .. } => validate_ffmpeg_headers("video", headers),
        VideoSource::Bytes { bytes, .. } => {
            if bytes.is_empty() {
                Err(TguiError::Media("video bytes source is empty".to_string()))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
pub(super) fn http_input_options(
    source: &VideoSource,
) -> Result<ffmpeg::Dictionary<'static>, TguiError> {
    match source {
        VideoSource::File(_) | VideoSource::Bytes { .. } => Ok(ffmpeg::Dictionary::new()),
        VideoSource::Url { headers, .. } => ffmpeg_http_input_options("video", headers),
    }
}

pub(super) struct OpenedVideoInput {
    pub(super) input: format::context::Input,
    pub(super) resource: Option<TemporaryMediaFile>,
}

pub(super) fn open_input(source: &VideoSource) -> Result<OpenedVideoInput, TguiError> {
    let (source_url, headers, resource) = match source {
        VideoSource::File(path) => (media_path_to_url("video", path)?, None, None),
        VideoSource::Url { url, headers } => (url.clone(), Some(headers.as_slice()), None),
        VideoSource::Bytes { bytes, extension } => {
            let file = create_temporary_media_file("video", bytes, extension.as_deref())?;
            let source_url = media_path_to_url("video", file.path())?;
            (source_url, None, Some(file))
        }
    };
    let input = open_ffmpeg_input("video", &source_url, headers)?;
    Ok(OpenedVideoInput { input, resource })
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

pub(super) struct VideoFrameConverter {
    rgba_frame: VideoFrame,
    pixel_scratch: Vec<u8>,
    scaler: Option<Scaler>,
    scaler_config: Option<VideoScalerConfig>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VideoScalerConfig {
    input_format: Pixel,
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
}

impl VideoFrameConverter {
    pub(super) fn new() -> Self {
        Self {
            rgba_frame: VideoFrame::empty(),
            pixel_scratch: Vec::new(),
            scaler: None,
            scaler_config: None,
        }
    }

    pub(super) fn convert(
        &mut self,
        decoded: &VideoFrame,
        target_raster: Option<RasterRequest>,
        texture_id: u64,
        revision: u64,
    ) -> Result<TextureFrame, TguiError> {
        let output = video_scaler_output(decoded, target_raster);
        if decoded_frame_matches_rgba_output(decoded, output) {
            self.scaler = None;
            self.scaler_config = None;
            self.rgba_frame = VideoFrame::empty();

            let width = decoded.width();
            let height = decoded.height();
            let stride = decoded.stride(0);
            let data = decoded.data(0);
            let pixels =
                copy_rgba_frame_pixels_arc(data, width, height, stride, &mut self.pixel_scratch);
            return Ok(TextureFrame::with_id_revision_and_pixels(
                texture_id, revision, width, height, pixels,
            ));
        }
        if let Some(conversion) = direct_packed_rgba_conversion(decoded, output) {
            self.scaler = None;
            self.scaler_config = None;
            self.rgba_frame = VideoFrame::empty();

            let width = decoded.width();
            let height = decoded.height();
            let stride = decoded.stride(0);
            let data = decoded.data(0);
            let pixels = copy_packed_frame_pixels_as_rgba(
                data,
                width,
                height,
                stride,
                conversion,
                &mut self.pixel_scratch,
            );
            return Ok(TextureFrame::with_id_revision_and_pixels(
                texture_id, revision, width, height, pixels,
            ));
        }

        let config = VideoScalerConfig::new(decoded, output);
        self.ensure_scaler(config)?;
        self.prepare_output_frame(config);
        let scaler = self
            .scaler
            .as_mut()
            .expect("video scaler should exist after ensure_scaler");
        scaler
            .run(decoded, &mut self.rgba_frame)
            .map_err(|error| TguiError::Media(format!("failed to convert video frame: {error}")))?;

        let width = self.rgba_frame.width();
        let height = self.rgba_frame.height();
        let stride = self.rgba_frame.stride(0);
        let data = self.rgba_frame.data(0);
        let pixels =
            copy_rgba_frame_pixels_arc(data, width, height, stride, &mut self.pixel_scratch);

        Ok(TextureFrame::with_id_revision_and_pixels(
            texture_id, revision, width, height, pixels,
        ))
    }

    pub(super) fn convert_render_frame(
        &mut self,
        decoded: &VideoFrame,
        target_raster: Option<RasterRequest>,
        texture_id: u64,
        revision: u64,
    ) -> Result<VideoRenderFrame, TguiError> {
        if let Some(format) = direct_yuv_render_format(decoded, target_raster) {
            self.scaler = None;
            self.scaler_config = None;
            self.rgba_frame = VideoFrame::empty();
            let yuv = direct_yuv_frame(decoded, texture_id, revision, format)?;
            return Ok(VideoRenderFrame::yuv(Arc::new(yuv)));
        }

        let texture = self.convert(decoded, target_raster, texture_id, revision)?;
        Ok(VideoRenderFrame::rgba(Arc::new(texture)))
    }

    fn ensure_scaler(&mut self, config: VideoScalerConfig) -> Result<(), TguiError> {
        if self.scaler_config == Some(config) && self.scaler.is_some() {
            return Ok(());
        }

        self.scaler = Some(create_video_scaler(config)?);
        self.scaler_config = Some(config);
        self.rgba_frame = VideoFrame::empty();
        Ok(())
    }

    fn prepare_output_frame(&mut self, config: VideoScalerConfig) {
        let matches_output = self.rgba_frame.format() == Pixel::RGBA
            && self.rgba_frame.width() == config.output_width
            && self.rgba_frame.height() == config.output_height;
        if !matches_output {
            self.rgba_frame = VideoFrame::empty();
        }
    }

    #[cfg(test)]
    pub(super) fn output_data_ptr(&self) -> Option<usize> {
        (self.rgba_frame.planes() > 0).then(|| self.rgba_frame.data(0).as_ptr() as usize)
    }

    #[cfg(test)]
    pub(super) fn has_scaler(&self) -> bool {
        self.scaler.is_some()
    }
}

impl VideoScalerConfig {
    fn new(decoded: &VideoFrame, output: RasterRequest) -> Self {
        Self {
            input_format: decoded.format(),
            input_width: decoded.width(),
            input_height: decoded.height(),
            output_width: output.width(),
            output_height: output.height(),
        }
    }
}

fn video_scaler_output(
    decoded: &VideoFrame,
    target_raster: Option<RasterRequest>,
) -> RasterRequest {
    target_raster.unwrap_or_else(|| {
        RasterRequest::new_clamped(decoded.width().max(1), decoded.height().max(1))
    })
}

fn decoded_frame_matches_rgba_output(decoded: &VideoFrame, output: RasterRequest) -> bool {
    decoded.format() == Pixel::RGBA
        && decoded.width() == output.width()
        && decoded.height() == output.height()
}

fn direct_packed_rgba_conversion(
    decoded: &VideoFrame,
    output: RasterRequest,
) -> Option<PackedRgbaConversion> {
    if decoded.width() != output.width() || decoded.height() != output.height() {
        return None;
    }
    PackedRgbaConversion::from_pixel(decoded.format())
}

fn direct_yuv_render_format(
    decoded: &VideoFrame,
    target_raster: Option<RasterRequest>,
) -> Option<VideoYuvFormat> {
    if target_raster.is_some_and(|target| target_downscales_decoded_frame(decoded, target)) {
        return None;
    }

    match decoded.format() {
        Pixel::NV12 => Some(VideoYuvFormat::Nv12),
        Pixel::YUV420P | Pixel::YUVJ420P => Some(VideoYuvFormat::Yuv420p),
        _ => None,
    }
}

fn target_downscales_decoded_frame(decoded: &VideoFrame, target: RasterRequest) -> bool {
    target.width() < decoded.width() || target.height() < decoded.height()
}

pub(super) fn direct_yuv_frame(
    decoded: &VideoFrame,
    texture_id: u64,
    revision: u64,
    format: VideoYuvFormat,
) -> Result<VideoYuvFrame, TguiError> {
    let width = decoded.width();
    let height = decoded.height();
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let color_space = decoded_yuv_color_space(decoded);
    let planes = match format {
        VideoYuvFormat::Nv12 => vec![
            copy_yuv_plane(decoded, 0, VideoYuvPlaneFormat::R8, width, height)?,
            copy_yuv_plane(
                decoded,
                1,
                VideoYuvPlaneFormat::Rg8,
                chroma_width,
                chroma_height,
            )?,
        ],
        VideoYuvFormat::Yuv420p => vec![
            copy_yuv_plane(decoded, 0, VideoYuvPlaneFormat::R8, width, height)?,
            copy_yuv_plane(
                decoded,
                1,
                VideoYuvPlaneFormat::R8,
                chroma_width,
                chroma_height,
            )?,
            copy_yuv_plane(
                decoded,
                2,
                VideoYuvPlaneFormat::R8,
                chroma_width,
                chroma_height,
            )?,
        ],
    };
    VideoYuvFrame::with_id_revision_and_planes(
        texture_id,
        revision,
        width,
        height,
        format,
        color_space,
        Arc::from(planes),
    )
}

fn copy_yuv_plane(
    decoded: &VideoFrame,
    index: usize,
    format: VideoYuvPlaneFormat,
    width: u32,
    height: u32,
) -> Result<VideoYuvPlane, TguiError> {
    if index >= decoded.planes() {
        return Err(TguiError::Media(format!(
            "invalid YUV plane: decoded frame has {} planes but plane {index} was requested",
            decoded.planes()
        )));
    }
    let stride = decoded.stride(index);
    let byte_len = stride
        .checked_mul(height as usize)
        .ok_or_else(|| TguiError::Media("invalid YUV plane: byte length overflow".into()))?;
    let data = decoded.data(index);
    if data.len() < byte_len {
        return Err(TguiError::Media(format!(
            "invalid YUV plane: plane {index} needs {byte_len} bytes but decoded frame has {}",
            data.len()
        )));
    }
    let bytes_per_row = u32::try_from(stride)
        .map_err(|_| TguiError::Media("invalid YUV plane: stride too large".into()))?;
    VideoYuvPlane::new(
        format,
        width,
        height,
        bytes_per_row,
        Arc::from(&data[..byte_len]),
    )
}

fn decoded_yuv_color_space(decoded: &VideoFrame) -> VideoYuvColorSpace {
    let matrix = match decoded.color_space() {
        ffmpeg::util::color::Space::BT2020NCL | ffmpeg::util::color::Space::BT2020CL => {
            VideoYuvColorMatrix::Bt2020
        }
        ffmpeg::util::color::Space::BT470BG
        | ffmpeg::util::color::Space::FCC
        | ffmpeg::util::color::Space::SMPTE170M
        | ffmpeg::util::color::Space::SMPTE240M => VideoYuvColorMatrix::Bt601,
        _ => VideoYuvColorMatrix::Bt709,
    };
    let range = if decoded.format() == Pixel::YUVJ420P {
        VideoYuvColorRange::Full
    } else {
        match decoded.color_range() {
            ffmpeg::util::color::Range::JPEG => VideoYuvColorRange::Full,
            ffmpeg::util::color::Range::MPEG | ffmpeg::util::color::Range::Unspecified => {
                VideoYuvColorRange::Limited
            }
        }
    };
    VideoYuvColorSpace { matrix, range }
}

fn create_video_scaler(config: VideoScalerConfig) -> Result<Scaler, TguiError> {
    Scaler::get(
        config.input_format,
        config.input_width,
        config.input_height,
        Pixel::RGBA,
        config.output_width,
        config.output_height,
        ScalingFlags::BILINEAR,
    )
    .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))
}

#[cfg(test)]
pub(super) fn copy_rgba_frame_pixels(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
) -> Vec<u8> {
    let mut scratch = Vec::new();
    copy_rgba_frame_pixels_into(data, width, height, stride, &mut scratch);
    scratch
}

pub(super) fn copy_rgba_frame_pixels_arc(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    scratch: &mut Vec<u8>,
) -> Arc<[u8]> {
    let row_len = width as usize * 4;
    let height = height as usize;
    let pixel_len = row_len * height;
    if stride == row_len {
        return Arc::from(&data[..pixel_len]);
    }

    copy_rgba_frame_pixels_into(data, width, height as u32, stride, scratch);
    Arc::from(scratch.as_slice())
}

#[cfg(test)]
pub(super) fn copy_bgra_frame_pixels(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
) -> Vec<u8> {
    let mut scratch = Vec::new();
    copy_packed_frame_pixels_into(
        data,
        width,
        height,
        stride,
        PackedRgbaConversion::Bgra,
        &mut scratch,
    );
    scratch
}

#[cfg(test)]
pub(super) fn copy_rgb24_frame_pixels(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
) -> Vec<u8> {
    let mut scratch = Vec::new();
    copy_packed_frame_pixels_into(
        data,
        width,
        height,
        stride,
        PackedRgbaConversion::Rgb24,
        &mut scratch,
    );
    scratch
}

#[cfg(test)]
pub(super) fn copy_bgr24_frame_pixels(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
) -> Vec<u8> {
    let mut scratch = Vec::new();
    copy_packed_frame_pixels_into(
        data,
        width,
        height,
        stride,
        PackedRgbaConversion::Bgr24,
        &mut scratch,
    );
    scratch
}

fn copy_packed_frame_pixels_as_rgba(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    conversion: PackedRgbaConversion,
    scratch: &mut Vec<u8>,
) -> Arc<[u8]> {
    copy_packed_frame_pixels_into(data, width, height, stride, conversion, scratch);
    Arc::from(scratch.as_slice())
}

fn copy_rgba_frame_pixels_into(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    scratch: &mut Vec<u8>,
) {
    let row_len = width as usize * 4;
    let height = height as usize;
    let pixel_len = row_len * height;
    if stride == row_len {
        scratch.clear();
        scratch.extend_from_slice(&data[..pixel_len]);
        return;
    }

    scratch.resize(pixel_len, 0);
    for row in 0..height {
        let src_offset = row * stride;
        let dst_offset = row * row_len;
        scratch[dst_offset..dst_offset + row_len]
            .copy_from_slice(&data[src_offset..src_offset + row_len]);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackedRgbaConversion {
    Bgra,
    Rgb24,
    Bgr24,
}

impl PackedRgbaConversion {
    fn from_pixel(pixel: Pixel) -> Option<Self> {
        match pixel {
            Pixel::BGRA => Some(Self::Bgra),
            Pixel::RGB24 => Some(Self::Rgb24),
            Pixel::BGR24 => Some(Self::Bgr24),
            _ => None,
        }
    }

    fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra => 4,
            Self::Rgb24 | Self::Bgr24 => 3,
        }
    }

    fn write_rgba(self, src: &[u8], dst: &mut [u8]) {
        match self {
            Self::Bgra => {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = src[3];
            }
            Self::Rgb24 => {
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
                dst[3] = 255;
            }
            Self::Bgr24 => {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = 255;
            }
        }
    }
}

fn copy_packed_frame_pixels_into(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    conversion: PackedRgbaConversion,
    scratch: &mut Vec<u8>,
) {
    let source_row_len = width as usize * conversion.bytes_per_pixel();
    let target_row_len = width as usize * 4;
    let height = height as usize;
    let pixel_len = target_row_len * height;
    scratch.resize(pixel_len, 0);
    for row in 0..height {
        let src_offset = row * stride;
        let dst_offset = row * target_row_len;
        let src = &data[src_offset..src_offset + source_row_len];
        let dst = &mut scratch[dst_offset..dst_offset + target_row_len];
        for (src, dst) in src
            .chunks_exact(conversion.bytes_per_pixel())
            .zip(dst.chunks_exact_mut(4))
        {
            conversion.write_rgba(src, dst);
        }
    }
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
