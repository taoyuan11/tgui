use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::foundation::binding::{Binding, InvalidationSignal};
use crate::foundation::color::Color;
use crate::foundation::error::TguiError;
use crate::ui::widget::{Rect, WidgetId};

#[cfg(all(feature = "video-ffmpeg", target_os = "windows"))]
#[link(name = "mfuuid")]
#[link(name = "strmiids")]
unsafe extern "system" {}

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_VIDEO_CONTROLLER_ID: AtomicU64 = AtomicU64::new(1);
static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
const MAX_IMAGE_DIMENSION: u32 = 2048;
const DEFAULT_VIDEO_FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoPlaybackStatus {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Ended,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoPlaybackSnapshot {
    pub status: VideoPlaybackStatus,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub progress: f32,
    pub muted: bool,
    pub volume: f32,
    pub looping: bool,
}

impl Default for VideoPlaybackSnapshot {
    fn default() -> Self {
        Self {
            status: VideoPlaybackStatus::Idle,
            position: Duration::ZERO,
            duration: None,
            progress: 0.0,
            muted: false,
            volume: 1.0,
            looping: false,
        }
    }
}

#[derive(Clone)]
pub struct VideoControllerHandle {
    shared: Arc<VideoControllerShared>,
}

struct VideoControllerShared {
    id: u64,
    snapshot: Mutex<VideoPlaybackSnapshot>,
    commands: Mutex<VecDeque<VideoControlCommand>>,
    invalidation: InvalidationSignal,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
enum VideoControlCommand {
    Play,
    Pause,
    Resume,
    Replay,
    SeekTime(Duration),
    SetMuted(bool),
    SetVolume(f32),
    SetLooping(bool),
}

impl VideoControllerHandle {
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self {
            shared: Arc::new(VideoControllerShared {
                id: NEXT_VIDEO_CONTROLLER_ID.fetch_add(1, Ordering::Relaxed),
                snapshot: Mutex::new(VideoPlaybackSnapshot::default()),
                commands: Mutex::new(VecDeque::new()),
                invalidation,
            }),
        }
    }

    pub fn play(&self) {
        self.push_command(VideoControlCommand::Play);
    }

    pub fn pause(&self) {
        self.push_command(VideoControlCommand::Pause);
    }

    pub fn resume(&self) {
        self.push_command(VideoControlCommand::Resume);
    }

    pub fn replay(&self) {
        self.push_command(VideoControlCommand::Replay);
    }

    pub fn seek_time(&self, position: Duration) {
        self.push_command(VideoControlCommand::SeekTime(position));
    }

    pub fn seek_percent(&self, percent: f32) {
        let duration = self.snapshot().duration.unwrap_or(Duration::ZERO);
        let target =
            Duration::from_secs_f64(duration.as_secs_f64() * percent.clamp(0.0, 1.0) as f64);
        self.seek_time(target);
    }

    pub fn set_muted(&self, muted: bool) {
        {
            let mut snapshot = self
                .shared
                .snapshot
                .lock()
                .expect("video controller lock poisoned");
            snapshot.muted = muted;
        }
        self.push_command(VideoControlCommand::SetMuted(muted));
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        {
            let mut snapshot = self
                .shared
                .snapshot
                .lock()
                .expect("video controller lock poisoned");
            snapshot.volume = volume;
        }
        self.push_command(VideoControlCommand::SetVolume(volume));
    }

    pub fn set_looping(&self, looping: bool) {
        {
            let mut snapshot = self
                .shared
                .snapshot
                .lock()
                .expect("video controller lock poisoned");
            snapshot.looping = looping;
        }
        self.push_command(VideoControlCommand::SetLooping(looping));
    }

    pub fn snapshot(&self) -> VideoPlaybackSnapshot {
        self.shared
            .snapshot
            .lock()
            .expect("video controller lock poisoned")
            .clone()
    }

    pub fn status_binding(&self) -> Binding<VideoPlaybackStatus> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().status.clone())
    }

    pub fn position_binding(&self) -> Binding<Duration> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().position)
    }

    pub fn duration_binding(&self) -> Binding<Option<Duration>> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().duration)
    }

    pub fn progress_binding(&self) -> Binding<f32> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().progress)
    }

    pub fn muted_binding(&self) -> Binding<bool> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().muted)
    }

    pub fn volume_binding(&self) -> Binding<f32> {
        let controller = self.clone();
        Binding::new(move || controller.snapshot().volume)
    }

    pub(crate) fn id(&self) -> u64 {
        self.shared.id
    }

    fn push_command(&self, command: VideoControlCommand) {
        self.shared
            .commands
            .lock()
            .expect("video controller lock poisoned")
            .push_back(command);
        self.shared.invalidation.mark_dirty();
    }

    #[allow(dead_code)]
    fn take_commands(&self) -> Vec<VideoControlCommand> {
        let mut commands = self
            .shared
            .commands
            .lock()
            .expect("video controller lock poisoned");
        commands.drain(..).collect()
    }

    fn clear_commands(&self) {
        self.shared
            .commands
            .lock()
            .expect("video controller lock poisoned")
            .clear();
    }

    fn publish_snapshot(&self, snapshot: VideoPlaybackSnapshot) {
        let mut guard = self
            .shared
            .snapshot
            .lock()
            .expect("video controller lock poisoned");
        if *guard != snapshot {
            *guard = snapshot;
            self.shared.invalidation.mark_dirty();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MediaSource {
    Path(PathBuf),
    Url(String),
}

impl MediaSource {
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }
}

impl From<PathBuf> for MediaSource {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

impl From<&Path> for MediaSource {
    fn from(value: &Path) -> Self {
        Self::Path(value.to_path_buf())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContentFit {
    #[default]
    Contain,
    Cover,
    Fill,
}

#[derive(Clone, Debug)]
pub(crate) struct TextureFrame {
    id: u64,
    revision: u64,
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

impl TextureFrame {
    pub(crate) fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            id: NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed),
            revision: 1,
            width,
            height,
            pixels: Arc::from(pixels),
        }
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

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntrinsicSize {
    pub width: f32,
    pub height: f32,
}

impl IntrinsicSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub fn from_pixels(width: u32, height: u32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
        }
    }

    pub fn aspect_ratio(self) -> Option<f32> {
        (self.width > 0.0 && self.height > 0.0).then_some(self.width / self.height)
    }
}

#[derive(Clone)]
pub(crate) struct ImageSnapshot {
    pub intrinsic_size: IntrinsicSize,
    pub texture: Option<Arc<TextureFrame>>,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for ImageSnapshot {
    fn default() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: false,
            error: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoConfig {
    pub autoplay: bool,
    pub looping: bool,
    #[allow(dead_code)]
    pub muted: bool,
    #[allow(dead_code)]
    pub volume: f32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            autoplay: false,
            looping: false,
            muted: false,
            volume: 1.0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct VideoSnapshot {
    pub intrinsic_size: IntrinsicSize,
    pub texture: Option<Arc<TextureFrame>>,
    pub loading: bool,
    pub error: Option<String>,
    pub playback: VideoPlaybackSnapshot,
    pub seek_generation: u64,
}

impl Default for VideoSnapshot {
    fn default() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: false,
            error: None,
            playback: VideoPlaybackSnapshot::default(),
            seek_generation: 0,
        }
    }
}

pub(crate) fn resolve_media_rect(frame: Rect, media: IntrinsicSize, fit: ContentFit) -> Rect {
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return Rect::new(frame.x, frame.y, 0.0, 0.0);
    }

    if media.width <= 0.0 || media.height <= 0.0 || fit == ContentFit::Fill {
        return frame;
    }

    let frame_ratio = frame.width / frame.height.max(1.0);
    let media_ratio = media.width / media.height.max(1.0);

    let (width, height) = match fit {
        ContentFit::Contain => {
            if media_ratio > frame_ratio {
                (frame.width, frame.width / media_ratio)
            } else {
                (frame.height * media_ratio, frame.height)
            }
        }
        ContentFit::Cover => {
            if media_ratio > frame_ratio {
                (frame.height * media_ratio, frame.height)
            } else {
                (frame.width, frame.width / media_ratio)
            }
        }
        ContentFit::Fill => (frame.width, frame.height),
    };

    Rect::new(
        frame.x + (frame.width - width) * 0.5,
        frame.y + (frame.height - height) * 0.5,
        width,
        height,
    )
}

pub(crate) struct MediaManager {
    invalidation: InvalidationSignal,
    images: Mutex<HashMap<MediaSource, Arc<Mutex<ImageEntry>>>>,
    videos: Mutex<HashMap<WidgetId, Arc<Mutex<VideoEntry>>>>,
}

impl MediaManager {
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self {
            invalidation,
            images: Mutex::new(HashMap::new()),
            videos: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn image_snapshot(&self, source: &MediaSource) -> ImageSnapshot {
        let entry = {
            let mut images = self.images.lock().expect("image cache lock poisoned");
            images
                .entry(source.clone())
                .or_insert_with(|| {
                    let entry = Arc::new(Mutex::new(ImageEntry::loading()));
                    spawn_image_loader(entry.clone(), source.clone(), self.invalidation.clone());
                    entry
                })
                .clone()
        };

        let snapshot = entry.lock().expect("image entry lock poisoned").snapshot();
        snapshot
    }

    pub(crate) fn video_snapshot(
        &self,
        widget_id: WidgetId,
        source: &MediaSource,
        config: VideoConfig,
        controller: Option<&VideoControllerHandle>,
    ) -> VideoSnapshot {
        let entry = {
            let mut videos = self.videos.lock().expect("video cache lock poisoned");
            let entry = videos
                .entry(widget_id)
                .or_insert_with(|| {
                    Arc::new(Mutex::new(VideoEntry::new(
                        source.clone(),
                        config,
                        controller.cloned(),
                    )))
                })
                .clone();
            {
                let mut guard = entry.lock().expect("video entry lock poisoned");
                guard.sync_source(
                    source,
                    config,
                    controller.cloned(),
                    self.invalidation.clone(),
                    widget_id,
                );
            }
            entry
        };
        let snapshot = entry.lock().expect("video entry lock poisoned").snapshot();
        snapshot
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        let videos = self.videos.lock().expect("video cache lock poisoned");
        videos
            .values()
            .filter_map(|entry| {
                let entry = entry.lock().expect("video entry lock poisoned");
                entry.next_frame_deadline(now)
            })
            .min()
    }

    pub(crate) fn has_active_video(&self) -> bool {
        self.videos
            .lock()
            .expect("video cache lock poisoned")
            .values()
            .any(|entry| entry.lock().expect("video entry lock poisoned").is_active())
    }
}

struct ImageEntry {
    intrinsic_size: IntrinsicSize,
    texture: Option<Arc<TextureFrame>>,
    loading: bool,
    error: Option<String>,
}

impl ImageEntry {
    fn loading() -> Self {
        Self {
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            loading: true,
            error: None,
        }
    }

    fn snapshot(&self) -> ImageSnapshot {
        ImageSnapshot {
            intrinsic_size: self.intrinsic_size,
            texture: self.texture.clone(),
            loading: self.loading,
            error: self.error.clone(),
        }
    }
}

fn spawn_image_loader(
    entry: Arc<Mutex<ImageEntry>>,
    source: MediaSource,
    invalidation: InvalidationSignal,
) {
    thread::spawn(move || {
        let result = load_image_source(&source);
        let mut guard = entry.lock().expect("image entry lock poisoned");
        match result {
            Ok(texture) => {
                let (width, height) = texture.size();
                guard.intrinsic_size = IntrinsicSize::from_pixels(width, height);
                guard.texture = Some(Arc::new(texture));
                guard.loading = false;
                guard.error = None;
            }
            Err(error) => {
                guard.intrinsic_size = IntrinsicSize::ZERO;
                guard.texture = None;
                guard.loading = false;
                guard.error = Some(error.to_string());
            }
        }
        invalidation.mark_dirty();
    });
}

fn load_image_source(source: &MediaSource) -> Result<TextureFrame, TguiError> {
    let bytes = match source {
        MediaSource::Path(path) => fs::read(path).map_err(|error| {
            TguiError::Media(format!("failed to read image {:?}: {error}", path))
        })?,
        MediaSource::Url(url) => http_client()
            .get(url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| TguiError::Media(format!("failed to fetch image {url}: {error}")))?
            .bytes()
            .map_err(|error| TguiError::Media(format!("failed to read image body {url}: {error}")))?
            .to_vec(),
    };

    let mut image = image::load_from_memory(&bytes)
        .map_err(|error| TguiError::Media(format!("failed to decode image {source:?}: {error}")))?;
    let longest_edge = image.width().max(image.height());
    if longest_edge > MAX_IMAGE_DIMENSION {
        let scale = MAX_IMAGE_DIMENSION as f32 / longest_edge as f32;
        let width = (image.width() as f32 * scale).round().max(1.0) as u32;
        let height = (image.height() as f32 * scale).round().max(1.0) as u32;
        image = image.resize(width, height, image::imageops::FilterType::Triangle);
    }
    let rgba = image.to_rgba8();
    Ok(TextureFrame::new(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}

fn http_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client should build")
    })
}

struct VideoEntry {
    source: MediaSource,
    config: VideoConfig,
    state: Arc<Mutex<VideoPlaybackState>>,
    worker: Option<VideoWorkerHandle>,
    controller_id: Option<u64>,
}

impl VideoEntry {
    fn new(
        source: MediaSource,
        config: VideoConfig,
        controller: Option<VideoControllerHandle>,
    ) -> Self {
        Self {
            source,
            config,
            state: Arc::new(Mutex::new(VideoPlaybackState::new(
                config,
                controller.clone(),
            ))),
            worker: None,
            controller_id: controller.as_ref().map(VideoControllerHandle::id),
        }
    }

    fn sync_source(
        &mut self,
        source: &MediaSource,
        config: VideoConfig,
        controller: Option<VideoControllerHandle>,
        invalidation: InvalidationSignal,
        widget_id: WidgetId,
    ) {
        let source_changed = &self.source != source;
        let controller_id = controller.as_ref().map(VideoControllerHandle::id);
        let controller_changed = self.controller_id != controller_id;
        if source_changed {
            if let Some(worker) = self.worker.take() {
                worker.stop();
            }
            self.source = source.clone();
            self.state = Arc::new(Mutex::new(VideoPlaybackState::new(
                config,
                controller.clone(),
            )));
            self.controller_id = controller_id;
            if let Some(controller) = controller.as_ref() {
                controller.clear_commands();
            }
        }

        self.config = config;
        {
            let mut state = self.state.lock().expect("video state lock poisoned");
            state.set_controller(controller.clone());
            if source_changed {
                state.publish_controller_snapshot();
            } else if controller_changed {
                self.controller_id = controller_id;
                state.publish_controller_snapshot();
            } else if controller.is_none() {
                state.apply_config(config);
            }
        }

        if self.worker.is_none() {
            self.worker = Some(spawn_video_loader(
                widget_id,
                self.source.clone(),
                self.state.clone(),
                invalidation,
                config,
            ));
        }
    }

    fn snapshot(&self) -> VideoSnapshot {
        self.state
            .lock()
            .expect("video state lock poisoned")
            .snapshot()
    }

    fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        self.state
            .lock()
            .expect("video state lock poisoned")
            .next_frame_deadline(now)
    }

    fn is_active(&self) -> bool {
        !self.state.lock().expect("video state lock poisoned").paused
    }
}

struct VideoPlaybackState {
    loading: bool,
    error: Option<String>,
    intrinsic_size: IntrinsicSize,
    texture: Option<Arc<TextureFrame>>,
    position: Duration,
    duration: Option<Duration>,
    paused: bool,
    looping: bool,
    muted: bool,
    volume: f32,
    ended: bool,
    frame_interval: Duration,
    seek_generation: u64,
    controller: Option<VideoControllerHandle>,
}

impl VideoPlaybackState {
    fn new(config: VideoConfig, controller: Option<VideoControllerHandle>) -> Self {
        let state = Self {
            loading: true,
            error: None,
            intrinsic_size: IntrinsicSize::ZERO,
            texture: None,
            paused: !config.autoplay,
            duration: None,
            looping: config.looping,
            muted: config.muted,
            volume: config.volume,
            position: Duration::ZERO,
            ended: false,
            frame_interval: DEFAULT_VIDEO_FRAME_INTERVAL,
            seek_generation: 0,
            controller,
        };
        state
    }

    fn apply_config(&mut self, config: VideoConfig) {
        self.looping = config.looping;
        self.muted = config.muted;
        self.volume = config.volume;
        if !config.autoplay && self.position.is_zero() {
            self.paused = true;
        }
        if config.autoplay && self.position.is_zero() && self.texture.is_some() {
            self.paused = false;
        }
        self.publish_controller_snapshot();
    }

    fn set_controller(&mut self, controller: Option<VideoControllerHandle>) {
        self.controller = controller;
    }

    fn snapshot(&self) -> VideoSnapshot {
        VideoSnapshot {
            intrinsic_size: self.intrinsic_size,
            texture: self.texture.clone(),
            loading: self.loading,
            error: self.error.clone(),
            playback: self.playback_snapshot(),
            seek_generation: self.seek_generation,
        }
    }

    fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        (!self.paused).then_some(now + self.frame_interval)
    }

    fn playback_snapshot(&self) -> VideoPlaybackSnapshot {
        let progress = self
            .duration
            .filter(|duration| !duration.is_zero())
            .map(|duration| {
                (self.position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
            })
            .unwrap_or(0.0);

        VideoPlaybackSnapshot {
            status: self.status(),
            position: self.position,
            duration: self.duration,
            progress,
            muted: self.muted,
            volume: self.volume,
            looping: self.looping,
        }
    }

    fn status(&self) -> VideoPlaybackStatus {
        if let Some(error) = self.error.as_ref() {
            return VideoPlaybackStatus::Error(error.clone());
        }
        if self.loading {
            return VideoPlaybackStatus::Loading;
        }
        if self.ended {
            return VideoPlaybackStatus::Ended;
        }
        if self.paused {
            if self.position.is_zero() {
                VideoPlaybackStatus::Ready
            } else {
                VideoPlaybackStatus::Paused
            }
        } else {
            VideoPlaybackStatus::Playing
        }
    }

    fn publish_controller_snapshot(&self) {
        if let Some(controller) = self.controller.as_ref() {
            controller.publish_snapshot(self.playback_snapshot());
        }
    }
}

fn spawn_video_loader(
    widget_id: WidgetId,
    source: MediaSource,
    state: Arc<Mutex<VideoPlaybackState>>,
    invalidation: InvalidationSignal,
    config: VideoConfig,
) -> VideoWorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    thread::spawn(move || {
        let result = run_video_session(
            widget_id,
            &source,
            state.clone(),
            invalidation.clone(),
            worker_stop,
            config,
        );
        if let Err(error) = result {
            let mut guard = state.lock().expect("video state lock poisoned");
            guard.loading = false;
            guard.error = Some(error.to_string());
            guard.texture = None;
            guard.paused = true;
            guard.ended = false;
            guard.publish_controller_snapshot();
            invalidation.mark_dirty();
        }
    });
    VideoWorkerHandle { stop }
}

