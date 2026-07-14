use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{bounded, unbounded, Sender};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScalingFlags};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::video::Video as VideoFrame;
use ffmpeg_next as ffmpeg;
use parking_lot::Mutex;

#[cfg(test)]
use crate::audio::backend::shared::ffmpeg_http_input_options;
use crate::audio::backend::shared::{
    create_temporary_media_file, ensure_ffmpeg_initialized,
    flush_audio_resampler_with_pending_and_buffer as flush_audio_resampler, media_path_to_url,
    normalize_playback_rate, open_ffmpeg_input,
    receive_audio_frames_with_pending_and_buffer as receive_audio_frames, validate_ffmpeg_headers,
    ReusableAudioFrame, SharedAudioClock, TemporaryMediaFile,
};
use crate::foundation::error::TguiError;
use crate::foundation::threading::join_with_timeout;
use crate::media::{IntrinsicSize, RasterRequest, TextureFrame};
use crate::video::{
    VideoAudioTrack, VideoAudioTrackSelection, VideoPlaybackState, VideoSize, VideoSource,
    VideoSubtitleBitmapCue, VideoSubtitleCue, VideoSubtitleCuePlacement, VideoSubtitleCueStyle,
    VideoSubtitleTrack, VideoSubtitleTrackSelection, VideoSurfaceSnapshot,
};

use super::{
    BackendSharedState, VideoBackend, VideoRenderFrame, VideoYuvColorMatrix, VideoYuvColorRange,
    VideoYuvColorSpace, VideoYuvFormat, VideoYuvFrame, VideoYuvPlane, VideoYuvPlaneFormat,
};

mod decode;
mod helpers;
mod present;
mod queue;

#[cfg(feature = "bench-support")]
pub(crate) mod bench_support;

use decode::decode_main;
use helpers::*;
use present::present_main;
use queue::*;

// 后台控制线程在空闲态下轮询控制命令的时间间隔。
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);

// 本地文件模式下，视频队列的软上限。
const LOCAL_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(3);
// 本地文件模式下，视频队列的硬上限。
const LOCAL_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(4);
// 本地文件模式下，视频队列允许保留的最大帧数保险丝。
const LOCAL_VIDEO_MAX_PACKET_COUNT: usize = 120;
const LOCAL_READY_VIDEO_FRAME_COUNT: usize = 4;
// 本地文件模式下，音频缓冲的软上限。
const LOCAL_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(1500);
// 本地文件模式下，音频缓冲的硬上限。
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);

// 网络流模式下，视频队列的软上限。
const NETWORK_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(5);
// 网络流模式下，视频队列的硬上限。
const NETWORK_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(6);
// 网络流模式下，视频队列允许保留的最大帧数保险丝。
const NETWORK_VIDEO_MAX_PACKET_COUNT: usize = 300;
const NETWORK_READY_VIDEO_FRAME_COUNT: usize = 8;
// 网络流模式下，音频缓冲的软上限。
const NETWORK_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(4000);
// 网络流模式下，音频缓冲的硬上限。
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);

// 本地文件首次点击播放时，音频至少要攒到多深才开始真正输出。
const LOCAL_START_BUFFER_TARGET: Duration = Duration::from_millis(1000);
// 本地文件从 Buffering 恢复播放时，音频至少要恢复到多深。
const LOCAL_REBUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时的音频启动门槛。
const NETWORK_START_BUFFER_TARGET: Duration = Duration::from_millis(2500);
// 网络流从 Buffering 恢复播放时的音频门槛。
const NETWORK_REBUFFER_TARGET: Duration = Duration::from_millis(2000);

// 本地文件首次播放时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_millis(1500);
// 本地文件从 Buffering 恢复时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 网络流从 Buffering 恢复时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 播放中如果视频前置缓存低于这个值，就主动暂停进入 Buffering。
const VIDEO_REBUFFER_ENTER_THRESHOLD: Duration = Duration::from_secs(2);
// Seek 后丢弃“明显还在目标位置之前”的视频帧时允许的误差。
const VIDEO_SEEK_PREROLL_TOLERANCE: Duration = Duration::from_millis(50);

// 本地文件模式下，音频缓冲低于这个值就认为快饿死了，需要进入 Buffering。
const LOCAL_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(120);
// 网络流模式下，音频进入“危险区”的阈值。
const NETWORK_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(250);

// 视频帧显示判定的时间容差。
const VIDEO_PRESENT_TOLERANCE: Duration = Duration::from_millis(8);
// 当 demux 因缓存已满而暂时空转时，后台线程每次 sleep 的时长。
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);
const SHUTDOWN_JOIN_TIMEOUT: Duration = Duration::from_millis(100);

static VIDEO_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

pub(super) fn video_debug_enabled() -> bool {
    *VIDEO_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("TGUI_VIDEO_DEBUG")
            .map(|value| {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("yes")
                    || value.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
    })
}

pub(crate) struct FfmpegVideoBackend {
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<VideoRenderFrame>>>,
    runtime: Mutex<VideoBackendRuntime>,
}

struct VideoBackendRuntime {
    workers: Option<VideoWorkerHandles>,
    target_raster: Option<RasterRequest>,
}

struct VideoWorkerHandles {
    command_tx: Sender<BackendCommand>,
    decode_tx: Sender<DecodeCommand>,
    present_worker: JoinHandle<()>,
    decode_worker: JoinHandle<()>,
}

