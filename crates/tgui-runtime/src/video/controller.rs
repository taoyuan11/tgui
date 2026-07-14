use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::audio::backend::shared::normalize_playback_rate;
use crate::foundation::binding::{Signal, ViewModelContext};
use crate::foundation::error::TguiError;
use crate::media::RasterRequest;

use super::backend::{
    ffmpeg::FfmpegVideoBackend, BackendSharedState, VideoBackend,
    DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
};
use super::types::{
    VideoAudioTrack, VideoAudioTrackSelection, VideoMetrics, VideoPlaybackState, VideoSize,
    VideoSource, VideoSubtitleBitmapCue, VideoSubtitleCue, VideoSubtitleCuePlacement,
    VideoSubtitleCueStyle, VideoSubtitleTrack, VideoSubtitleTrackSelection, VideoSurfaceSnapshot,
};

/// Controls video playback and exposes bindable playback signals.
///
/// A controller is created inside a view model with [`ViewModelContext`] and can
/// be shared by [`Video`](crate::video::Video) or
/// [`VideoSurface`](crate::video::VideoSurface) widgets. The default backend is
/// FFmpeg-backed and is available when the crate is built with the `video`
/// feature.
#[derive(Clone)]
pub struct VideoController {
    inner: Arc<VideoControllerInner>,
}

struct VideoControllerInner {
    shared: BackendSharedState,
    backend: Arc<dyn VideoBackend>,
    target_raster_key: AtomicU64,
}

impl VideoController {
    /// Creates a new video controller bound to a view model context.
    pub fn new(ctx: &ViewModelContext) -> Self {
        let shared = BackendSharedState {
            playback_state: ctx.state(VideoPlaybackState::Idle),
            metrics: ctx.state(VideoMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            playback_rate: ctx.state(1.0),
            audio_tracks: ctx.state(Vec::new()),
            audio_track_selection: ctx.state(VideoAudioTrackSelection::Auto),
            subtitle_tracks: ctx.state(Vec::new()),
            subtitle_track_selection: ctx.state(VideoSubtitleTrackSelection::Disabled),
            current_subtitle: ctx.state(None),
            current_subtitle_placement: ctx.state(None),
            current_subtitle_style: ctx.state(None),
            current_subtitle_bitmap: ctx.state(None),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.state(VideoSize::default()),
            error: ctx.state(None),
            surface: ctx.state(VideoSurfaceSnapshot::default()),
        };
        let backend: Arc<dyn VideoBackend> = Arc::new(FfmpegVideoBackend::new(shared.clone()));
        Self::from_parts(shared, backend)
    }

    pub(crate) fn from_parts(shared: BackendSharedState, backend: Arc<dyn VideoBackend>) -> Self {
        Self {
            inner: Arc::new(VideoControllerInner {
                shared,
                backend,
                target_raster_key: AtomicU64::new(encode_target_raster(None)),
            }),
        }
    }

    /// Loads a new source and resets playback state to loading.
    ///
    /// If the backend rejects the source synchronously, the controller publishes
    /// an error state before returning the error.
    pub fn load(&self, source: impl Into<VideoSource>) -> Result<(), TguiError> {
        let source = source.into();
        self.inner.shared.reset_for_load();
        if let Err(error) = self.inner.backend.load(source) {
            self.inner.shared.set_error(error.to_string());
            return Err(error);
        }
        Ok(())
    }

    /// Starts or resumes playback.
    ///
    /// When playback has ended, this seeks to the beginning before playing.
    pub fn play(&self) {
        if self.inner.shared.playback_state.get() == VideoPlaybackState::Ended {
            self.replay();
            return;
        }
        self.inner.backend.play();
    }

    /// Seeks to the beginning and starts playback.
    pub fn replay(&self) {
        self.seek(Duration::ZERO);
        self.inner.backend.play();
    }

    /// Pauses playback at the current position.
    pub fn pause(&self) {
        self.inner.backend.pause();
    }

    /// Stops playback and clears the current frame and timeline state.
    pub fn stop(&self) {
        self.inner.backend.stop();
    }

    /// Seeks to a playback position.
    ///
    /// The exposed position signal is updated immediately; the backend then
    /// reopens or drains media around the requested position.
    pub fn seek(&self, position: Duration) {
        let mut metrics = self.inner.shared.metrics.get();
        metrics.position = position;
        self.inner.shared.metrics.set(metrics);
        self.inner.backend.seek(position);
    }

    /// Sets playback volume, clamped to `0.0..=1.0`.
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.inner.shared.volume.set(volume);
        self.inner.backend.set_volume(volume);
    }