struct VideoWorkerHandle {
    stop: Arc<AtomicBool>,
}

impl VideoWorkerHandle {
    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(all(
    feature = "video-ffmpeg",
    any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
fn run_video_session(
    widget_id: WidgetId,
    source: &MediaSource,
    state: Arc<Mutex<VideoPlaybackState>>,
    invalidation: InvalidationSignal,
    stop: Arc<AtomicBool>,
    config: VideoConfig,
) -> Result<(), TguiError> {
    let _ = widget_id;
    ffmpeg_impl::run_video_session(source, state, invalidation, stop, config)
}

#[cfg(not(all(
    feature = "video-ffmpeg",
    any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )
)))]
fn run_video_session(
    widget_id: WidgetId,
    source: &MediaSource,
    state: Arc<Mutex<VideoPlaybackState>>,
    invalidation: InvalidationSignal,
    stop: Arc<AtomicBool>,
    config: VideoConfig,
) -> Result<(), TguiError> {
    let _ = (widget_id, source, state, invalidation, stop, config);
    Err(TguiError::Unsupported(
        "video playback requires the `video-ffmpeg` feature on desktop targets".to_string(),
    ))
}

#[cfg(all(
    feature = "video-ffmpeg",
    any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
#[path = "browser_pipeline.rs"]
mod ffmpeg_impl;