impl FfmpegVideoBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        Self {
            shared,
            latest_frame: Arc::new(Mutex::new(None)),
            runtime: Mutex::new(VideoBackendRuntime {
                workers: None,
                target_raster: None,
            }),
        }
    }

    fn ensure_workers(&self) -> Result<Sender<BackendCommand>, TguiError> {
        let mut runtime = self.runtime.lock();
        if let Some(workers) = runtime.workers.as_ref() {
            return Ok(workers.command_tx.clone());
        }

        ensure_ffmpeg_initialized()?;

        let (backend_tx, backend_rx) = unbounded();
        let (decode_tx, decode_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();

        let decode_queue = shared_queue.clone();
        let decode_clock = playback_clock.clone();
        let decode_worker = thread::spawn(move || {
            decode_main(decode_rx, event_tx, decode_queue, decode_clock);
        });

        let present_latest = self.latest_frame.clone();
        let present_queue = shared_queue;
        let present_clock = playback_clock;
        let shared = self.shared.clone();
        let shutdown_decode_tx = decode_tx.clone();
        let present_worker = thread::spawn(move || {
            present_main(
                backend_rx,
                decode_tx,
                event_rx,
                shared,
                present_latest,
                present_queue,
                present_clock,
            );
        });

        let command_tx = backend_tx.clone();
        let _ = command_tx.send(BackendCommand::SetVolume(self.shared.volume.get()));
        let _ = command_tx.send(BackendCommand::SetMuted(self.shared.muted.get()));
        let _ = command_tx.send(BackendCommand::SetLooping(self.shared.looping.get()));
        let _ = command_tx.send(BackendCommand::SetPlaybackRate(
            self.shared.playback_rate.get(),
        ));
        let _ = command_tx.send(BackendCommand::SetAudioTrackSelection(
            self.shared.audio_track_selection.get(),
        ));
        let _ = command_tx.send(BackendCommand::SetSubtitleTrackSelection(
            self.shared.subtitle_track_selection.get(),
        ));
        let _ = command_tx.send(BackendCommand::SetBufferMemoryLimitBytes(
            self.shared.buffer_memory_limit_bytes.get(),
        ));
        if runtime.target_raster.is_some() {
            let _ = command_tx.send(BackendCommand::SetTargetRaster(runtime.target_raster));
        }

        runtime.workers = Some(VideoWorkerHandles {
            command_tx: backend_tx,
            decode_tx: shutdown_decode_tx,
            present_worker,
            decode_worker,
        });

        Ok(command_tx)
    }

    fn active_command_tx(&self) -> Option<Sender<BackendCommand>> {
        self.runtime
            .lock()
            .workers
            .as_ref()
            .map(|workers| workers.command_tx.clone())
    }

    fn send_if_active(&self, command: BackendCommand) {
        if let Some(command_tx) = self.active_command_tx() {
            let _ = command_tx.send(command);
        }
    }
}

impl Drop for FfmpegVideoBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl VideoBackend for FfmpegVideoBackend {
    fn load(&self, source: VideoSource) -> Result<(), TguiError> {
        validate_video_source(&source)?;
        self.ensure_workers()?
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("video backend is unavailable".to_string()))
    }

    fn play(&self) {
        self.send_if_active(BackendCommand::Play);
    }

    fn pause(&self) {
        self.send_if_active(BackendCommand::Pause);
    }

    fn stop(&self) {
        let Some(command_tx) = self.active_command_tx() else {
            clear_latest_frame(&self.latest_frame);
            self.shared.reset_for_stop();
            return;
        };

        let (completed_tx, completed_rx) = bounded(1);
        if command_tx
            .send(BackendCommand::Stop {
                completed: completed_tx,
            })
            .is_err()
        {
            clear_latest_frame(&self.latest_frame);
            self.shared.reset_for_stop();
            return;
        }

        if completed_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
        {
            clear_latest_frame(&self.latest_frame);
            self.shared.reset_for_stop();
        }
    }

    fn seek(&self, position: Duration) {
        self.send_if_active(BackendCommand::Seek(position));
    }

    fn set_volume(&self, volume: f32) {
        self.send_if_active(BackendCommand::SetVolume(volume));
    }

    fn set_muted(&self, muted: bool) {
        self.send_if_active(BackendCommand::SetMuted(muted));
    }

    fn set_looping(&self, looping: bool) {
        self.send_if_active(BackendCommand::SetLooping(looping));
    }

    fn set_playback_rate(&self, rate: f32) {
        self.send_if_active(BackendCommand::SetPlaybackRate(rate));
    }

    fn set_audio_track_selection(&self, selection: VideoAudioTrackSelection) {
        self.send_if_active(BackendCommand::SetAudioTrackSelection(selection));
    }

    fn set_subtitle_track_selection(&self, selection: VideoSubtitleTrackSelection) {
        self.send_if_active(BackendCommand::SetSubtitleTrackSelection(selection));
    }

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        self.send_if_active(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn set_target_raster(&self, raster: Option<RasterRequest>) {
        let mut runtime = self.runtime.lock();
        runtime.target_raster = raster;
        if let Some(workers) = runtime.workers.as_ref() {
            let _ = workers
                .command_tx
                .send(BackendCommand::SetTargetRaster(raster));
        }
    }

    fn current_render_frame(&self) -> Option<VideoRenderFrame> {
        self.latest_frame.lock().clone()
    }

    fn shutdown(&self) {
        let Some(workers) = self.runtime.lock().workers.take() else {
            clear_latest_frame(&self.latest_frame);
            self.shared.reset_for_stop();
            return;
        };

        let _ = workers.command_tx.send(BackendCommand::Shutdown);
        let _ = workers.decode_tx.send(DecodeCommand::Shutdown);
        let _ = join_with_timeout(workers.present_worker, SHUTDOWN_JOIN_TIMEOUT);
        let _ = join_with_timeout(workers.decode_worker, SHUTDOWN_JOIN_TIMEOUT);
        clear_latest_frame(&self.latest_frame);
        self.shared.reset_for_stop();
    }
}