    /// Enables or disables muted playback.
    pub fn set_muted(&self, muted: bool) {
        self.inner.shared.muted.set(muted);
        self.inner.backend.set_muted(muted);
    }

    /// Enables or disables looping at end of stream.
    ///
    /// Looping restarts the current source from the beginning after both video
    /// frames and audio buffers have drained.
    pub fn set_looping(&self, looping: bool) {
        self.inner.shared.looping.set(looping);
        self.inner.backend.set_looping(looping);
    }

    /// Sets playback speed, clamped to `0.25..=4.0`.
    ///
    /// This changes speed and pitch for decoded audio.
    pub fn set_playback_rate(&self, rate: f32) {
        let rate = normalize_playback_rate(rate);
        self.inner.shared.playback_rate.set(rate);
        self.inner.backend.set_playback_rate(rate);
    }

    /// Selects the audio track used for video playback.
    ///
    /// Use [`VideoAudioTrackSelection::Auto`] to let FFmpeg choose the best
    /// track, [`VideoAudioTrackSelection::Disabled`] to play without audio, or
    /// [`VideoAudioTrackSelection::Stream`] with a stream index from
    /// [`audio_tracks`](Self::audio_tracks). When a source is already loaded,
    /// changing this selection reopens that source at the current playback
    /// position so the backend can rebuild the audio pipeline.
    pub fn set_audio_track_selection(&self, selection: VideoAudioTrackSelection) {
        self.inner.shared.audio_track_selection.set(selection);
        self.inner.backend.set_audio_track_selection(selection);
    }

    /// Selects the subtitle track used for video playback.
    ///
    /// Use [`VideoSubtitleTrackSelection::Disabled`] to play without subtitles,
    /// or [`VideoSubtitleTrackSelection::Stream`] with a stream index from
    /// [`subtitle_tracks`](Self::subtitle_tracks). This stores the preference
    /// on the controller; subtitle rendering is built on top of this track
    /// metadata and selection state.
    pub fn set_subtitle_track_selection(&self, selection: VideoSubtitleTrackSelection) {
        self.inner.shared.subtitle_track_selection.set(selection);
        self.inner.backend.set_subtitle_track_selection(selection);
    }