#[cfg(all(
    feature = "video-ffmpeg",
    any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
#[allow(dead_code)]
mod legacy_ffmpeg_impl {
    use super::{
        IntrinsicSize, MediaSource, TextureFrame, VideoConfig, VideoControlCommand,
        VideoPlaybackState,
    };
    use crate::foundation::binding::InvalidationSignal;
    use crate::foundation::error::TguiError;
    use bytemuck::cast_slice;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
    use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError};
    use ffmpeg_next as ffmpeg;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    const AUDIO_BUFFER_AHEAD: Duration = Duration::from_millis(900);
    const AUDIO_SYNC_PREROLL: Duration = Duration::from_millis(180);
    const VIDEO_PACKET_CHANNEL_CAPACITY: usize = 192;
    const VIDEO_BUFFER_LOW_WATERMARK: Duration = Duration::from_secs(2);
    const VIDEO_BUFFER_RESUME_WATERMARK: Duration = Duration::from_secs(5);
    const VIDEO_DROP_GOP_WATERMARK: Duration = Duration::from_secs(8);
    const VIDEO_LATE_FRAME_THRESHOLD: Duration = Duration::from_millis(120);
    const BUFFER_WAIT_SLICE: Duration = Duration::from_millis(20);
    const NETWORK_OPEN_TIMEOUT: Duration = Duration::from_secs(8);
    const NETWORK_RW_TIMEOUT: Duration = Duration::from_secs(10);
    const NETWORK_BUFFER_SIZE: usize = 512 * 1024;
    const NETWORK_FIFO_SIZE: usize = 2 * 1024 * 1024;
    const STREAM_PROBE_SIZE: usize = 256 * 1024;
    const STREAM_ANALYZE_DURATION: Duration = Duration::from_millis(800);

    struct AudioPlayback {
        _stream: Stream,
        shared: Arc<Mutex<AudioOutputShared>>,
        worker: AudioWorkerHandle,
        source: MediaSource,
        base_offset: Duration,
        ready_for_sync: Arc<AtomicBool>,
        output_sample_rate: u32,
        decode_channels: u16,
    }

    struct AudioOutputShared {
        samples: VecDeque<f32>,
        queued_frames: u64,
        played_frames: u64,
        output_channels: u16,
        decode_channels: u16,
        muted: bool,
        volume: f32,
        paused: bool,
        generation: u64,
    }

    impl AudioPlayback {
        fn new(
            source: MediaSource,
            muted: bool,
            volume: f32,
            start_offset: Duration,
            playing: bool,
        ) -> Result<Self, TguiError> {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or_else(|| TguiError::Media("failed to open default audio output".to_string()))?;
            let supported_config = device.default_output_config().map_err(|error| {
                TguiError::Media(format!("failed to query default audio output: {error}"))
            })?;
            let sample_format = supported_config.sample_format();
            let config = supported_config.config();
            let output_channels = config.channels.max(1);
            let decode_channels = if output_channels <= 1 { 1 } else { 2 };
            let ready_for_sync = Arc::new(AtomicBool::new(false));
            let shared = Arc::new(Mutex::new(AudioOutputShared {
                samples: VecDeque::new(),
                queued_frames: 0,
                played_frames: 0,
                output_channels,
                decode_channels,
                muted,
                volume: volume.clamp(0.0, 1.0),
                paused: !playing,
                generation: 0,
            }));
            let stream = build_audio_output_stream(
                &device,
                &config,
                sample_format,
                shared.clone(),
                ready_for_sync.clone(),
            )?;
            stream
                .play()
                .map_err(|error| TguiError::Media(format!("failed to start audio output: {error}")))?;

            let mut playback = Self {
                _stream: stream,
                shared,
                worker: AudioWorkerHandle::stopped(),
                source,
                base_offset: start_offset,
                ready_for_sync,
                output_sample_rate: config.sample_rate.max(1),
                decode_channels,
            };
            playback.worker = spawn_audio_worker(
                playback.source.clone(),
                playback.shared.clone(),
                playback.ready_for_sync.clone(),
                start_offset,
                playback.output_sample_rate,
                playback.decode_channels,
            );
            Ok(playback)
        }

        fn restart_stream(
            &mut self,
            start_offset: Duration,
            playing: bool,
        ) -> Result<(), TguiError> {
            self.worker.stop();
            {
                let mut shared = self.shared.lock().expect("audio state lock poisoned");
                shared.samples.clear();
                shared.queued_frames = 0;
                shared.played_frames = 0;
                shared.paused = !playing;
                shared.generation = shared.generation.saturating_add(1);
            }
            self.base_offset = start_offset;
            self.ready_for_sync.store(false, Ordering::Release);
            self.worker = spawn_audio_worker(
                self.source.clone(),
                self.shared.clone(),
                self.ready_for_sync.clone(),
                start_offset,
                self.output_sample_rate,
                self.decode_channels,
            );
            Ok(())
        }

        fn pause(&mut self) {
            let mut shared = self.shared.lock().expect("audio state lock poisoned");
            shared.paused = true;
        }

        fn resume(&mut self) {
            let mut shared = self.shared.lock().expect("audio state lock poisoned");
            shared.paused = false;
        }

        fn seek(&mut self, position: Duration, playing: bool) -> Result<(), TguiError> {
            self.restart_stream(position, playing)
        }

        fn set_muted(&mut self, muted: bool) {
            let mut shared = self.shared.lock().expect("audio state lock poisoned");
            shared.muted = muted;
        }

        fn set_volume(&mut self, volume: f32) {
            let mut shared = self.shared.lock().expect("audio state lock poisoned");
            shared.volume = volume.clamp(0.0, 1.0);
        }

        fn position(&self) -> Duration {
            let played_frames = {
                let shared = self.shared.lock().expect("audio state lock poisoned");
                shared.played_frames
            };
            self.base_offset
                + Duration::from_secs_f64(played_frames as f64 / f64::from(self.output_sample_rate))
        }

        fn sync_position(&self) -> Option<Duration> {
            self.ready_for_sync
                .load(Ordering::Acquire)
                .then_some(self.position())
        }
    }

    struct AudioWorkerHandle {
        stop: Arc<AtomicBool>,
    }

    impl AudioWorkerHandle {
        fn stopped() -> Self {
            Self {
                stop: Arc::new(AtomicBool::new(true)),
            }
        }

        fn stop(&self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    struct AudioChunk {
        samples: Vec<f32>,
        channels: u16,
    }

    struct AudioDecoder {
        input: ffmpeg::format::context::Input,
        decoder: ffmpeg::decoder::Audio,
        resampler: ffmpeg::software::resampling::Context,
        audio_index: usize,
        output_channels: u16,
        output_rate: u32,
        eof_sent: bool,
    }

    fn spawn_audio_worker(
        source: MediaSource,
        shared: Arc<Mutex<AudioOutputShared>>,
        ready_for_sync: Arc<AtomicBool>,
        start_offset: Duration,
        target_sample_rate: u32,
        target_channels: u16,
    ) -> AudioWorkerHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let generation = {
            let shared = shared.lock().expect("audio state lock poisoned");
            shared.generation
        };
        thread::spawn(move || {
            let mut decoder = match AudioDecoder::open(
                &source,
                start_offset,
                target_sample_rate,
                target_channels,
            ) {
                Ok(Some(decoder)) => decoder,
                Ok(None) => {
                    ready_for_sync.store(false, Ordering::Release);
                    return;
                }
                Err(error) => {
                    ready_for_sync.store(false, Ordering::Release);
                    eprintln!("tgui media audio disabled: {error}");
                    return;
                }
            };

            loop {
                let buffered = {
                    let shared = shared.lock().expect("audio state lock poisoned");
                    if shared.generation != generation {
                        return;
                    }
                    queued_audio_duration(shared.queued_frames, decoder.output_rate)
                };

                ready_for_sync.store(buffered >= AUDIO_SYNC_PREROLL, Ordering::Release);

                if worker_stop.load(Ordering::Relaxed) {
                    return;
                }

                if buffered >= AUDIO_BUFFER_AHEAD {
                    thread::sleep(Duration::from_millis(6));
                    continue;
                }

                match decoder.next_chunk(&worker_stop) {
                    Ok(Some(chunk)) => {
                        if !push_audio_chunk(&shared, generation, chunk) {
                            return;
                        }
                    }
                    Ok(None) => return,
                    Err(error) => {
                        ready_for_sync.store(false, Ordering::Release);
                        eprintln!("tgui media audio disabled: {error}");
                        return;
                    }
                }
            }
        });
        AudioWorkerHandle { stop }
    }

    impl AudioDecoder {
        fn open(
            source: &MediaSource,
            start_offset: Duration,
            target_sample_rate: u32,
            target_channels: u16,
        ) -> Result<Option<Self>, TguiError> {
            let input = open_media_input(source, "audio stream")?;

            let (audio_index, decoder) = {
                let Some(audio_stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
                    return Ok(None);
                };
                let context =
                    ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())
                        .map_err(|error| {
                            TguiError::Media(format!("failed to create audio decoder: {error}"))
                        })?;
                let decoder = context.decoder().audio().map_err(|error| {
                    TguiError::Media(format!("failed to open audio decoder: {error}"))
                })?;
                (audio_stream.index(), decoder)
            };

            let input_layout =
                normalized_channel_layout(decoder.channel_layout(), decoder.channels());
            let output_layout = ffmpeg::ChannelLayout::default(i32::from(target_channels.max(1)));
            let output_channels = output_layout.channels().max(1) as u16;
            let output_rate = target_sample_rate.max(1);
            let output_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);
            let resampler = ffmpeg::software::resampling::Context::get(
                decoder.format(),
                input_layout,
                decoder.rate().max(1),
                output_format,
                output_layout,
                output_rate,
            )
            .map_err(|error| {
                TguiError::Media(format!("failed to create audio resampler: {error}"))
            })?;

            let mut decoder_state = Self {
                input,
                decoder,
                resampler,
                audio_index,
                output_channels,
                output_rate,
                eof_sent: false,
            };

            if !start_offset.is_zero() {
                decoder_state.seek_to(start_offset)?;
            }

            Ok(Some(decoder_state))
        }

        fn seek_to(&mut self, target: Duration) -> Result<(), TguiError> {
            let timestamp = av_time_from_duration(target);
            self.input.seek(timestamp, ..).map_err(|error| {
                TguiError::Media(format!("failed to seek audio stream: {error}"))
            })?;
            self.decoder.flush();
            self.eof_sent = false;
            Ok(())
        }

        fn next_chunk(&mut self, stop: &Arc<AtomicBool>) -> Result<Option<AudioChunk>, TguiError> {
            let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
            let mut converted = ffmpeg::util::frame::audio::Audio::empty();

            loop {
                if stop.load(Ordering::Relaxed) {
                    return Ok(None);
                }

                if self.decoder.receive_frame(&mut decoded).is_ok() {
                    let samples = resample_audio_frame(
                        &mut self.resampler,
                        &decoded,
                        &mut converted,
                        self.output_channels,
                    )?;
                    if samples.is_empty() {
                        continue;
                    }
                    return Ok(Some(AudioChunk {
                        samples,
                        channels: self.output_channels,
                    }));
                }

                let next_packet = {
                    let mut packets = self.input.packets();
                    packets.next()
                };

                match next_packet {
                    Some((stream, packet)) => {
                        if stream.index() != self.audio_index {
                            continue;
                        }
                        self.decoder.send_packet(&packet).map_err(|error| {
                            TguiError::Media(format!("failed to send audio packet: {error}"))
                        })?;
                    }
                    None if !self.eof_sent => {
                        self.decoder.send_eof().map_err(|error| {
                            TguiError::Media(format!("failed to finalize audio stream: {error}"))
                        })?;
                        self.eof_sent = true;
                    }
                    None => return Ok(None),
                }
            }
        }
    }

    fn build_audio_output_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        shared: Arc<Mutex<AudioOutputShared>>,
        ready_for_sync: Arc<AtomicBool>,
    ) -> Result<Stream, TguiError> {
        let err_fn = move |error| {
            ready_for_sync.store(false, Ordering::Release);
            eprintln!("tgui media audio output error: {error}");
        };

        match sample_format {
            SampleFormat::F32 => build_audio_stream_typed::<f32>(device, config, shared, err_fn),
            SampleFormat::I16 => build_audio_stream_typed::<i16>(device, config, shared, err_fn),
            SampleFormat::U16 => build_audio_stream_typed::<u16>(device, config, shared, err_fn),
            other => Err(TguiError::Media(format!(
                "unsupported audio sample format: {other:?}"
            ))),
        }
    }

    fn build_audio_stream_typed<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        shared: Arc<Mutex<AudioOutputShared>>,
        err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
    ) -> Result<Stream, TguiError>
    where
        T: SizedSample + FromSample<f32>,
    {
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _| write_audio_output::<T>(data, &shared),
                err_fn,
                None,
            )
            .map_err(|error| TguiError::Media(format!("failed to build audio output stream: {error}")))
    }

    fn write_audio_output<T>(data: &mut [T], shared: &Arc<Mutex<AudioOutputShared>>)
    where
        T: Sample + FromSample<f32>,
    {
        let mut shared = shared.lock().expect("audio state lock poisoned");
        let output_channels = shared.output_channels.max(1) as usize;
        let decode_channels = shared.decode_channels.max(1) as usize;
        let equilibrium = T::EQUILIBRIUM;
        let volume = if shared.muted { 0.0 } else { shared.volume };

        for frame in data.chunks_mut(output_channels) {
            if shared.paused || shared.samples.len() < decode_channels {
                frame.fill(equilibrium);
                continue;
            }

            let source_left = shared.samples.pop_front().unwrap_or(0.0);
            let source_right = if decode_channels > 1 {
                shared.samples.pop_front().unwrap_or(source_left)
            } else {
                source_left
            };

            let mono = (source_left + source_right) * 0.5 * volume;
            for (channel_index, output) in frame.iter_mut().enumerate() {
                let sample = if shared.output_channels == 1 {
                    mono
                } else if decode_channels == 1 {
                    source_left * volume
                } else {
                    match channel_index {
                        0 => source_left * volume,
                        1 => source_right * volume,
                        _ => 0.0,
                    }
                };
                *output = T::from_sample(sample);
            }

            shared.queued_frames = shared.queued_frames.saturating_sub(1);
            shared.played_frames = shared.played_frames.saturating_add(1);
        }
    }

    fn push_audio_chunk(
        shared: &Arc<Mutex<AudioOutputShared>>,
        generation: u64,
        chunk: AudioChunk,
    ) -> bool {
        let mut shared = shared.lock().expect("audio state lock poisoned");
        if shared.generation != generation {
            return false;
        }
        let frame_count = chunk.samples.len() / usize::from(chunk.channels.max(1));
        shared.samples.extend(chunk.samples);
        shared.queued_frames = shared.queued_frames.saturating_add(frame_count as u64);
        true
    }

    fn queued_audio_duration(frames: u64, sample_rate: u32) -> Duration {
        if sample_rate == 0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(frames as f64 / f64::from(sample_rate))
        }
    }

    pub(super) fn run_video_session(
        source: &MediaSource,
        state: Arc<Mutex<VideoPlaybackState>>,
        invalidation: InvalidationSignal,
        stop: Arc<AtomicBool>,
        config: VideoConfig,
    ) -> Result<(), TguiError> {
        ffmpeg::init()
            .map_err(|error| TguiError::Media(format!("failed to init ffmpeg: {error}")))?;

        let mut decoder = VideoDecoder::open(source, stop.clone())?;
        let mut audio_playback = AudioPlayback::new(
            source.clone(),
            config.muted,
            config.volume,
            Duration::ZERO,
            false,
        )
        .ok();
        let initial_frame = decoder.frame_at_or_last(Duration::ZERO)?;
        let mut rebuffering = config.autoplay
            && decoder.buffered_duration() < VIDEO_BUFFER_RESUME_WATERMARK
            && !decoder.is_exhausted();
        {
            let mut guard = state.lock().expect("video state lock poisoned");
            guard.loading = rebuffering;
            guard.error = None;
            guard.intrinsic_size = IntrinsicSize::from_pixels(decoder.width, decoder.height);
            guard.duration = decoder.duration;
            guard.frame_interval = decoder.frame_interval;
            guard.paused = !config.autoplay;
            guard.looping = config.looping;
            guard.muted = config.muted;
            guard.volume = config.volume;
            guard.ended = false;
            if let Some(frame) = initial_frame {
                guard.position = frame.timestamp;
                guard.texture = Some(Arc::new(frame.texture));
            }
            guard.publish_controller_snapshot();
        }
        invalidation.mark_dirty();

        let mut anchor_position = {
            let guard = state.lock().expect("video state lock poisoned");
            guard.position
        };
        let mut anchor_instant = Instant::now();

        loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }

            let commands = drain_controller_commands(&state);
            if !commands.is_empty() {
                apply_controller_commands(
                    source,
                    &state,
                    &invalidation,
                    &mut decoder,
                    audio_playback.as_mut(),
                    &mut anchor_position,
                    &mut anchor_instant,
                    &stop,
                    &mut rebuffering,
                    commands,
                )?;
            }

            let (paused, looping) = {
                let guard = state.lock().expect("video state lock poisoned");
                (guard.paused, guard.looping)
            };

            if paused {
                if let Some(audio) = audio_playback.as_mut() {
                    audio.pause();
                }
                decoder.drain_packet_channel();
                anchor_position = {
                    let guard = state.lock().expect("video state lock poisoned");
                    guard.position
                };
                anchor_instant = Instant::now();
                thread::sleep(Duration::from_millis(12));
                continue;
            }

            if rebuffering {
                if let Some(audio) = audio_playback.as_mut() {
                    audio.pause();
                }
                decoder.fill_buffer(VIDEO_BUFFER_RESUME_WATERMARK);
                if decoder.buffered_duration() < VIDEO_BUFFER_RESUME_WATERMARK && !decoder.is_exhausted()
                {
                    set_video_loading_state(&state, &invalidation, true);
                    thread::sleep(Duration::from_millis(12));
                    continue;
                }
                rebuffering = false;
                anchor_position = {
                    let guard = state.lock().expect("video state lock poisoned");
                    guard.position
                };
                anchor_instant = Instant::now();
                set_video_loading_state(&state, &invalidation, false);
            } else {
                decoder.fill_buffer(VIDEO_BUFFER_LOW_WATERMARK);
                if decoder.buffered_duration() < VIDEO_BUFFER_LOW_WATERMARK && !decoder.is_exhausted()
                {
                    rebuffering = true;
                    set_video_loading_state(&state, &invalidation, true);
                    if let Some(audio) = audio_playback.as_mut() {
                        audio.pause();
                    }
                    thread::sleep(Duration::from_millis(12));
                    continue;
                }
            }

            if let Some(audio) = audio_playback.as_mut() {
                audio.resume();
            }

            let Some(frame) = decoder.next_frame()? else {
                if looping {
                    let loop_frame = restart_from_position(
                        source,
                        Duration::ZERO,
                        true,
                        false,
                        &state,
                        &invalidation,
                        &mut decoder,
                        audio_playback.as_mut(),
                        &mut anchor_position,
                        &mut anchor_instant,
                        &stop,
                        &mut rebuffering,
                    )?;
                    anchor_position = loop_frame;
                    continue;
                }

                let mut guard = state.lock().expect("video state lock poisoned");
                guard.paused = true;
                guard.ended = true;
                if let Some(duration) = decoder.duration {
                    guard.position = duration;
                }
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
                return Ok(());
            };

            match wait_until_frame(
                frame.timestamp,
                anchor_position,
                anchor_instant,
                decoder.frame_interval,
                audio_playback.as_ref(),
                &state,
                &stop,
            ) {
                FrameSyncAction::Retry | FrameSyncAction::Drop => continue,
                FrameSyncAction::Render => {}
            }

            {
                let mut guard = state.lock().expect("video state lock poisoned");
                if guard.paused {
                    continue;
                }
                guard.texture = Some(Arc::new(frame.texture));
                guard.position = frame.timestamp;
                guard.ended = false;
                guard.error = None;
                guard.loading = false;
                guard.publish_controller_snapshot();
            }
            invalidation.mark_dirty();
        }
    }

    fn drain_controller_commands(
        state: &Arc<Mutex<VideoPlaybackState>>,
    ) -> Vec<VideoControlCommand> {
        let controller = state
            .lock()
            .expect("video state lock poisoned")
            .controller
            .clone();
        controller
            .map(|controller| controller.take_commands())
            .unwrap_or_default()
    }

    fn apply_controller_commands(
        source: &MediaSource,
        state: &Arc<Mutex<VideoPlaybackState>>,
        invalidation: &InvalidationSignal,
        decoder: &mut VideoDecoder,
        audio_playback: Option<&mut AudioPlayback>,
        anchor_position: &mut Duration,
        anchor_instant: &mut Instant,
        stop: &Arc<AtomicBool>,
        rebuffering: &mut bool,
        commands: Vec<VideoControlCommand>,
    ) -> Result<(), TguiError> {
        let mut audio_playback = audio_playback;
        for command in commands {
            match command {
                VideoControlCommand::Play => {
                    let should_restart = {
                        let guard = state.lock().expect("video state lock poisoned");
                        guard.ended
                    };
                    if should_restart {
                        restart_from_position(
                            source,
                            Duration::ZERO,
                            true,
                            false,
                            state,
                            invalidation,
                            decoder,
                            audio_playback.as_deref_mut(),
                            anchor_position,
                            anchor_instant,
                            stop,
                            rebuffering,
                        )?;
                    } else {
                        let mut guard = state.lock().expect("video state lock poisoned");
                        if guard.error.is_some() {
                            continue;
                        }
                        guard.paused = false;
                        guard.ended = false;
                        *anchor_position = guard.position;
                        *anchor_instant = Instant::now();
                        guard.publish_controller_snapshot();
                        invalidation.mark_dirty();
                    }
                }
                VideoControlCommand::Pause => {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    guard.paused = true;
                    guard.publish_controller_snapshot();
                    invalidation.mark_dirty();
                }
                VideoControlCommand::Resume => {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    if guard.error.is_some() || guard.ended {
                        continue;
                    }
                    guard.paused = false;
                    guard.publish_controller_snapshot();
                    *anchor_position = guard.position;
                    *anchor_instant = Instant::now();
                    invalidation.mark_dirty();
                }
                VideoControlCommand::Replay => {
                    restart_from_position(
                        source,
                        Duration::ZERO,
                        true,
                        false,
                        state,
                        invalidation,
                        decoder,
                        audio_playback.as_deref_mut(),
                        anchor_position,
                        anchor_instant,
                        stop,
                        rebuffering,
                    )?;
                }
                VideoControlCommand::SeekTime(target) => {
                    let should_play = {
                        let guard = state.lock().expect("video state lock poisoned");
                        !guard.paused && guard.error.is_none()
                    };
                    restart_from_position(
                        source,
                        target,
                        should_play,
                        true,
                        state,
                        invalidation,
                        decoder,
                        audio_playback.as_deref_mut(),
                        anchor_position,
                        anchor_instant,
                        stop,
                        rebuffering,
                    )?;
                }
                VideoControlCommand::SetMuted(muted) => {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    guard.muted = muted;
                    guard.publish_controller_snapshot();
                    if let Some(audio) = audio_playback.as_deref_mut() {
                        audio.set_muted(muted);
                    }
                    invalidation.mark_dirty();
                }
                VideoControlCommand::SetVolume(volume) => {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    guard.volume = volume;
                    guard.publish_controller_snapshot();
                    if let Some(audio) = audio_playback.as_deref_mut() {
                        audio.set_volume(volume);
                    }
                    invalidation.mark_dirty();
                }
                VideoControlCommand::SetLooping(looping) => {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    guard.looping = looping;
                    guard.publish_controller_snapshot();
                    invalidation.mark_dirty();
                }
            }
        }
        Ok(())
    }

    fn restart_from_position(
        source: &MediaSource,
        target: Duration,
        playing: bool,
        emit_seek: bool,
        state: &Arc<Mutex<VideoPlaybackState>>,
        invalidation: &InvalidationSignal,
        decoder: &mut VideoDecoder,
        audio_playback: Option<&mut AudioPlayback>,
        anchor_position: &mut Duration,
        anchor_instant: &mut Instant,
        stop: &Arc<AtomicBool>,
        rebuffering: &mut bool,
    ) -> Result<Duration, TguiError> {
        let target = {
            let guard = state.lock().expect("video state lock poisoned");
            guard
                .duration
                .map(|duration| target.min(duration))
                .unwrap_or(target)
        };
        *decoder = VideoDecoder::open_at(source, target, stop.clone())?;
        let frame = decoder.frame_at_or_last(target)?;
        *rebuffering = playing
            && decoder.buffered_duration() < VIDEO_BUFFER_RESUME_WATERMARK
            && !decoder.is_exhausted();
        let mut resolved_position = Duration::ZERO;
        {
            let mut guard = state.lock().expect("video state lock poisoned");
            guard.loading = *rebuffering;
            guard.error = None;
            guard.intrinsic_size = IntrinsicSize::from_pixels(decoder.width, decoder.height);
            guard.duration = decoder.duration;
            guard.frame_interval = decoder.frame_interval;
            guard.paused = !playing;
            guard.ended = false;
            if emit_seek {
                guard.seek_generation = guard.seek_generation.saturating_add(1);
            }
            if let Some(frame) = frame {
                resolved_position = frame.timestamp;
                guard.position = frame.timestamp;
                guard.texture = Some(Arc::new(frame.texture));
            } else {
                guard.position = Duration::ZERO;
            }
            guard.publish_controller_snapshot();
        }
        if let Some(audio) = audio_playback {
            audio.seek(resolved_position, playing && !*rebuffering)?;
        }
        *anchor_position = resolved_position;
        *anchor_instant = Instant::now();
        invalidation.mark_dirty();
        Ok(resolved_position)
    }

    fn resample_audio_frame(
        resampler: &mut ffmpeg::software::resampling::Context,
        decoded: &ffmpeg::util::frame::audio::Audio,
        converted: &mut ffmpeg::util::frame::audio::Audio,
        output_channels: u16,
    ) -> Result<Vec<f32>, TguiError> {
        *converted = ffmpeg::util::frame::audio::Audio::empty();
        resampler.run(decoded, converted).map_err(|error| {
            TguiError::Media(format!("failed to resample audio frame: {error}"))
        })?;
        Ok(samples_from_audio_frame(converted, output_channels))
    }

    fn samples_from_audio_frame(
        frame: &ffmpeg::util::frame::audio::Audio,
        output_channels: u16,
    ) -> Vec<f32> {
        let frame_samples = frame.samples();
        if frame_samples == 0 {
            return Vec::new();
        }

        let sample_count = frame_samples * output_channels as usize;
        let byte_count = sample_count * std::mem::size_of::<f32>();
        let bytes = frame.data(0);
        if bytes.len() < byte_count {
            return Vec::new();
        }
        cast_slice::<u8, f32>(&bytes[..byte_count]).to_vec()
    }

    fn normalized_channel_layout(
        layout: ffmpeg::ChannelLayout,
        channels: u16,
    ) -> ffmpeg::ChannelLayout {
        if layout.is_empty() {
            ffmpeg::ChannelLayout::default(i32::from(channels.max(1)))
        } else {
            layout
        }
    }

    fn source_path(source: &MediaSource) -> String {
        match source {
            MediaSource::Path(path) => path.to_string_lossy().into_owned(),
            MediaSource::Url(url) => url.clone(),
        }
    }

    struct VideoDecoder {
        packet_rx: Receiver<VideoPacketMessage>,
        packet_queue: VecDeque<QueuedVideoPacket>,
        decoder: ffmpeg::decoder::Video,
        scaler: ffmpeg::software::scaling::Context,
        time_base: ffmpeg::Rational,
        duration: Option<Duration>,
        frame_interval: Duration,
        width: u32,
        height: u32,
        eof_sent: bool,
        input_exhausted: bool,
        io_worker: VideoPacketReaderHandle,
        session_stop: Arc<AtomicBool>,
    }

    struct DecodedVideoFrame {
        timestamp: Duration,
        texture: TextureFrame,
    }

    struct QueuedVideoPacket {
        packet: ffmpeg::Packet,
        timestamp: Option<Duration>,
        duration: Duration,
        is_key: bool,
    }

    enum VideoPacketMessage {
        Packet(QueuedVideoPacket),
        EndOfStream,
    }

    impl VideoDecoder {
        fn open(source: &MediaSource, stop: Arc<AtomicBool>) -> Result<Self, TguiError> {
            Self::open_at(source, Duration::ZERO, stop)
        }

        fn open_at(
            source: &MediaSource,
            start_offset: Duration,
            stop: Arc<AtomicBool>,
        ) -> Result<Self, TguiError> {
            let mut input = open_media_input(source, "video source")?;
            let (video_index, time_base, duration, frame_interval, parameters) = {
                let video_stream = input
                    .streams()
                    .best(ffmpeg::media::Type::Video)
                    .ok_or_else(|| {
                        TguiError::Media("video source has no video stream".to_string())
                    })?;
                let time_base = video_stream.time_base();
                let duration = (video_stream.duration() > 0)
                    .then_some(duration_from_pts(video_stream.duration(), time_base));
                let frame_interval = frame_interval_from_rate(video_stream.avg_frame_rate())
                    .or_else(|| frame_interval_from_rate(video_stream.rate()))
                    .unwrap_or(super::DEFAULT_VIDEO_FRAME_INTERVAL);
                (
                    video_stream.index(),
                    time_base,
                    duration,
                    frame_interval,
                    video_stream.parameters(),
                )
            };
            if !start_offset.is_zero() {
                let timestamp = av_time_from_duration(start_offset);
                input.seek(timestamp, ..).map_err(|error| {
                    TguiError::Media(format!("failed to seek video stream: {error}"))
                })?;
            }
            let (decoder, scaler) = create_video_decoder(&parameters)?;

            let width = decoder.width();
            let height = decoder.height();
            let (packet_tx, packet_rx) = bounded(VIDEO_PACKET_CHANNEL_CAPACITY);
            let io_worker = spawn_video_packet_reader(
                input,
                video_index,
                time_base,
                frame_interval,
                packet_tx,
                stop.clone(),
            );

            Ok(Self {
                packet_rx,
                packet_queue: VecDeque::with_capacity(VIDEO_PACKET_CHANNEL_CAPACITY),
                decoder,
                scaler,
                time_base,
                duration,
                frame_interval,
                width,
                height,
                eof_sent: false,
                input_exhausted: false,
                io_worker,
                session_stop: stop,
            })
        }

        fn next_frame(&mut self) -> Result<Option<DecodedVideoFrame>, TguiError> {
            let mut decoded = ffmpeg::util::frame::video::Video::empty();
            let mut rgba = ffmpeg::util::frame::video::Video::empty();

            loop {
                if self.session_stop.load(Ordering::Relaxed) {
                    return Ok(None);
                }

                if self.decoder.receive_frame(&mut decoded).is_ok() {
                    self.scaler.run(&decoded, &mut rgba).map_err(|error| {
                        TguiError::Media(format!("failed to scale video frame: {error}"))
                    })?;
                    let timestamp = decoded
                        .timestamp()
                        .map(|value| duration_from_pts(value, self.time_base))
                        .or(self.duration)
                        .unwrap_or(Duration::ZERO);
                    return Ok(Some(DecodedVideoFrame {
                        timestamp,
                        texture: rgba_frame_to_texture(&rgba),
                    }));
                }

                self.drain_packet_channel();
                self.trim_packet_backlog();

                if let Some(queued) = self.packet_queue.pop_front() {
                    self.decoder.send_packet(&queued.packet).map_err(|error| {
                        TguiError::Media(format!("failed to send packet: {error}"))
                    })?;
                    continue;
                }

                if self.input_exhausted {
                    if !self.eof_sent {
                        self.decoder.send_eof().map_err(|error| {
                            TguiError::Media(format!("failed to finalize video stream: {error}"))
                        })?;
                        self.eof_sent = true;
                        continue;
                    }
                    return Ok(None);
                }

                match self.packet_rx.recv_timeout(BUFFER_WAIT_SLICE) {
                    Ok(message) => self.handle_packet_message(message),
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => self.input_exhausted = true,
                }
            }
        }

        fn drain_packet_channel(&mut self) {
            loop {
                match self.packet_rx.try_recv() {
                    Ok(message) => self.handle_packet_message(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.input_exhausted = true;
                        break;
                    }
                }
            }
        }

        fn fill_buffer(&mut self, target: Duration) {
            self.drain_packet_channel();
            while self.buffered_duration() < target && !self.input_exhausted {
                if self.session_stop.load(Ordering::Relaxed) {
                    return;
                }
                match self.packet_rx.recv_timeout(BUFFER_WAIT_SLICE) {
                    Ok(message) => self.handle_packet_message(message),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.input_exhausted = true;
                        break;
                    }
                }
            }
        }

        fn buffered_duration(&self) -> Duration {
            buffered_packet_duration(&self.packet_queue, self.frame_interval)
        }

        fn is_exhausted(&self) -> bool {
            self.input_exhausted
        }

        fn frame_at_or_last(&mut self, target: Duration) -> Result<Option<DecodedVideoFrame>, TguiError> {
            let mut last_frame = None;
            while let Some(frame) = self.next_frame()? {
                if frame.timestamp >= target {
                    return Ok(Some(frame));
                }
                last_frame = Some(frame);
            }
            Ok(last_frame)
        }

        fn handle_packet_message(&mut self, message: VideoPacketMessage) {
            match message {
                VideoPacketMessage::Packet(packet) => self.packet_queue.push_back(packet),
                VideoPacketMessage::EndOfStream => self.input_exhausted = true,
            }
        }

        fn trim_packet_backlog(&mut self) {
            if self.buffered_duration() < VIDEO_DROP_GOP_WATERMARK {
                return;
            }

            let Some(next_key_index) = self
                .packet_queue
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(index, packet)| packet.is_key.then_some(index))
            else {
                return;
            };

            self.packet_queue.drain(..next_key_index);
            self.decoder.flush();
            self.eof_sent = false;
        }
    }

    impl Drop for VideoDecoder {
        fn drop(&mut self) {
            self.io_worker.stop();
        }
    }

    impl QueuedVideoPacket {
        fn new(
            packet: ffmpeg::Packet,
            time_base: ffmpeg::Rational,
            frame_interval: Duration,
        ) -> Self {
            let timestamp = packet
                .pts()
                .or_else(|| packet.dts())
                .map(|value| duration_from_pts(value, time_base));
            let duration = match packet.duration() {
                value if value > 0 => duration_from_pts(value, time_base),
                _ => frame_interval,
            };

            Self {
                is_key: packet.is_key(),
                packet,
                timestamp,
                duration: if duration.is_zero() { frame_interval } else { duration },
            }
        }
    }

    fn rgba_frame_to_texture(frame: &ffmpeg::util::frame::video::Video) -> TextureFrame {
        let width = frame.width();
        let height = frame.height();
        let stride = frame.stride(0);
        let data = frame.data(0);
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        for row in 0..height as usize {
            let src_start = row * stride;
            let src_end = src_start + width as usize * 4;
            let dst_start = row * width as usize * 4;
            let dst_end = dst_start + width as usize * 4;
            pixels[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
        }
        TextureFrame::new(width, height, pixels)
    }

    enum FrameSyncAction {
        Render,
        Drop,
        Retry,
    }

    fn wait_until_frame(
        timestamp: Duration,
        anchor_position: Duration,
        anchor_instant: Instant,
        frame_interval: Duration,
        audio_playback: Option<&AudioPlayback>,
        state: &Arc<Mutex<VideoPlaybackState>>,
        stop: &Arc<AtomicBool>,
    ) -> FrameSyncAction {
        let wall_clock_target = if timestamp > anchor_position {
            anchor_instant + (timestamp - anchor_position)
        } else {
            Instant::now()
        };
        let late_threshold = VIDEO_LATE_FRAME_THRESHOLD.max(frame_interval.saturating_mul(4));

        loop {
            if stop.load(Ordering::Relaxed) {
                return FrameSyncAction::Retry;
            }
            {
                let guard = state.lock().expect("video state lock poisoned");
                if guard.paused {
                    return FrameSyncAction::Retry;
                }
            }

            if let Some(audio) = audio_playback {
                let Some(position) = audio.sync_position() else {
                    let now = Instant::now();
                    if now >= wall_clock_target {
                        return if now.duration_since(wall_clock_target) > late_threshold {
                            FrameSyncAction::Drop
                        } else {
                            FrameSyncAction::Render
                        };
                    }
                    thread::sleep((wall_clock_target - now).min(Duration::from_millis(8)));
                    continue;
                };
                if position > timestamp && position - timestamp > late_threshold {
                    return FrameSyncAction::Drop;
                }
                if position + Duration::from_millis(4) >= timestamp {
                    return FrameSyncAction::Render;
                }
                let sleep_for = timestamp
                    .saturating_sub(position)
                    .min(Duration::from_millis(8));
                thread::sleep(sleep_for.max(Duration::from_millis(1)));
                continue;
            }

            let now = Instant::now();
            if now >= wall_clock_target {
                return if now.duration_since(wall_clock_target) > late_threshold {
                    FrameSyncAction::Drop
                } else {
                    FrameSyncAction::Render
                };
            }
            thread::sleep((wall_clock_target - now).min(Duration::from_millis(8)));
        }
    }

    fn duration_from_pts(value: i64, time_base: ffmpeg::Rational) -> Duration {
        let seconds = (value as f64)
            * (f64::from(time_base.numerator()) / f64::from(time_base.denominator()));
        Duration::from_secs_f64(seconds.max(0.0))
    }

    fn frame_interval_from_rate(rate: ffmpeg::Rational) -> Option<Duration> {
        let numerator = rate.numerator();
        let denominator = rate.denominator();
        if numerator <= 0 || denominator <= 0 {
            return None;
        }

        let fps = f64::from(numerator) / f64::from(denominator);
        if !fps.is_finite() || fps <= 0.0 {
            return None;
        }

        Some(Duration::from_secs_f64(
            (1.0 / fps).clamp(1.0 / 240.0, 0.25),
        ))
    }

    fn av_time_from_duration(value: Duration) -> i64 {
        (value.as_secs_f64() * f64::from(ffmpeg::ffi::AV_TIME_BASE)).round() as i64
    }

    fn set_video_loading_state(
        state: &Arc<Mutex<VideoPlaybackState>>,
        invalidation: &InvalidationSignal,
        loading: bool,
    ) {
        let mut guard = state.lock().expect("video state lock poisoned");
        if guard.loading == loading {
            return;
        }
        guard.loading = loading;
        guard.publish_controller_snapshot();
        invalidation.mark_dirty();
    }

    fn open_media_input(
        source: &MediaSource,
        label: &str,
    ) -> Result<ffmpeg::format::context::Input, TguiError> {
        let source_path = source_path(source);
        ffmpeg::format::input_with_dictionary(&source_path, input_options(source)).map_err(|error| {
            TguiError::Media(format!("failed to open {label} {source_path}: {error}"))
        })
    }

    fn input_options(source: &MediaSource) -> ffmpeg::Dictionary<'static> {
        let mut options = ffmpeg::Dictionary::new();
        options.set("probesize", &(STREAM_PROBE_SIZE * 2).to_string());
        options.set(
            "analyzeduration",
            &duration_to_micros(STREAM_ANALYZE_DURATION.saturating_mul(2)),
        );

        if matches!(source, MediaSource::Url(_)) {
            options.set("rw_timeout", &duration_to_micros(NETWORK_RW_TIMEOUT));
            options.set("timeout", &duration_to_micros(NETWORK_OPEN_TIMEOUT));
            options.set("buffer_size", &NETWORK_BUFFER_SIZE.to_string());
            options.set("fifo_size", &NETWORK_FIFO_SIZE.to_string());
            options.set("reconnect", "1");
            options.set("reconnect_streamed", "1");
            options.set("reconnect_on_network_error", "1");
            options.set("reconnect_on_http_error", "4xx,5xx");
            options.set("reconnect_delay_max", "2");
        }

        options
    }

    fn duration_to_micros(value: Duration) -> String {
        value.as_micros().max(1).to_string()
    }

    fn create_video_decoder(
        parameters: &ffmpeg::codec::Parameters,
    ) -> Result<
        (
            ffmpeg::decoder::Video,
            ffmpeg::software::scaling::Context,
        ),
        TguiError,
    > {
        if should_try_cuda_decoder() {
            for decoder_name in preferred_cuda_decoders(parameters.id()) {
                let Some(codec) = ffmpeg::codec::decoder::find_by_name(decoder_name) else {
                    continue;
                };
                let context =
                    ffmpeg::codec::context::Context::from_parameters(parameters.clone()).map_err(
                        |error| TguiError::Media(format!("failed to create video decoder: {error}")),
                    )?;
                let mut decoder_builder = context.decoder();
                configure_decoder_tolerance(&mut decoder_builder);
                if let Ok(opened) = decoder_builder
                    .open_as(codec)
                    .and_then(|opened| opened.video())
                {
                    if let Ok(scaler) = create_video_scaler(&opened) {
                        return Ok((opened, scaler));
                    }
                }
            }
        }

        let context =
            ffmpeg::codec::context::Context::from_parameters(parameters.clone()).map_err(|error| {
                TguiError::Media(format!("failed to create video decoder: {error}"))
            })?;
        let mut decoder_builder = context.decoder();
        configure_decoder_tolerance(&mut decoder_builder);
        let decoder = decoder_builder.video().map_err(|error| {
            TguiError::Media(format!("failed to open video decoder: {error}"))
        })?;
        let scaler = create_video_scaler(&decoder)?;
        Ok((decoder, scaler))
    }

    fn configure_decoder_tolerance(decoder: &mut ffmpeg::decoder::Decoder) {
        decoder.conceal(
            ffmpeg::decoder::Conceal::GUESS_MVS
                | ffmpeg::decoder::Conceal::DEBLOCK
                | ffmpeg::decoder::Conceal::FAVOR_INTER,
        );
        decoder.check(ffmpeg::decoder::Check::IGNORE_ERROR);
    }

    fn create_video_scaler(
        decoder: &ffmpeg::decoder::Video,
    ) -> Result<ffmpeg::software::scaling::Context, TguiError> {
        ffmpeg::software::scaling::Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg::format::Pixel::RGBA,
            decoder.width(),
            decoder.height(),
            ffmpeg::software::scaling::flag::Flags::BILINEAR,
        )
        .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))
    }

    fn should_try_cuda_decoder() -> bool {
        std::env::var("TGUI_VIDEO_DISABLE_HWACCEL")
            .map(|value| {
                let value = value.trim();
                !(value == "1" || value.eq_ignore_ascii_case("true"))
            })
            .unwrap_or(true)
    }

    fn preferred_cuda_decoders(codec_id: ffmpeg::codec::Id) -> &'static [&'static str] {
        match codec_id {
            ffmpeg::codec::Id::H264 => &["h264_cuvid"],
            ffmpeg::codec::Id::HEVC => &["hevc_cuvid"],
            ffmpeg::codec::Id::MPEG2VIDEO => &["mpeg2_cuvid"],
            ffmpeg::codec::Id::VP8 => &["vp8_cuvid"],
            ffmpeg::codec::Id::VP9 => &["vp9_cuvid"],
            ffmpeg::codec::Id::AV1 => &["av1_cuvid"],
            _ => &[],
        }
    }

    fn spawn_video_packet_reader(
        mut input: ffmpeg::format::context::Input,
        video_index: usize,
        time_base: ffmpeg::Rational,
        frame_interval: Duration,
        sender: Sender<VideoPacketMessage>,
        session_stop: Arc<AtomicBool>,
    ) -> VideoPacketReaderHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = stop.clone();
        thread::spawn(move || {
            let mut packets = input.packets();
            loop {
                if session_stop.load(Ordering::Relaxed) || reader_stop.load(Ordering::Relaxed) {
                    return;
                }

                match packets.next() {
                    Some((stream, packet)) => {
                        if stream.index() != video_index {
                            continue;
                        }
                        if !send_video_packet_message(
                            &sender,
                            VideoPacketMessage::Packet(QueuedVideoPacket::new(
                                packet,
                                time_base,
                                frame_interval,
                            )),
                            &session_stop,
                            &reader_stop,
                        ) {
                            return;
                        }
                    }
                    None => {
                        let _ = send_video_packet_message(
                            &sender,
                            VideoPacketMessage::EndOfStream,
                            &session_stop,
                            &reader_stop,
                        );
                        return;
                    }
                }
            }
        });
        VideoPacketReaderHandle { stop }
    }

    fn send_video_packet_message(
        sender: &Sender<VideoPacketMessage>,
        mut message: VideoPacketMessage,
        session_stop: &Arc<AtomicBool>,
        reader_stop: &Arc<AtomicBool>,
    ) -> bool {
        loop {
            if session_stop.load(Ordering::Relaxed) || reader_stop.load(Ordering::Relaxed) {
                return false;
            }

            match sender.try_send(message) {
                Ok(_) => return true,
                Err(TrySendError::Full(returned)) => {
                    message = returned;
                    thread::sleep(Duration::from_millis(4));
                }
                Err(TrySendError::Disconnected(_)) => return false,
            }
        }
    }

    struct VideoPacketReaderHandle {
        stop: Arc<AtomicBool>,
    }

    impl VideoPacketReaderHandle {
        fn stop(&self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    fn buffered_packet_duration(
        packets: &VecDeque<QueuedVideoPacket>,
        frame_interval: Duration,
    ) -> Duration {
        let Some(front) = packets.front() else {
            return Duration::ZERO;
        };
        let Some(back) = packets.back() else {
            return front.duration;
        };

        if let (Some(start), Some(end)) = (front.timestamp, back.timestamp) {
            if end >= start {
                return (end - start).saturating_add(back.duration.max(frame_interval));
            }
        }

        let total = packets
            .iter()
            .fold(Duration::ZERO, |acc, packet| acc.saturating_add(packet.duration));
        if total.is_zero() {
            frame_interval.saturating_mul(packets.len() as u32)
        } else {
            total
        }
    }
}

pub(crate) fn media_placeholder_color(loading: bool, error: bool) -> Color {
    if error {
        Color::hexa(0x7F1D1DFF)
    } else if loading {
        Color::hexa(0x1F2937FF)
    } else {
        Color::hexa(0x111827FF)
    }
}

pub(crate) fn media_placeholder_label(loading: bool, error: Option<&str>, kind: &str) -> String {
    if loading {
        format!("Loading {kind}...")
    } else if let Some(error) = error {
        let compact = error.lines().next().unwrap_or(error);
        format!("{kind} unavailable: {compact}")
    } else {
        format!("No {kind}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn playback_snapshot_reports_ready_paused_playing_and_ended() {
        let controller = VideoControllerHandle::new(InvalidationSignal::new());
        let mut state = VideoPlaybackState::new(VideoConfig::default(), Some(controller.clone()));
        state.loading = false;
        state.duration = Some(Duration::from_secs(10));
        state.publish_controller_snapshot();
        assert_eq!(controller.snapshot().status, VideoPlaybackStatus::Ready);

        state.paused = false;
        state.publish_controller_snapshot();
        assert_eq!(controller.snapshot().status, VideoPlaybackStatus::Playing);

        state.paused = true;
        state.position = Duration::from_secs(4);
        state.publish_controller_snapshot();
        let paused = controller.snapshot();
        assert_eq!(paused.status, VideoPlaybackStatus::Paused);
        assert_eq!(paused.progress, 0.4);

        state.ended = true;
        state.publish_controller_snapshot();
        assert_eq!(controller.snapshot().status, VideoPlaybackStatus::Ended);
    }

    #[test]
    fn paused_seek_position_is_not_treated_as_ready() {
        let controller = VideoControllerHandle::new(InvalidationSignal::new());
        let mut state = VideoPlaybackState::new(VideoConfig::default(), Some(controller.clone()));
        state.loading = false;
        state.duration = Some(Duration::from_secs(20));
        state.position = Duration::from_secs(10);
        state.paused = true;
        state.publish_controller_snapshot();

        let snapshot = controller.snapshot();
        assert_eq!(snapshot.status, VideoPlaybackStatus::Paused);
        assert_eq!(snapshot.progress, 0.5);
    }

    #[test]
    fn publishing_new_state_resets_controller_snapshot_for_new_source() {
        let controller = VideoControllerHandle::new(InvalidationSignal::new());
        controller.publish_snapshot(VideoPlaybackSnapshot {
            status: VideoPlaybackStatus::Paused,
            position: Duration::from_secs(6),
            duration: Some(Duration::from_secs(12)),
            progress: 0.5,
            muted: false,
            volume: 1.0,
            looping: false,
        });

        let state = VideoPlaybackState::new(
            VideoConfig {
                autoplay: true,
                looping: true,
                muted: true,
                volume: 0.4,
            },
            Some(controller.clone()),
        );
        state.publish_controller_snapshot();

        assert_eq!(
            controller.snapshot(),
            VideoPlaybackSnapshot {
                status: VideoPlaybackStatus::Loading,
                position: Duration::ZERO,
                duration: None,
                progress: 0.0,
                muted: true,
                volume: 0.4,
                looping: true,
            }
        );
    }

    #[test]
    fn next_frame_deadline_uses_state_frame_interval() {
        let mut state = VideoPlaybackState::new(VideoConfig::default(), None);
        state.paused = false;
        state.frame_interval = Duration::from_millis(16);

        let now = Instant::now();
        assert_eq!(
            state.next_frame_deadline(now),
            Some(now + Duration::from_millis(16))
        );
    }

    #[cfg(not(all(
        feature = "video-ffmpeg",
        any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        )
    )))]
    #[test]
    fn unsupported_backend_surfaces_controller_error() {
        let invalidation = InvalidationSignal::new();
        let media = MediaManager::new(invalidation.clone());
        let controller = VideoControllerHandle::new(invalidation);
        let widget_id = WidgetId::next();
        let source = MediaSource::url("https://example.com/demo.mp4");

        let _ = media.video_snapshot(
            widget_id,
            &source,
            VideoConfig::default(),
            Some(&controller),
        );

        for _ in 0..50 {
            if matches!(controller.snapshot().status, VideoPlaybackStatus::Error(_)) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("expected unsupported backend to publish a controller error");
    }
}