#[derive(Clone, Debug)]
enum BackendCommand {
    Load(VideoSource),
    Play,
    Pause,
    Stop { completed: Sender<()> },
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    SetLooping(bool),
    SetPlaybackRate(f32),
    SetAudioTrackSelection(VideoAudioTrackSelection),
    SetSubtitleTrackSelection(VideoSubtitleTrackSelection),
    SetBufferMemoryLimitBytes(u64),
    SetTargetRaster(Option<RasterRequest>),
    Shutdown,
}

#[derive(Clone, Debug)]
enum DecodeCommand {
    Load {
        generation: u64,
        source: VideoSource,
    },
    Seek {
        generation: u64,
        source: VideoSource,
        position: Duration,
    },
    SetPlaying {
        generation: u64,
        playing: bool,
    },
    SetVolume(f32),
    SetMuted(bool),
    SetPlaybackRate(f32),
    SetAudioTrackSelection(VideoAudioTrackSelection),
    SetSubtitleTrackSelection(VideoSubtitleTrackSelection),
    SetBufferMemoryLimitBytes(u64),
    SetTargetRaster(Option<RasterRequest>),
    Stop,
    Shutdown,
}

#[derive(Clone)]
enum DecodeEvent {
    StreamOpened(StreamOpenedEvent),
    SubtitleCue(SubtitleCueEvent),
    SubtitleBitmapCue(SubtitleBitmapCueEvent),
    FirstFrameReady {
        generation: u64,
        _position: Duration,
    },
    BufferSnapshot(BufferSnapshot),
    EofDrained {
        generation: u64,
    },
    FatalError {
        generation: u64,
        message: String,
    },
}

#[derive(Clone)]
struct StreamOpenedEvent {
    generation: u64,
    start_position: Duration,
    duration: Option<Duration>,
    intrinsic_size: IntrinsicSize,
    video_size: VideoSize,
    buffering_profile: BufferingProfile,
    audio_clock: Option<SharedAudioClock>,
    audio_tracks: Vec<VideoAudioTrack>,
    audio_track_selection: VideoAudioTrackSelection,
    subtitle_tracks: Vec<VideoSubtitleTrack>,
    subtitle_track_selection: VideoSubtitleTrackSelection,
}

#[derive(Clone)]
struct SubtitleCueEvent {
    generation: u64,
    cue: VideoSubtitleCue,
    placement: Option<VideoSubtitleCuePlacement>,
    style: Option<VideoSubtitleCueStyle>,
}

#[derive(Clone)]
struct SubtitleBitmapCueEvent {
    generation: u64,
    cue: VideoSubtitleBitmapCue,
}

#[derive(Clone, Debug, Default)]
struct BufferSnapshot {
    generation: u64,
    eof_sent: bool,
    total_buffered_memory_bytes: u64,
    buffering_constrained_by_memory_limit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenReason {
    Load,
    Seek,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferingProfile {
    video_queue_high_water: Duration,
    video_queue_hard_water: Duration,
    video_max_packet_count: usize,
    ready_video_frame_count: usize,
    audio_queue_high_water: Duration,
    audio_queue_hard_water: Duration,
    start_buffer_target: Duration,
    rebuffer_target: Duration,
    video_start_buffer_target: Duration,
    video_resume_buffer_target: Duration,
    audio_starving_threshold: Duration,
}

const LOCAL_BUFFERING_PROFILE: BufferingProfile = BufferingProfile {
    video_queue_high_water: LOCAL_VIDEO_QUEUE_HIGH_WATER,
    video_queue_hard_water: LOCAL_VIDEO_QUEUE_HARD_WATER,
    video_max_packet_count: LOCAL_VIDEO_MAX_PACKET_COUNT,
    ready_video_frame_count: LOCAL_READY_VIDEO_FRAME_COUNT,
    audio_queue_high_water: LOCAL_AUDIO_QUEUE_HIGH_WATER,
    audio_queue_hard_water: LOCAL_AUDIO_QUEUE_HARD_WATER,
    start_buffer_target: LOCAL_START_BUFFER_TARGET,
    rebuffer_target: LOCAL_REBUFFER_TARGET,
    video_start_buffer_target: LOCAL_VIDEO_START_BUFFER_TARGET,
    video_resume_buffer_target: LOCAL_VIDEO_RESUME_BUFFER_TARGET,
    audio_starving_threshold: LOCAL_AUDIO_STARVING_THRESHOLD,
};

const NETWORK_BUFFERING_PROFILE: BufferingProfile = BufferingProfile {
    video_queue_high_water: NETWORK_VIDEO_QUEUE_HIGH_WATER,
    video_queue_hard_water: NETWORK_VIDEO_QUEUE_HARD_WATER,
    video_max_packet_count: NETWORK_VIDEO_MAX_PACKET_COUNT,
    ready_video_frame_count: NETWORK_READY_VIDEO_FRAME_COUNT,
    audio_queue_high_water: NETWORK_AUDIO_QUEUE_HIGH_WATER,
    audio_queue_hard_water: NETWORK_AUDIO_QUEUE_HARD_WATER,
    start_buffer_target: NETWORK_START_BUFFER_TARGET,
    rebuffer_target: NETWORK_REBUFFER_TARGET,
    video_start_buffer_target: NETWORK_VIDEO_START_BUFFER_TARGET,
    video_resume_buffer_target: NETWORK_VIDEO_RESUME_BUFFER_TARGET,
    audio_starving_threshold: NETWORK_AUDIO_STARVING_THRESHOLD,
};

struct QueuedVideoPacket {
    packet: ffmpeg::Packet,
    end_position: Duration,
}

struct OpenedVideoDecoder {
    decoder: ffmpeg::decoder::Video,
    codec_id: codec::Id,
    decoder_name: String,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::video::backend::DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES;
    use crate::video::{VideoController, VideoMetrics};

    use super::*;

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
    fn backend_creation_and_target_updates_do_not_start_workers() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = FfmpegVideoBackend::new(shared);

        assert!(backend.runtime.lock().workers.is_none());

        VideoBackend::set_volume(&backend, 0.5);
        VideoBackend::set_muted(&backend, true);
        VideoBackend::set_looping(&backend, true);
        VideoBackend::set_playback_rate(&backend, 1.5);
        VideoBackend::set_buffer_memory_limit_bytes(&backend, 16 * 1024 * 1024);
        VideoBackend::set_target_raster(&backend, Some(RasterRequest::new_clamped(320, 180)));

        let runtime = backend.runtime.lock();
        assert!(runtime.workers.is_none());
        assert_eq!(
            runtime.target_raster,
            Some(RasterRequest::new_clamped(320, 180))
        );
    }

    #[test]
    fn controller_load_rejects_invalid_source_without_starting_workers() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = Arc::new(FfmpegVideoBackend::new(shared.clone()));
        let controller = VideoController::from_parts(shared.clone(), backend.clone());
        let source =
            VideoSource::url("https://example.com/demo.mp4").with_header("Bad\nHeader", "value");

        let error = controller
            .load(source)
            .expect_err("invalid header should fail synchronously");

        assert!(matches!(
            error,
            TguiError::Media(message) if message.contains("invalid line break")
        ));
        assert!(backend.runtime.lock().workers.is_none());
        assert!(matches!(
            controller.playback_state().get(),
            VideoPlaybackState::Error(message) if message.contains("invalid line break")
        ));
        assert!(controller
            .error()
            .get()
            .is_some_and(|message| message.contains("invalid line break")));
        let snapshot = controller.surface_metadata();
        assert!(!snapshot.loading);
        assert!(snapshot.texture.is_none());
        assert!(snapshot
            .error
            .is_some_and(|message| message.contains("invalid line break")));
    }