    /// Sets the decoded media buffer memory limit in bytes.
    ///
    /// The limit is forwarded to the backend and is used to throttle buffering.
    pub fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.inner.shared.buffer_memory_limit_bytes.set(bytes);
        self.inner.backend.set_buffer_memory_limit_bytes(bytes);
    }

    /// Returns the current playback state.
    pub fn playback_state(&self) -> Signal<VideoPlaybackState> {
        self.inner.shared.playback_state.signal()
    }

    /// Returns the current playback position.
    pub fn position(&self) -> Signal<Duration> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.position)
    }

    /// Returns the current media duration, when known.
    pub fn duration(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.duration)
    }

    /// Returns the estimated buffered position, when known.
    pub fn buffered_position(&self) -> Signal<Option<Duration>> {
        self.inner.shared.enable_metrics();
        self.inner
            .shared
            .metrics
            .signal()
            .map(|metrics| metrics.buffered)
    }

    /// Returns the current volume.
    pub fn volume(&self) -> Signal<f32> {
        self.inner.shared.volume.signal()
    }

    /// Returns whether playback is muted.
    pub fn muted(&self) -> Signal<bool> {
        self.inner.shared.muted.signal()
    }

    /// Returns whether end-of-stream looping is enabled.
    pub fn looping(&self) -> Signal<bool> {
        self.inner.shared.looping.signal()
    }

    /// Returns the current playback speed multiplier.
    pub fn playback_rate(&self) -> Signal<f32> {
        self.inner.shared.playback_rate.signal()
    }

    /// Returns the audio tracks discovered in the current source.
    ///
    /// The list is cleared while a new source is loading and after playback is
    /// stopped. It is populated again after the backend opens the source.
    pub fn audio_tracks(&self) -> Signal<Vec<VideoAudioTrack>> {
        self.inner.shared.audio_tracks.signal()
    }

    /// Returns the requested audio track selection.
    ///
    /// The selection is kept across loads and stops until changed explicitly.
    pub fn audio_track_selection(&self) -> Signal<VideoAudioTrackSelection> {
        self.inner.shared.audio_track_selection.signal()
    }

    /// Returns the subtitle tracks discovered in the current source.
    ///
    /// The list is cleared while a new source is loading and after playback is
    /// stopped. It is populated again after the backend opens the source.
    pub fn subtitle_tracks(&self) -> Signal<Vec<VideoSubtitleTrack>> {
        self.inner.shared.subtitle_tracks.signal()
    }

    /// Returns the requested subtitle track selection.
    ///
    /// The selection is kept across loads and stops until changed explicitly.
    pub fn subtitle_track_selection(&self) -> Signal<VideoSubtitleTrackSelection> {
        self.inner.shared.subtitle_track_selection.signal()
    }

    /// Returns the subtitle cue currently active at the playback position.
    ///
    /// This is `None` when subtitles are disabled, when no selected text cue is
    /// active, or when the selected subtitle format cannot be represented as
    /// text.
    pub fn current_subtitle(&self) -> Signal<Option<VideoSubtitleCue>> {
        self.inner.shared.current_subtitle.signal()
    }

    /// Returns the bitmap subtitle cue currently active at the playback position.
    ///
    /// This is `None` when subtitles are disabled, when no selected bitmap cue
    /// is active, or when the selected subtitle format only emits text cues.
    pub fn current_subtitle_bitmap(&self) -> Signal<Option<VideoSubtitleBitmapCue>> {
        self.inner.shared.current_subtitle_bitmap.signal()
    }

    pub(crate) fn current_subtitle_placement(&self) -> Signal<Option<VideoSubtitleCuePlacement>> {
        self.inner.shared.current_subtitle_placement.signal()
    }

    pub(crate) fn current_subtitle_style(&self) -> Signal<Option<VideoSubtitleCueStyle>> {
        self.inner.shared.current_subtitle_style.signal()
    }

    /// Returns the decoded video dimensions in pixels.
    pub fn video_size(&self) -> Signal<VideoSize> {
        self.inner.shared.video_size.signal()
    }

    /// Returns the most recent playback or loading error.
    pub fn error(&self) -> Signal<Option<String>> {
        self.inner.shared.error.signal()
    }

    pub(crate) fn set_target_raster(&self, raster: Option<RasterRequest>) {
        let next_key = encode_target_raster(raster);
        let mut current_key = self.inner.target_raster_key.load(Ordering::Acquire);
        loop {
            if current_key == next_key {
                return;
            }
            match self.inner.target_raster_key.compare_exchange_weak(
                current_key,
                next_key,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => current_key = observed,
            }
        }
        self.inner.backend.set_target_raster(raster);
    }

    pub(crate) fn surface_metadata(&self) -> VideoSurfaceSnapshot {
        self.inner.shared.surface.get()
    }

    pub(crate) fn current_render_frame(&self) -> Option<super::backend::VideoRenderFrame> {
        self.inner.backend.current_render_frame()
    }

    #[allow(dead_code)]
    pub(crate) fn current_frame(&self) -> Option<Arc<crate::media::TextureFrame>> {
        self.current_render_frame()
            .and_then(|frame| frame.as_rgba_texture())
    }

    pub(crate) fn notify_surface_lost(&self) {
        self.inner.backend.on_surface_lost();
    }

    pub(crate) fn notify_surface_restored(&self) {
        self.inner.backend.on_surface_restored();
    }

    pub(crate) fn notify_app_background(&self) {
        self.inner.backend.on_app_background();
    }

    pub(crate) fn notify_app_foreground(&self) {
        self.inner.backend.on_app_foreground();
    }
}

