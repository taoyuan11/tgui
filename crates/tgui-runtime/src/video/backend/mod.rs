use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::foundation::binding::State;
use crate::foundation::error::TguiError;
use crate::media::{RasterRequest, TextureFrame};

use super::types::{
    VideoAudioTrack, VideoAudioTrackSelection, VideoMetrics, VideoPlaybackState, VideoSize,
    VideoSource, VideoSubtitleBitmapCue, VideoSubtitleCue, VideoSubtitleCuePlacement,
    VideoSubtitleCueStyle, VideoSubtitleTrack, VideoSubtitleTrackSelection, VideoSurfaceSnapshot,
};

pub(crate) mod ffmpeg;

pub(crate) const DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VideoRenderFrame {
    Rgba(Arc<TextureFrame>),
    #[allow(dead_code)]
    Yuv(Arc<VideoYuvFrame>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum VideoYuvFormat {
    Nv12,
    Yuv420p,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum VideoYuvColorMatrix {
    Bt601,
    Bt709,
    Bt2020,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum VideoYuvColorRange {
    Limited,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VideoYuvColorSpace {
    pub(crate) matrix: VideoYuvColorMatrix,
    pub(crate) range: VideoYuvColorRange,
}

impl Default for VideoYuvColorSpace {
    fn default() -> Self {
        Self {
            matrix: VideoYuvColorMatrix::Bt709,
            range: VideoYuvColorRange::Limited,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum VideoYuvPlaneFormat {
    R8,
    Rg8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VideoYuvPlane {
    pub(crate) format: VideoYuvPlaneFormat,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes_per_row: u32,
    pub(crate) bytes: Arc<[u8]>,
}

impl VideoYuvPlane {
    pub(crate) fn new(
        format: VideoYuvPlaneFormat,
        width: u32,
        height: u32,
        bytes_per_row: u32,
        bytes: Arc<[u8]>,
    ) -> Result<Self, TguiError> {
        validate_yuv_plane(format, width, height, bytes_per_row, bytes.len())?;
        Ok(Self {
            format,
            width,
            height,
            bytes_per_row,
            bytes,
        })
    }

    pub(crate) fn decoded_bytes(&self) -> u64 {
        self.bytes.len() as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VideoYuvFrame {
    id: u64,
    revision: u64,
    width: u32,
    height: u32,
    format: VideoYuvFormat,
    color_space: VideoYuvColorSpace,
    planes: Arc<[VideoYuvPlane]>,
}

impl VideoYuvFrame {
    #[allow(dead_code)]
    pub(crate) fn with_id_revision_and_planes(
        id: u64,
        revision: u64,
        width: u32,
        height: u32,
        format: VideoYuvFormat,
        color_space: VideoYuvColorSpace,
        planes: Arc<[VideoYuvPlane]>,
    ) -> Result<Self, TguiError> {
        validate_yuv_frame(width, height, format, &planes)?;
        Ok(Self {
            id,
            revision: revision.max(1),
            width,
            height,
            format,
            color_space,
            planes,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn new(
        width: u32,
        height: u32,
        format: VideoYuvFormat,
        color_space: VideoYuvColorSpace,
        planes: Arc<[VideoYuvPlane]>,
    ) -> Result<Self, TguiError> {
        Self::with_id_revision_and_planes(
            TextureFrame::allocate_id(),
            1,
            width,
            height,
            format,
            color_space,
            planes,
        )
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub(crate) fn format(&self) -> VideoYuvFormat {
        self.format
    }

    pub(crate) fn color_space(&self) -> VideoYuvColorSpace {
        self.color_space
    }

    pub(crate) fn planes(&self) -> &[VideoYuvPlane] {
        &self.planes
    }

    pub(crate) fn decoded_bytes(&self) -> u64 {
        self.planes.iter().map(VideoYuvPlane::decoded_bytes).sum()
    }
}

impl VideoRenderFrame {
    pub(crate) fn rgba(texture: Arc<TextureFrame>) -> Self {
        Self::Rgba(texture)
    }

    #[allow(dead_code)]
    pub(crate) fn yuv(frame: Arc<VideoYuvFrame>) -> Self {
        Self::Yuv(frame)
    }

    #[allow(dead_code)]
    pub(crate) fn id(&self) -> u64 {
        match self {
            Self::Rgba(texture) => texture.id(),
            Self::Yuv(frame) => frame.id(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn revision(&self) -> u64 {
        match self {
            Self::Rgba(texture) => texture.revision(),
            Self::Yuv(frame) => frame.revision(),
        }
    }

    pub(crate) fn as_rgba_texture(&self) -> Option<Arc<TextureFrame>> {
        match self {
            Self::Rgba(texture) => Some(texture.clone()),
            Self::Yuv(_) => None,
        }
    }

    pub(crate) fn decoded_bytes(&self) -> u64 {
        match self {
            Self::Rgba(texture) => texture.pixels().len() as u64,
            Self::Yuv(frame) => frame.decoded_bytes(),
        }
    }
}

fn validate_yuv_plane(
    format: VideoYuvPlaneFormat,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    byte_len: usize,
) -> Result<(), TguiError> {
    if width == 0 || height == 0 {
        return Err(TguiError::Media(
            "invalid YUV plane: width and height must be non-zero".to_string(),
        ));
    }
    let bytes_per_texel = match format {
        VideoYuvPlaneFormat::R8 => 1,
        VideoYuvPlaneFormat::Rg8 => 2,
    };
    let min_row_bytes = width
        .checked_mul(bytes_per_texel)
        .ok_or_else(|| TguiError::Media("invalid YUV plane: row byte count overflow".into()))?;
    if bytes_per_row < min_row_bytes {
        return Err(TguiError::Media(format!(
            "invalid YUV plane: stride {bytes_per_row} is smaller than required row bytes {min_row_bytes}"
        )));
    }
    let required_len = u64::from(bytes_per_row)
        .checked_mul(u64::from(height))
        .ok_or_else(|| TguiError::Media("invalid YUV plane: byte length overflow".into()))?;
    if byte_len as u64 != required_len {
        return Err(TguiError::Media(format!(
            "invalid YUV plane: expected {required_len} bytes from stride and height but got {byte_len}"
        )));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_yuv_frame(
    width: u32,
    height: u32,
    format: VideoYuvFormat,
    planes: &[VideoYuvPlane],
) -> Result<(), TguiError> {
    if width == 0 || height == 0 {
        return Err(TguiError::Media(
            "invalid YUV frame: width and height must be non-zero".to_string(),
        ));
    }

    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let expected: &[(VideoYuvPlaneFormat, u32, u32)] = match format {
        VideoYuvFormat::Nv12 => &[
            (VideoYuvPlaneFormat::R8, width, height),
            (VideoYuvPlaneFormat::Rg8, chroma_width, chroma_height),
        ],
        VideoYuvFormat::Yuv420p => &[
            (VideoYuvPlaneFormat::R8, width, height),
            (VideoYuvPlaneFormat::R8, chroma_width, chroma_height),
            (VideoYuvPlaneFormat::R8, chroma_width, chroma_height),
        ],
    };

    if planes.len() != expected.len() {
        return Err(TguiError::Media(format!(
            "invalid YUV frame: {format:?} requires {} planes but got {}",
            expected.len(),
            planes.len()
        )));
    }

    for (index, (plane, (format, width, height))) in planes.iter().zip(expected).enumerate() {
        if plane.format != *format || plane.width != *width || plane.height != *height {
            return Err(TguiError::Media(format!(
                "invalid YUV frame: plane {index} expected {format:?} {width}x{height} but got {:?} {}x{}",
                plane.format, plane.width, plane.height
            )));
        }
    }

    Ok(())
}

#[derive(Clone)]
pub(crate) struct BackendSharedState {
    pub playback_state: State<VideoPlaybackState>,
    pub metrics: State<VideoMetrics>,
    pub volume: State<f32>,
    pub muted: State<bool>,
    pub looping: State<bool>,
    pub playback_rate: State<f32>,
    pub audio_tracks: State<Vec<VideoAudioTrack>>,
    pub audio_track_selection: State<VideoAudioTrackSelection>,
    pub subtitle_tracks: State<Vec<VideoSubtitleTrack>>,
    pub subtitle_track_selection: State<VideoSubtitleTrackSelection>,
    pub current_subtitle: State<Option<VideoSubtitleCue>>,
    pub current_subtitle_placement: State<Option<VideoSubtitleCuePlacement>>,
    pub current_subtitle_style: State<Option<VideoSubtitleCueStyle>>,
    pub current_subtitle_bitmap: State<Option<VideoSubtitleBitmapCue>>,
    pub metrics_observed: Arc<AtomicBool>,
    pub buffer_memory_limit_bytes: State<u64>,
    pub video_size: State<VideoSize>,
    pub error: State<Option<String>>,
    pub surface: State<VideoSurfaceSnapshot>,
}

impl BackendSharedState {
    pub fn enable_metrics(&self) {
        self.metrics_observed.store(true, Ordering::SeqCst);
    }

    pub fn metrics_enabled(&self) -> bool {
        self.metrics_observed.load(Ordering::SeqCst)
    }

    pub fn publish_frame(&self) {
        self.request_redraw();
    }

    pub fn request_redraw(&self) {
        self.surface.invalidation().request_redraw();
    }

    pub fn reset_for_load(&self) {
        self.playback_state.set(VideoPlaybackState::Loading);
        self.metrics.set(VideoMetrics::default());
        self.video_size.set(VideoSize::default());
        self.audio_tracks.set(Vec::new());
        self.subtitle_tracks.set(Vec::new());
        self.current_subtitle.set(None);
        self.current_subtitle_placement.set(None);
        self.current_subtitle_style.set(None);
        self.current_subtitle_bitmap.set(None);
        self.error.set(None);
        self.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::ZERO,
            texture: None,
            loading: true,
            error: None,
        });
    }

    pub fn reset_for_stop(&self) {
        self.playback_state.set(VideoPlaybackState::Idle);
        self.metrics.set(VideoMetrics::default());
        self.video_size.set(VideoSize::default());
        self.audio_tracks.set(Vec::new());
        self.subtitle_tracks.set(Vec::new());
        self.current_subtitle.set(None);
        self.current_subtitle_placement.set(None);
        self.current_subtitle_style.set(None);
        self.current_subtitle_bitmap.set(None);
        self.error.set(None);
        self.surface.set(VideoSurfaceSnapshot::default());
    }

    pub fn set_error(&self, message: String) {
        self.playback_state
            .set(VideoPlaybackState::Error(message.clone()));
        self.current_subtitle.set(None);
        self.current_subtitle_placement.set(None);
        self.current_subtitle_style.set(None);
        self.current_subtitle_bitmap.set(None);
        self.error.set(Some(message.clone()));
        self.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.video_size.get().intrinsic_size(),
            texture: None,
            loading: false,
            error: Some(message),
        });
    }
}

#[allow(dead_code)]
pub(crate) trait VideoBackend: Send + Sync {
    fn load(&self, source: VideoSource) -> Result<(), TguiError>;
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, position: std::time::Duration);
    fn set_volume(&self, volume: f32);
    fn set_muted(&self, muted: bool);
    fn set_looping(&self, looping: bool);
    fn set_playback_rate(&self, rate: f32);
    fn set_audio_track_selection(&self, selection: VideoAudioTrackSelection);
    fn set_subtitle_track_selection(&self, selection: VideoSubtitleTrackSelection);
    fn set_buffer_memory_limit_bytes(&self, bytes: u64);
    fn set_target_raster(&self, raster: Option<RasterRequest>);
    fn current_render_frame(&self) -> Option<VideoRenderFrame>;
    fn current_frame(&self) -> Option<Arc<TextureFrame>> {
        self.current_render_frame()
            .and_then(|frame| frame.as_rgba_texture())
    }
    fn shutdown(&self);

    fn on_surface_lost(&self) {}
    fn on_surface_restored(&self) {}
    fn on_app_background(&self) {}
    fn on_app_foreground(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yuv_plane(
        format: VideoYuvPlaneFormat,
        width: u32,
        height: u32,
        bytes_per_row: u32,
    ) -> VideoYuvPlane {
        let len = bytes_per_row as usize * height as usize;
        VideoYuvPlane::new(
            format,
            width,
            height,
            bytes_per_row,
            Arc::from(vec![0; len]),
        )
        .expect("test plane should be valid")
    }

    #[test]
    fn video_render_frame_wraps_rgba_texture_without_copying_pixels() {
        let texture = Arc::new(TextureFrame::new(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]));
        let frame = VideoRenderFrame::rgba(texture.clone());

        let projected = frame
            .as_rgba_texture()
            .expect("rgba render frame should expose a texture frame");

        assert!(Arc::ptr_eq(&texture, &projected));
        assert_eq!(frame.decoded_bytes(), 8);
    }

    #[test]
    fn video_render_frame_wraps_yuv_frame_without_rgba_projection() {
        let yuv = Arc::new(
            VideoYuvFrame::with_id_revision_and_planes(
                77,
                4,
                4,
                2,
                VideoYuvFormat::Nv12,
                VideoYuvColorSpace::default(),
                Arc::from(vec![
                    yuv_plane(VideoYuvPlaneFormat::R8, 4, 2, 4),
                    yuv_plane(VideoYuvPlaneFormat::Rg8, 2, 1, 4),
                ]),
            )
            .expect("nv12 frame should be valid"),
        );
        let frame = VideoRenderFrame::yuv(yuv.clone());

        assert_eq!(frame.id(), 77);
        assert_eq!(frame.revision(), 4);
        assert_eq!(frame.as_rgba_texture(), None);
        assert_eq!(frame.decoded_bytes(), 12);
        assert_eq!(yuv.size(), (4, 2));
        assert_eq!(yuv.format(), VideoYuvFormat::Nv12);
        assert_eq!(yuv.color_space(), VideoYuvColorSpace::default());
        assert_eq!(yuv.planes().len(), 2);
    }

    #[test]
    fn yuv420p_frame_uses_ceil_chroma_plane_dimensions() {
        let frame = VideoYuvFrame::new(
            3,
            3,
            VideoYuvFormat::Yuv420p,
            VideoYuvColorSpace {
                matrix: VideoYuvColorMatrix::Bt601,
                range: VideoYuvColorRange::Full,
            },
            Arc::from(vec![
                yuv_plane(VideoYuvPlaneFormat::R8, 3, 3, 3),
                yuv_plane(VideoYuvPlaneFormat::R8, 2, 2, 2),
                yuv_plane(VideoYuvPlaneFormat::R8, 2, 2, 2),
            ]),
        )
        .expect("odd-sized yuv420p frame should be valid");

        assert_eq!(frame.size(), (3, 3));
        assert_eq!(frame.decoded_bytes(), 17);
        assert_eq!(frame.planes()[1].width, 2);
        assert_eq!(frame.planes()[1].height, 2);
    }

    #[test]
    fn yuv_plane_rejects_short_stride_or_mismatched_length() {
        assert!(matches!(
            VideoYuvPlane::new(
                VideoYuvPlaneFormat::Rg8,
                2,
                1,
                3,
                Arc::from(vec![0; 3])
            ),
            Err(TguiError::Media(message)) if message.contains("stride")
        ));
        assert!(matches!(
            VideoYuvPlane::new(
                VideoYuvPlaneFormat::R8,
                4,
                2,
                4,
                Arc::from(vec![0; 7])
            ),
            Err(TguiError::Media(message)) if message.contains("expected 8 bytes")
        ));
    }

    #[test]
    fn yuv_frame_rejects_wrong_plane_layout() {
        assert!(matches!(
            VideoYuvFrame::new(
                4,
                2,
                VideoYuvFormat::Nv12,
                VideoYuvColorSpace::default(),
                Arc::from(vec![
                    yuv_plane(VideoYuvPlaneFormat::R8, 4, 2, 4),
                    yuv_plane(VideoYuvPlaneFormat::R8, 2, 1, 2),
                ]),
            ),
            Err(TguiError::Media(message)) if message.contains("plane 1")
        ));
    }
}