    #[test]
    fn shutdown_returns_after_timeout_when_workers_are_blocked() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let backend = FfmpegVideoBackend::new(shared);
        let (command_tx, _command_rx) = unbounded();
        let (decode_tx, _decode_rx) = unbounded();
        let present_worker = thread::spawn(|| thread::sleep(Duration::from_millis(250)));
        let decode_worker = thread::spawn(|| thread::sleep(Duration::from_millis(250)));
        backend.runtime.lock().workers = Some(VideoWorkerHandles {
            command_tx,
            decode_tx,
            present_worker,
            decode_worker,
        });

        let started = Instant::now();
        VideoBackend::shutdown(&backend);

        assert!(
            started.elapsed() < Duration::from_millis(300),
            "shutdown should detach blocked video workers after their configured timeouts"
        );
        assert!(backend.runtime.lock().workers.is_none());
        assert!(backend.latest_frame.lock().is_none());
        assert_eq!(
            backend.shared.playback_state.get(),
            VideoPlaybackState::Idle
        );
    }

    #[test]
    fn rational_frame_duration_converts_fps_to_frame_span() {
        assert_eq!(
            rational_frame_duration(ffmpeg::Rational(24, 1)),
            Some(Duration::from_secs_f64(1.0 / 24.0))
        );
        assert_eq!(
            rational_frame_duration(ffmpeg::Rational(24000, 1001)),
            Some(Duration::from_secs_f64(1001.0 / 24000.0))
        );
    }

    #[test]
    fn copy_rgba_frame_pixels_handles_contiguous_and_padded_rows() {
        let contiguous = vec![1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(copy_rgba_frame_pixels(&contiguous, 1, 2, 4), contiguous);

        let padded = vec![1, 2, 3, 4, 99, 5, 6, 7, 8, 88];
        assert_eq!(
            copy_rgba_frame_pixels(&padded, 1, 2, 5),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn copy_rgba_frame_pixels_arc_skips_scratch_for_contiguous_rows() {
        let contiguous = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut scratch = Vec::new();

        let pixels = copy_rgba_frame_pixels_arc(&contiguous, 1, 2, 4, &mut scratch);

        assert_eq!(&*pixels, contiguous.as_slice());
        assert_eq!(scratch.capacity(), 0);
    }

    #[test]
    fn copy_rgba_frame_pixels_arc_reuses_scratch_for_padded_rows() {
        let first = vec![1, 2, 3, 4, 99, 5, 6, 7, 8, 88];
        let second = vec![9, 10, 11, 12, 77, 13, 14, 15, 16, 66];
        let mut scratch = Vec::with_capacity(8);
        let scratch_ptr = scratch.as_ptr();

        let first_pixels = copy_rgba_frame_pixels_arc(&first, 1, 2, 5, &mut scratch);
        let first_scratch_ptr = scratch.as_ptr();
        let second_pixels = copy_rgba_frame_pixels_arc(&second, 1, 2, 5, &mut scratch);

        assert_eq!(&*first_pixels, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&*second_pixels, &[9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(first_scratch_ptr, scratch_ptr);
        assert_eq!(scratch.as_ptr(), scratch_ptr);
        assert_eq!(scratch.capacity(), 8);
    }

    #[test]
    fn copy_bgra_frame_pixels_swizzles_padded_rows_to_rgba() {
        let data = [
            1, 2, 3, 4, 5, 6, 7, 8, 0xEE, 0xEE, 0xEE, 0xEE, 9, 10, 11, 12, 13, 14, 15, 16, 0xEE,
            0xEE, 0xEE, 0xEE,
        ];

        let pixels = copy_bgra_frame_pixels(&data, 2, 2, 12);

        assert_eq!(
            pixels,
            vec![3, 2, 1, 4, 7, 6, 5, 8, 11, 10, 9, 12, 15, 14, 13, 16]
        );
    }

    #[test]
    fn copy_rgb24_frame_pixels_adds_alpha_and_skips_padding() {
        let data = [
            1, 2, 3, 4, 5, 6, 0xEE, 0xEE, 7, 8, 9, 10, 11, 12, 0xEE, 0xEE,
        ];

        let pixels = copy_rgb24_frame_pixels(&data, 2, 2, 8);

        assert_eq!(
            pixels,
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }

    #[test]
    fn copy_bgr24_frame_pixels_swizzles_adds_alpha_and_skips_padding() {
        let data = [
            1, 2, 3, 4, 5, 6, 0xEE, 0xEE, 7, 8, 9, 10, 11, 12, 0xEE, 0xEE,
        ];

        let pixels = copy_bgr24_frame_pixels(&data, 2, 2, 8);

        assert_eq!(
            pixels,
            vec![3, 2, 1, 255, 6, 5, 4, 255, 9, 8, 7, 255, 12, 11, 10, 255]
        );
    }

    #[test]
    fn video_frame_converter_reuses_matching_output_frame() {
        let decoded = test_rgb24_frame(2, 2);
        let mut converter = VideoFrameConverter::new();
        let target = Some(RasterRequest::new_clamped(1, 1));

        let first = converter
            .convert(&decoded, target, 1, 1)
            .expect("first conversion should succeed");
        let first_ptr = converter
            .output_data_ptr()
            .expect("converted frame should be allocated");
        let second = converter
            .convert(&decoded, target, 1, 2)
            .expect("second conversion should succeed");
        let second_ptr = converter
            .output_data_ptr()
            .expect("converted frame should still be allocated");

        assert_eq!(first.size(), (1, 1));
        assert_eq!(second.size(), (1, 1));
        assert_eq!(first_ptr, second_ptr);
        assert!(converter.has_scaler());

        let resized_target = Some(RasterRequest::new_clamped(2, 1));
        let resized = converter
            .convert(&decoded, resized_target, 1, 3)
            .expect("resized conversion should succeed");
        let resized_ptr = converter
            .output_data_ptr()
            .expect("resized frame should be allocated");
        let resized_again = converter
            .convert(&decoded, resized_target, 1, 4)
            .expect("second resized conversion should succeed");
        let resized_again_ptr = converter
            .output_data_ptr()
            .expect("resized frame should still be allocated");

        assert_eq!(resized.size(), (2, 1));
        assert_eq!(resized_again.size(), (2, 1));
        assert_eq!(resized_ptr, resized_again_ptr);
    }

    #[test]
    fn video_frame_converter_skips_scaler_for_matching_rgba_frames() {
        let decoded = test_rgba_frame(2, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert(&decoded, None, 3, 7)
            .expect("rgba conversion should succeed");

        assert_eq!(converted.id(), 3);
        assert_eq!(converted.revision(), 7);
        assert_eq!(converted.size(), (2, 2));
        assert_eq!(
            converted.pixels(),
            copy_rgba_frame_pixels(decoded.data(0), 2, 2, decoded.stride(0)).as_slice()
        );
        assert_eq!(
            converter.output_data_ptr(),
            None,
            "matching RGBA frames should not allocate or use the scaler output frame"
        );
        assert!(
            !converter.has_scaler(),
            "matching RGBA frames should not create a scaler"
        );
    }

    #[test]
    fn video_frame_converter_skips_scaler_for_matching_bgra_frames() {
        let decoded = test_bgra_frame(2, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert(&decoded, None, 5, 9)
            .expect("bgra conversion should succeed");

        assert_eq!(converted.id(), 5);
        assert_eq!(converted.revision(), 9);
        assert_eq!(converted.size(), (2, 2));
        assert_eq!(
            converted.pixels(),
            copy_bgra_frame_pixels(decoded.data(0), 2, 2, decoded.stride(0)).as_slice()
        );
        assert_eq!(
            converter.output_data_ptr(),
            None,
            "matching BGRA frames should not allocate or use the scaler output frame"
        );
        assert!(
            !converter.has_scaler(),
            "matching BGRA frames should not create a scaler"
        );
    }

    #[test]
    fn video_frame_converter_skips_scaler_for_matching_rgb24_frames() {
        let decoded = test_rgb24_frame(2, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert(&decoded, None, 6, 10)
            .expect("rgb24 conversion should succeed");

        assert_eq!(converted.id(), 6);
        assert_eq!(converted.revision(), 10);
        assert_eq!(converted.size(), (2, 2));
        assert_eq!(
            converted.pixels(),
            copy_rgb24_frame_pixels(decoded.data(0), 2, 2, decoded.stride(0)).as_slice()
        );
        assert_eq!(
            converter.output_data_ptr(),
            None,
            "matching RGB24 frames should not allocate or use the scaler output frame"
        );
        assert!(
            !converter.has_scaler(),
            "matching RGB24 frames should not create a scaler"
        );
    }

    #[test]
    fn video_frame_converter_skips_scaler_for_matching_bgr24_frames() {
        let decoded = test_bgr24_frame(2, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert(&decoded, None, 7, 11)
            .expect("bgr24 conversion should succeed");

        assert_eq!(converted.id(), 7);
        assert_eq!(converted.revision(), 11);
        assert_eq!(converted.size(), (2, 2));
        assert_eq!(
            converted.pixels(),
            copy_bgr24_frame_pixels(decoded.data(0), 2, 2, decoded.stride(0)).as_slice()
        );
        assert_eq!(
            converter.output_data_ptr(),
            None,
            "matching BGR24 frames should not allocate or use the scaler output frame"
        );
        assert!(
            !converter.has_scaler(),
            "matching BGR24 frames should not create a scaler"
        );
    }

    #[test]
    fn video_frame_converter_returns_yuv_for_matching_nv12_frames() {
        let mut decoded = test_nv12_frame(4, 2);
        decoded.set_color_space(ffmpeg::util::color::Space::BT2020NCL);
        decoded.set_color_range(ffmpeg::util::color::Range::JPEG);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert_render_frame(&decoded, None, 11, 6)
            .expect("nv12 conversion should succeed");

        let VideoRenderFrame::Yuv(frame) = converted else {
            panic!("matching NV12 frames should stay in YUV form");
        };
        assert_eq!(frame.id(), 11);
        assert_eq!(frame.revision(), 6);
        assert_eq!(frame.size(), (4, 2));
        assert_eq!(frame.format(), VideoYuvFormat::Nv12);
        assert_eq!(
            frame.color_space(),
            VideoYuvColorSpace {
                matrix: VideoYuvColorMatrix::Bt2020,
                range: VideoYuvColorRange::Full,
            }
        );
        assert_eq!(frame.planes().len(), 2);
        assert_eq!(frame.planes()[0].format, VideoYuvPlaneFormat::R8);
        assert_eq!(frame.planes()[1].format, VideoYuvPlaneFormat::Rg8);
        assert_eq!(
            frame.decoded_bytes(),
            (decoded.stride(0) * 2 + decoded.stride(1)) as u64
        );
        assert_eq!(converter.output_data_ptr(), None);
        assert!(!converter.has_scaler());
    }

    #[test]
    fn video_frame_converter_returns_yuv_for_matching_yuv420p_frames() {
        let mut decoded = test_yuv420p_frame(3, 3);
        decoded.set_color_space(ffmpeg::util::color::Space::SMPTE170M);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert_render_frame(&decoded, None, 12, 7)
            .expect("yuv420p conversion should succeed");

        let VideoRenderFrame::Yuv(frame) = converted else {
            panic!("matching YUV420P frames should stay in YUV form");
        };
        assert_eq!(frame.format(), VideoYuvFormat::Yuv420p);
        assert_eq!(
            frame.color_space(),
            VideoYuvColorSpace {
                matrix: VideoYuvColorMatrix::Bt601,
                range: VideoYuvColorRange::Limited,
            }
        );
        assert_eq!(frame.planes().len(), 3);
        assert_eq!(frame.planes()[1].width, 2);
        assert_eq!(frame.planes()[1].height, 2);
        assert_eq!(frame.planes()[2].width, 2);
        assert_eq!(frame.planes()[2].height, 2);
    }

    #[test]
    fn video_frame_converter_keeps_yuv_when_target_raster_scales_frame() {
        let decoded = test_nv12_frame(4, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert_render_frame(&decoded, Some(RasterRequest::new_clamped(8, 4)), 13, 8)
            .expect("scaled NV12 conversion should succeed");

        let VideoRenderFrame::Yuv(frame) = converted else {
            panic!("YUV frames should stay in YUV form when target raster scales");
        };
        assert_eq!(frame.id(), 13);
        assert_eq!(frame.revision(), 8);
        assert_eq!(frame.size(), (4, 2));
        assert!(!converter.has_scaler());
        assert_eq!(converter.output_data_ptr(), None);
    }

    #[test]
    fn video_frame_converter_downscales_yuv_with_rgba_scaler() {
        let decoded = test_nv12_frame(4, 2);
        let mut converter = VideoFrameConverter::new();

        let converted = converter
            .convert_render_frame(&decoded, Some(RasterRequest::new_clamped(2, 1)), 14, 9)
            .expect("downscaled NV12 conversion should succeed");

        let VideoRenderFrame::Rgba(texture) = converted else {
            panic!("downscaled YUV frames should use the RGBA scaler path");
        };
        assert_eq!(texture.id(), 14);
        assert_eq!(texture.revision(), 9);
        assert_eq!(texture.size(), (2, 1));
        assert!(converter.has_scaler());
        assert!(converter.output_data_ptr().is_some());
    }

    #[test]
    fn direct_yuv_frame_rejects_missing_planes_without_panicking() {
        let decoded = test_rgba_frame(2, 2);

        assert!(matches!(
            direct_yuv_frame(&decoded, 1, 1, VideoYuvFormat::Yuv420p),
            Err(TguiError::Media(message)) if message.contains("plane 1")
        ));
    }

    #[test]
    fn url_sources_use_deeper_buffer_profile() {
        let profile =
            buffering_profile_for_source(&VideoSource::url("https://example.com/demo.mp4"));
        assert_eq!(profile, NETWORK_BUFFERING_PROFILE);
        assert_eq!(profile.video_start_buffer_target, Duration::from_secs(5));
    }

    #[test]
    fn local_sources_use_shallower_video_startup_targets() {
        let profile = buffering_profile_for_source(&VideoSource::File("demo.mp4".into()));
        assert_eq!(profile, LOCAL_BUFFERING_PROFILE);
        assert_eq!(
            profile.video_start_buffer_target,
            Duration::from_millis(1500)
        );
        assert_eq!(profile.ready_video_frame_count, 4);
    }

    #[test]
    fn bytes_sources_use_local_buffer_profile_and_no_http_options() {
        let source = VideoSource::bytes_with_extension(vec![1, 2, 3], "mp4");
        let profile = buffering_profile_for_source(&source);
        let options = http_input_options(&source).expect("bytes options should build");

        assert_eq!(profile, LOCAL_BUFFERING_PROFILE);
        assert_eq!(options.get("user_agent"), None);
        assert_eq!(options.get("headers"), None);
    }

    #[test]
    fn http_sources_enable_recovery_and_connection_reuse_options() {
        let options = http_input_options(&VideoSource::url("https://example.com/demo.mp4"))
            .expect("http options should build");
        assert_eq!(
            options.get("user_agent"),
            Some(concat!("tgui/", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(options.get("multiple_requests"), Some("1"));
        assert_eq!(options.get("short_seek_size"), Some("65536"));
        assert_eq!(options.get("reconnect"), Some("1"));
        assert_eq!(options.get("reconnect_streamed"), Some("1"));
        assert_eq!(options.get("reconnect_on_network_error"), Some("1"));
        assert_eq!(options.get("reconnect_on_http_error"), Some("4xx,5xx"));
        assert_eq!(options.get("timeout"), Some("15000000"));
        assert_eq!(options.get("rw_timeout"), Some("15000000"));
    }

    #[test]
    fn http_sources_serialize_custom_headers_for_ffmpeg() {
        let source = VideoSource::url("https://example.com/demo.mp4").with_headers([
            ("Authorization", "Bearer token"),
            ("Referer", "https://example.com/app"),
            ("Cookie", "a=1; b=2"),
        ]);

        let options = http_input_options(&source).expect("http options should build");

        assert_eq!(
            options.get("headers"),
            Some(
                "Authorization: Bearer token\r\nReferer: https://example.com/app\r\nCookie: a=1; b=2\r\n"
            )
        );
        assert_eq!(
            options.get("user_agent"),
            Some(concat!("tgui/", env!("CARGO_PKG_VERSION")))
        );
    }

    #[test]
    fn custom_user_agent_overrides_default_user_agent_option() {
        let source = VideoSource::url("https://example.com/demo.mp4")
            .with_header("User-Agent", "custom-agent/1.0");

        let options = http_input_options(&source).expect("http options should build");

        assert_eq!(
            options.get("headers"),
            Some("User-Agent: custom-agent/1.0\r\n")
        );
        assert_eq!(options.get("user_agent"), None);
    }

    #[test]
    fn invalid_http_headers_are_rejected() {
        let empty_name = VideoSource::url("https://example.com/demo.mp4").with_header("", "value");
        let invalid_name =
            VideoSource::url("https://example.com/demo.mp4").with_header("Bad\nHeader", "value");
        let invalid_value = VideoSource::url("https://example.com/demo.mp4")
            .with_header("Authorization", "Bearer\ntoken");

        assert!(matches!(
            validate_video_source(&empty_name),
            Err(TguiError::Media(message)) if message.contains("cannot be empty")
        ));
        assert!(matches!(
            validate_video_source(&invalid_name),
            Err(TguiError::Media(message)) if message.contains("invalid line break")
        ));
        assert!(matches!(
            validate_video_source(&invalid_value),
            Err(TguiError::Media(message)) if message.contains("invalid line break")
        ));
    }

    #[test]
    fn empty_bytes_video_source_is_rejected_before_open() {
        assert!(matches!(
            validate_video_source(&VideoSource::bytes(Vec::<u8>::new())),
            Err(TguiError::Media(message)) if message.contains("bytes source is empty")
        ));
    }

    fn test_rgb24_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::RGB24, width, height);
        for (index, byte) in frame.data_mut(0).iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        frame
    }

    fn test_bgr24_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::BGR24, width, height);
        let row_len = width as usize * 3;
        let stride = frame.stride(0);
        for y in 0..height as usize {
            for x in 0..width as usize {
                let offset = y * stride + x * 3;
                let pixel = (y * width as usize + x) as u8;
                frame.data_mut(0)[offset..offset + 3].copy_from_slice(&[
                    pixel,
                    pixel.wrapping_add(1),
                    pixel.wrapping_add(2),
                ]);
            }
            for byte in &mut frame.data_mut(0)[y * stride + row_len..(y + 1) * stride] {
                *byte = 0xEE;
            }
        }
        frame
    }

    fn test_rgba_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::RGBA, width, height);
        for (index, byte) in frame.data_mut(0).iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        frame
    }

    fn test_bgra_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::BGRA, width, height);
        let row_len = width as usize * 4;
        let stride = frame.stride(0);
        for y in 0..height as usize {
            for x in 0..width as usize {
                let offset = y * stride + x * 4;
                let pixel = (y * width as usize + x) as u8;
                frame.data_mut(0)[offset..offset + 4].copy_from_slice(&[
                    pixel,
                    pixel.wrapping_add(1),
                    pixel.wrapping_add(2),
                    255,
                ]);
            }
            for byte in &mut frame.data_mut(0)[y * stride + row_len..(y + 1) * stride] {
                *byte = 0xEE;
            }
        }
        frame
    }

    fn test_nv12_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::NV12, width, height);
        fill_plane(&mut frame, 0, height, 0x40);
        fill_plane(&mut frame, 1, height.div_ceil(2), 0x80);
        frame
    }

    fn test_yuv420p_frame(width: u32, height: u32) -> VideoFrame {
        let mut frame = VideoFrame::new(Pixel::YUV420P, width, height);
        let chroma_height = height.div_ceil(2);
        fill_plane(&mut frame, 0, height, 0x40);
        fill_plane(&mut frame, 1, chroma_height, 0x80);
        fill_plane(&mut frame, 2, chroma_height, 0x80);
        frame
    }

    fn fill_plane(frame: &mut VideoFrame, index: usize, height: u32, seed: u8) {
        let stride = frame.stride(index);
        for y in 0..height as usize {
            let row_start = y * stride;
            let row_end = row_start + stride;
            for (offset, byte) in frame.data_mut(index)[row_start..row_end]
                .iter_mut()
                .enumerate()
            {
                *byte = seed.wrapping_add((offset % 17) as u8);
            }
        }
    }

    #[test]
    fn queue_drops_stale_generation_frames() {
        let queue = SharedVideoQueue::new();
        queue.replace_generation(2);
        queue.push_frames(vec![
            QueuedVideoFrame {
                generation: 1,
                position: Duration::ZERO,
                end_position: Duration::from_millis(33),
                frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
                compressed_bytes: 4,
                decoded_bytes: 4,
            },
            QueuedVideoFrame {
                generation: 2,
                position: Duration::from_millis(33),
                end_position: Duration::from_millis(66),
                frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
                compressed_bytes: 4,
                decoded_bytes: 4,
            },
        ]);

        assert_eq!(queue.ready_frame_count(1), 0);
        assert_eq!(queue.ready_frame_count(2), 1);
    }

    #[test]
    fn queue_pop_front_due_respects_generation_and_position() {
        let queue = SharedVideoQueue::new();
        queue.replace_generation(1);
        queue.push_frames(vec![
            QueuedVideoFrame {
                generation: 1,
                position: Duration::from_millis(33),
                end_position: Duration::from_millis(66),
                frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
                compressed_bytes: 4,
                decoded_bytes: 4,
            },
            QueuedVideoFrame {
                generation: 1,
                position: Duration::from_millis(66),
                end_position: Duration::from_millis(99),
                frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
                compressed_bytes: 8,
                decoded_bytes: 8,
            },
        ]);

        assert_eq!(queue.front_position(1), Some(Duration::from_millis(33)));
        assert!(queue.pop_front_due(2, Duration::from_millis(99)).is_none());
        assert!(queue.pop_front_due(1, Duration::from_millis(32)).is_none());
        assert_eq!(queue.ready_frame_count(1), 2);
        assert_eq!(queue.ready_memory_bytes(1), 12);

        let popped = queue
            .pop_front_due(1, Duration::from_millis(33))
            .expect("first frame should be due");

        assert_eq!(popped.position, Duration::from_millis(33));
        assert_eq!(queue.front_position(1), Some(Duration::from_millis(66)));
        assert_eq!(queue.ready_frame_count(1), 1);
        assert_eq!(queue.ready_memory_bytes(1), 8);
    }

    #[test]
    fn stop_without_workers_resets_state_without_starting_workers() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        shared.playback_state.set(VideoPlaybackState::Playing);
        shared.video_size.set(VideoSize {
            width: 16,
            height: 9,
        });
        shared.error.set(Some("boom".to_string()));
        let backend = FfmpegVideoBackend::new(shared.clone());
        *backend.latest_frame.lock() = Some(VideoRenderFrame::rgba(Arc::new(TextureFrame::new(
            1,
            1,
            vec![255; 4],
        ))));

        VideoBackend::stop(&backend);

        assert!(backend.runtime.lock().workers.is_none());
        assert_eq!(shared.playback_state.get(), VideoPlaybackState::Idle);
        assert_eq!(shared.video_size.get(), VideoSize::default());
        assert_eq!(shared.error.get(), None);
        assert!(backend.latest_frame.lock().is_none());
    }

    #[test]
    fn ready_memory_counts_decoded_frame_bytes() {
        let queue = SharedVideoQueue::new();
        queue.replace_generation(1);
        let small = Arc::new(TextureFrame::new(1, 1, vec![255; 4]));
        let large = Arc::new(TextureFrame::new(2, 1, vec![255; 8]));
        queue.push_frames(vec![
            QueuedVideoFrame {
                generation: 1,
                position: Duration::ZERO,
                end_position: Duration::from_millis(33),
                decoded_bytes: small.pixels().len() as u64,
                compressed_bytes: 100,
                frame: VideoRenderFrame::rgba(small),
            },
            QueuedVideoFrame {
                generation: 1,
                position: Duration::from_millis(33),
                end_position: Duration::from_millis(66),
                decoded_bytes: large.pixels().len() as u64,
                compressed_bytes: 100,
                frame: VideoRenderFrame::rgba(large),
            },
        ]);

        assert_eq!(queue.ready_memory_bytes(1), 12);
        assert_eq!(queue.head_frame_memory_bytes(1), Some(4));

        assert!(queue.pop_front_matching(1).is_some());

        assert_eq!(queue.ready_memory_bytes(1), 8);
        assert_eq!(queue.head_frame_memory_bytes(1), Some(8));
    }
}