fn encode_target_raster(raster: Option<RasterRequest>) -> u64 {
    const SOME_BIT: u64 = 1 << 63;
    raster
        .map(|raster| SOME_BIT | ((raster.width() as u64) << 32) | raster.height() as u64)
        .unwrap_or(0)
}

impl PartialEq for VideoController {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for VideoController {}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::media::{IntrinsicSize, MediaPlaybackSource, MediaSource, TextureFrame};

    use super::super::backend::{BackendSharedState, VideoBackend, VideoRenderFrame};
    use super::*;

    #[derive(Default)]
    struct RecordedCommands {
        loads: Vec<VideoSource>,
        commands: Vec<&'static str>,
        pause_count: usize,
        stop_count: usize,
        seeks: Vec<Duration>,
        volumes: Vec<f32>,
        muteds: Vec<bool>,
        loopings: Vec<bool>,
        playback_rates: Vec<f32>,
        audio_track_selections: Vec<VideoAudioTrackSelection>,
        subtitle_track_selections: Vec<VideoSubtitleTrackSelection>,
        buffer_memory_limits: Vec<u64>,
        target_rasters: Vec<Option<RasterRequest>>,
        lifecycle_events: Vec<&'static str>,
    }

    struct MockBackend {
        commands: Arc<Mutex<RecordedCommands>>,
        frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
        load_error: Option<&'static str>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                commands: Arc::new(Mutex::new(RecordedCommands::default())),
                frame: Arc::new(Mutex::new(None)),
                load_error: None,
            }
        }

        fn failing_load(message: &'static str) -> Self {
            Self {
                commands: Arc::new(Mutex::new(RecordedCommands::default())),
                frame: Arc::new(Mutex::new(None)),
                load_error: Some(message),
            }
        }
    }

    impl VideoBackend for MockBackend {
        fn load(&self, source: VideoSource) -> Result<(), TguiError> {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .loads
                .push(source);
            if let Some(message) = self.load_error {
                return Err(TguiError::Media(message.to_string()));
            }
            Ok(())
        }

        fn play(&self) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("play");
        }

        fn pause(&self) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("pause");
            commands.pause_count += 1;
        }

        fn stop(&self) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("stop");
            commands.stop_count += 1;
        }

        fn seek(&self, position: Duration) {
            let mut commands = self.commands.lock().expect("commands lock poisoned");
            commands.commands.push("seek");
            commands.seeks.push(position);
        }

        fn set_volume(&self, volume: f32) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .volumes
                .push(volume);
        }

        fn set_muted(&self, muted: bool) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .muteds
                .push(muted);
        }

        fn set_looping(&self, looping: bool) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .loopings
                .push(looping);
        }

        fn set_playback_rate(&self, rate: f32) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .playback_rates
                .push(rate);
        }

        fn set_audio_track_selection(&self, selection: VideoAudioTrackSelection) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .audio_track_selections
                .push(selection);
        }

        fn set_subtitle_track_selection(&self, selection: VideoSubtitleTrackSelection) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .subtitle_track_selections
                .push(selection);
        }

        fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .buffer_memory_limits
                .push(bytes);
        }

        fn set_target_raster(&self, raster: Option<RasterRequest>) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .target_rasters
                .push(raster);
        }

        fn current_render_frame(&self) -> Option<VideoRenderFrame> {
            self.frame
                .lock()
                .expect("frame lock poisoned")
                .clone()
                .map(VideoRenderFrame::rgba)
        }

        fn shutdown(&self) {}

        fn on_surface_lost(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .lifecycle_events
                .push("surface_lost");
        }

        fn on_surface_restored(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .lifecycle_events
                .push("surface_restored");
        }

        fn on_app_background(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .lifecycle_events
                .push("app_background");
        }

        fn on_app_foreground(&self) {
            self.commands
                .lock()
                .expect("commands lock poisoned")
                .lifecycle_events
                .push("app_foreground");
        }
    }

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    fn test_shared(ctx: &ViewModelContext) -> BackendSharedState {
        BackendSharedState {
            playback_state: ctx.state(VideoPlaybackState::Idle),
            metrics: ctx.state(VideoMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            playback_rate: ctx.state(1.0),
            audio_tracks: ctx.state(Vec::new()),
            audio_track_selection: ctx.state(VideoAudioTrackSelection::Auto),
            subtitle_tracks: ctx.state(Vec::new()),
            subtitle_track_selection: ctx.state(VideoSubtitleTrackSelection::Disabled),
            current_subtitle: ctx.state(None),
            current_subtitle_placement: ctx.state(None),
            current_subtitle_style: ctx.state(None),
            current_subtitle_bitmap: ctx.state(None),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.state(VideoSize::default()),
            error: ctx.state(None),
            surface: ctx.state(VideoSurfaceSnapshot::default()),
        }
    }

    #[test]
    fn controller_forwards_commands_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(VideoSource::File("demo.mp4".into()))
            .expect("mock load should succeed");
        controller.play();
        controller.pause();
        controller.stop();
        controller.seek(Duration::from_secs(9));
        controller.set_volume(0.25);
        controller.set_muted(true);
        controller.set_looping(true);
        controller.set_playback_rate(1.75);
        controller.set_audio_track_selection(VideoAudioTrackSelection::Stream(3));
        controller.set_subtitle_track_selection(VideoSubtitleTrackSelection::Stream(7));
        controller.set_buffer_memory_limit_bytes(32 * 1024 * 1024);

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.loads, vec![VideoSource::File("demo.mp4".into())]);
        assert_eq!(commands.commands, vec!["play", "pause", "stop", "seek"]);
        assert_eq!(commands.pause_count, 1);
        assert_eq!(commands.stop_count, 1);
        assert_eq!(commands.seeks, vec![Duration::from_secs(9)]);
        assert_eq!(commands.volumes, vec![0.25]);
        assert_eq!(commands.muteds, vec![true]);
        assert_eq!(commands.loopings, vec![true]);
        assert_eq!(commands.playback_rates, vec![1.75]);
        assert_eq!(
            commands.audio_track_selections,
            vec![VideoAudioTrackSelection::Stream(3)]
        );
        assert_eq!(
            commands.subtitle_track_selections,
            vec![VideoSubtitleTrackSelection::Stream(7)]
        );
        assert_eq!(commands.buffer_memory_limits, vec![32 * 1024 * 1024]);
    }

    #[test]
    fn controller_load_accepts_media_source() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(MediaSource::url("https://example.com/demo.mp4"))
            .expect("mock load should succeed");

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            commands.loads,
            vec![VideoSource::url("https://example.com/demo.mp4")]
        );
    }

    #[test]
    fn controller_load_accepts_media_playback_source() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(
                MediaPlaybackSource::url("https://example.com/demo.mp4")
                    .with_header("X-Test", "value"),
            )
            .expect("mock load should succeed");

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            commands.loads,
            vec![VideoSource::url("https://example.com/demo.mp4").with_header("X-Test", "value")]
        );
    }

    #[test]
    fn playback_rate_is_clamped_and_exposed_as_signal() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.set_playback_rate(8.0);
        controller.set_playback_rate(0.0);
        controller.set_playback_rate(f32::INFINITY);

        assert_eq!(controller.playback_rate().get(), 1.0);
        assert_eq!(
            commands
                .lock()
                .expect("commands lock poisoned")
                .playback_rates,
            vec![4.0, 0.25, 1.0]
        );
    }

    #[test]
    fn target_raster_updates_are_deduplicated() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        let small = Some(RasterRequest::new_clamped(320, 180));
        let large = Some(RasterRequest::new_clamped(640, 360));

        controller.set_target_raster(small);
        controller.set_target_raster(small);
        controller.set_target_raster(large);
        controller.set_target_raster(large);
        controller.set_target_raster(None);
        controller.set_target_raster(None);

        assert_eq!(
            commands
                .lock()
                .expect("commands lock poisoned")
                .target_rasters,
            vec![small, large, None]
        );
    }

    #[test]
    fn target_raster_keys_distinguish_none_and_dimensions() {
        let none = encode_target_raster(None);
        let small = encode_target_raster(Some(RasterRequest::new_clamped(320, 180)));
        let wide = encode_target_raster(Some(RasterRequest::new_clamped(320, 181)));
        let tall = encode_target_raster(Some(RasterRequest::new_clamped(321, 180)));

        assert_ne!(none, small);
        assert_ne!(small, wide);
        assert_ne!(small, tall);
        assert_eq!(
            small,
            encode_target_raster(Some(RasterRequest::new_clamped(320, 180)))
        );
    }

    #[test]
    fn controller_forwards_lifecycle_hooks_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.notify_app_background();
        controller.notify_surface_lost();
        controller.notify_surface_restored();
        controller.notify_app_foreground();

        assert_eq!(
            commands
                .lock()
                .expect("commands lock poisoned")
                .lifecycle_events,
            vec![
                "app_background",
                "surface_lost",
                "surface_restored",
                "app_foreground"
            ]
        );
    }

    #[test]
    fn load_failure_sets_error_state_and_surface_snapshot() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::failing_load("invalid video header"));
        let controller = VideoController::from_parts(shared.clone(), backend);

        let error = controller
            .load(VideoSource::url("https://example.com/demo.mp4"))
            .expect_err("mock load should fail");

        assert_eq!(error.to_string(), "invalid video header");
        assert_eq!(
            controller.playback_state().get(),
            VideoPlaybackState::Error("invalid video header".to_string())
        );
        assert_eq!(
            controller.error().get(),
            Some("invalid video header".to_string())
        );
        let snapshot = controller.surface_metadata();
        assert_eq!(snapshot.intrinsic_size, IntrinsicSize::ZERO);
        assert!(snapshot.texture.is_none());
        assert!(!snapshot.loading);
        assert_eq!(snapshot.error, Some("invalid video header".to_string()));
    }

    #[test]
    fn seek_updates_position_signal_immediately() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(30)),
            position: Duration::from_secs(3),
            buffered: Some(Duration::from_secs(12)),
            video_width: 16,
            video_height: 9,
        });
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::from_secs(9));

        assert_eq!(controller.position().get(), Duration::from_secs(9));
        assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
        assert_eq!(
            controller.buffered_position().get(),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            commands.lock().expect("commands lock poisoned").seeks,
            vec![Duration::from_secs(9)]
        );
    }

    #[test]
    fn controller_forwards_url_sources_with_headers_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller
            .load(
                VideoSource::url("https://example.com/demo.mp4")
                    .with_header("Authorization", "Bearer token")
                    .with_header("Referer", "https://example.com/app"),
            )
            .expect("mock load should succeed");

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            commands.loads,
            vec![VideoSource::Url {
                url: "https://example.com/demo.mp4".to_string(),
                headers: vec![
                    ("Authorization".to_string(), "Bearer token".to_string()),
                    ("Referer".to_string(), "https://example.com/app".to_string()),
                ],
            }]
        );
    }

    #[test]
    fn controller_bindings_reflect_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let frame = Arc::new(TextureFrame::new(8, 4, vec![255; 8 * 4 * 4]));
        *backend.frame.lock().expect("frame lock poisoned") = Some(frame.clone());
        let controller = VideoController::from_parts(shared.clone(), backend);

        shared.playback_state.set(VideoPlaybackState::Paused);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(30)),
            position: Duration::from_secs(12),
            buffered: Some(Duration::from_secs(16)),
            video_width: 8,
            video_height: 4,
        });
        shared.video_size.set(VideoSize {
            width: 8,
            height: 4,
        });
        shared.looping.set(true);
        shared.playback_rate.set(1.5);
        shared.audio_tracks.set(vec![VideoAudioTrack {
            stream_index: 3,
            title: Some("Commentary".to_string()),
            language: Some("en".to_string()),
            channels: 2,
            sample_rate: 48_000,
        }]);
        shared
            .audio_track_selection
            .set(VideoAudioTrackSelection::Stream(3));
        shared.subtitle_tracks.set(vec![VideoSubtitleTrack {
            stream_index: 7,
            title: Some("English CC".to_string()),
            language: Some("en".to_string()),
            codec: Some("subrip".to_string()),
        }]);
        shared
            .subtitle_track_selection
            .set(VideoSubtitleTrackSelection::Stream(7));
        shared.current_subtitle.set(Some(VideoSubtitleCue {
            text: "Caption line".to_string(),
            start: Duration::from_secs(10),
            end: Duration::from_secs(12),
        }));
        shared.error.set(Some("boom".to_string()));
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: IntrinsicSize::from_pixels(8, 4),
            texture: None,
            loading: false,
            error: None,
        });

        assert_eq!(
            controller.playback_state().get(),
            VideoPlaybackState::Paused
        );
        assert_eq!(controller.position().get(), Duration::from_secs(12));
        assert_eq!(controller.duration().get(), Some(Duration::from_secs(30)));
        assert_eq!(
            controller.buffered_position().get(),
            Some(Duration::from_secs(16))
        );
        assert_eq!(controller.video_size().get().width, 8);
        assert!(controller.looping().get());
        assert_eq!(controller.playback_rate().get(), 1.5);
        assert_eq!(
            controller.audio_tracks().get(),
            vec![VideoAudioTrack {
                stream_index: 3,
                title: Some("Commentary".to_string()),
                language: Some("en".to_string()),
                channels: 2,
                sample_rate: 48_000,
            }]
        );
        assert_eq!(
            controller.audio_track_selection().get(),
            VideoAudioTrackSelection::Stream(3)
        );
        assert_eq!(
            controller.subtitle_tracks().get(),
            vec![VideoSubtitleTrack {
                stream_index: 7,
                title: Some("English CC".to_string()),
                language: Some("en".to_string()),
                codec: Some("subrip".to_string()),
            }]
        );
        assert_eq!(
            controller.subtitle_track_selection().get(),
            VideoSubtitleTrackSelection::Stream(7)
        );
        assert_eq!(
            controller.current_subtitle().get(),
            Some(VideoSubtitleCue {
                text: "Caption line".to_string(),
                start: Duration::from_secs(10),
                end: Duration::from_secs(12),
            })
        );
        assert_eq!(controller.error().get(), Some("boom".to_string()));
        assert_eq!(
            controller
                .current_frame()
                .expect("backend frame should be available")
                .size(),
            (8, 4)
        );
    }

    #[test]
    fn play_restarts_from_beginning_after_playback_ended() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Ended);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.play();

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.commands, vec!["seek", "play"]);
        assert_eq!(commands.seeks, vec![Duration::ZERO]);
    }

    #[test]
    fn replay_seeks_to_start_then_plays() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.replay();

        let commands = commands.lock().expect("commands lock poisoned");
        assert_eq!(commands.commands, vec!["seek", "play"]);
        assert_eq!(commands.seeks, vec![Duration::ZERO]);
    }

    // 补充测试：更多边界情况和错误处理

    #[test]
    fn volume_clamps_to_valid_range() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared.clone(), backend);

        // 超出范围的值应被 clamp
        controller.set_volume(1.5);
        controller.set_volume(-0.5);
        controller.set_volume(0.5);

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            recorded.volumes,
            vec![1.0, 0.0, 0.5],
            "volumes should be clamped to [0.0, 1.0]"
        );
    }

    #[test]
    fn multiple_pause_calls_are_idempotent() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.pause();
        controller.pause();
        controller.pause();

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(
            recorded.pause_count, 3,
            "all pause calls should be forwarded"
        );
    }

    #[test]
    fn seek_then_play_maintains_position() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::from_secs(15));
        controller.play();

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.commands, vec!["seek", "play"]);
        assert_eq!(recorded.seeks, vec![Duration::from_secs(15)]);
    }

    #[test]
    fn buffer_memory_limit_propagates_to_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        let limits = vec![16 * 1024 * 1024, 64 * 1024 * 1024, 128 * 1024 * 1024];
        for limit in &limits {
            controller.set_buffer_memory_limit_bytes(*limit);
        }

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.buffer_memory_limits, limits);
    }

    #[test]
    fn mute_unmute_sequence() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller.set_muted(true);
        assert!(shared.muted.get(), "should be muted");

        controller.set_muted(false);
        assert!(!shared.muted.get(), "should be unmuted");

        controller.set_muted(true);
        assert!(shared.muted.get(), "should be muted again");

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.muteds, vec![true, false, true]);
    }

    #[test]
    fn looping_updates_shared_state_and_backend() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller.set_looping(true);
        assert!(shared.looping.get(), "looping should be enabled");
        assert!(controller.looping().get());

        controller.set_looping(false);
        assert!(!shared.looping.get(), "looping should be disabled");
        assert!(!controller.looping().get());

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.loopings, vec![true, false]);
    }

    #[test]
    fn load_resets_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Playing);
        shared.metrics.set(VideoMetrics {
            duration: Some(Duration::from_secs(60)),
            position: Duration::from_secs(30),
            buffered: Some(Duration::from_secs(45)),
            video_width: 1920,
            video_height: 1080,
        });
        shared.audio_tracks.set(vec![VideoAudioTrack {
            stream_index: 5,
            title: Some("Director commentary".to_string()),
            language: Some("en".to_string()),
            channels: 2,
            sample_rate: 48_000,
        }]);
        shared
            .audio_track_selection
            .set(VideoAudioTrackSelection::Stream(5));
        shared.subtitle_tracks.set(vec![VideoSubtitleTrack {
            stream_index: 8,
            title: Some("English CC".to_string()),
            language: Some("en".to_string()),
            codec: Some("subrip".to_string()),
        }]);
        shared
            .subtitle_track_selection
            .set(VideoSubtitleTrackSelection::Stream(8));
        shared.current_subtitle.set(Some(VideoSubtitleCue {
            text: "Old caption".to_string(),
            start: Duration::from_secs(3),
            end: Duration::from_secs(4),
        }));

        let backend = Arc::new(MockBackend::new());
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller
            .load(VideoSource::File("new.mp4".into()))
            .expect("load should succeed");

        // reset_for_load 设置状态为 Loading（正在加载新源）
        assert_eq!(shared.playback_state.get(), VideoPlaybackState::Loading);
        assert_eq!(shared.metrics.get().position, Duration::ZERO);
        assert!(shared.audio_tracks.get().is_empty());
        assert_eq!(
            shared.audio_track_selection.get(),
            VideoAudioTrackSelection::Stream(5)
        );
        assert!(shared.subtitle_tracks.get().is_empty());
        assert_eq!(
            shared.subtitle_track_selection.get(),
            VideoSubtitleTrackSelection::Stream(8)
        );
        assert_eq!(shared.current_subtitle.get(), None);
    }

    #[test]
    fn play_when_idle_does_not_seek() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Idle);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.play();

        let recorded = commands.lock().expect("commands lock poisoned");
        // Idle 状态下 play() 不应触发 seek，只调用 play
        assert_eq!(recorded.commands, vec!["play"]);
        assert!(recorded.seeks.is_empty());
    }

    #[test]
    fn seek_to_zero_is_valid() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let commands = backend.commands.clone();
        let controller = VideoController::from_parts(shared, backend);

        controller.seek(Duration::ZERO);

        let recorded = commands.lock().expect("commands lock poisoned");
        assert_eq!(recorded.seeks, vec![Duration::ZERO]);
        assert_eq!(controller.position().get(), Duration::ZERO);
    }

    #[test]
    fn volume_and_mute_are_independent() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(MockBackend::new());
        let controller = VideoController::from_parts(shared.clone(), backend);

        controller.set_volume(0.5);
        controller.set_muted(true);

        assert_eq!(
            shared.volume.get(),
            0.5,
            "volume setting independent of mute"
        );
        assert!(shared.muted.get(), "mute flag should be set");

        controller.set_muted(false);
        assert_eq!(
            shared.volume.get(),
            0.5,
            "unmuting should not change volume"
        );
    }
}
