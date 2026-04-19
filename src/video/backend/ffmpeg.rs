use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender, TryRecvError};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScalingFlags};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::{audio::Audio as AudioFrame, video::Video as VideoFrame};
use ffmpeg_next as ffmpeg;

use crate::media::{IntrinsicSize, TextureFrame};
use crate::TguiError;

use super::{BackendSharedState, VideoBackend};
use crate::video::{PlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot};

// 后台播放线程在非播放态下轮询控制命令的时间间隔。
// 越小，Play/Pause/Seek 响应越快；越大，空闲时更省 CPU。
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);

// 本地文件模式下，视频队列的软上限。
// 使用“时间”而不是“帧数”衡量，避免 60/120fps 视频下缓存变得过浅。
const LOCAL_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(3);
// 本地文件模式下，视频队列的硬上限。
// 达到后必须暂停 demux/解码，避免继续累积过多待显示帧。
const LOCAL_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(4);
// 本地文件模式下，视频队列允许保留的最大帧数保险丝。
// 这个值不是主要策略，只用于防止异常时间戳或损坏源导致无限堆帧。
const LOCAL_VIDEO_MAX_PACKET_COUNT: usize = 120;
const LOCAL_READY_VIDEO_FRAME_COUNT: usize = 4;
// 本地文件模式下，音频缓冲的软上限。
// 正常播放时达到该值后，可暂缓继续读包，避免音频缓存过深。
const LOCAL_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(1500);
// 本地文件模式下，音频缓冲的硬上限。
// 无论当前状态如何，达到该值都应强制节流，防止内存持续增长。
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);

// 网络流模式下，视频队列的软上限。
// 使用时间水位控制，在高帧率视频上也能保持稳定的真实缓存时长。
const NETWORK_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(5);
// 网络流模式下，视频队列的硬上限。
// 用比本地更深的视频缓存吸收网络读包抖动。
const NETWORK_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(6);
// 网络流模式下，视频队列允许保留的最大帧数保险丝。
// 只做安全兜底，不参与正常的时间水位调度。
const NETWORK_VIDEO_MAX_PACKET_COUNT: usize = 300;
const NETWORK_READY_VIDEO_FRAME_COUNT: usize = 8;
// 网络流模式下，音频缓冲的软上限。
// 设得更深，尽量减少网络短抖动引发的反复 Buffering。
const NETWORK_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(4000);
// 网络流模式下，音频缓冲的硬上限。
// 防止网络恢复后一次性灌入过多音频样本。
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);

// 本地文件首次点击播放时，音频至少要攒到多深才开始真正输出。
const LOCAL_START_BUFFER_TARGET: Duration = Duration::from_millis(1000);
// 本地文件从 Buffering 恢复播放时，音频至少要恢复到多深。
const LOCAL_REBUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时的音频启动门槛。
// 比本地更高，用更深缓冲换取更稳定的连续播放。
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
// 避免因为时间戳微小偏差把应该保留的首帧也丢掉。
const VIDEO_SEEK_PREROLL_TOLERANCE: Duration = Duration::from_millis(50);

// 本地文件模式下，音频缓冲低于这个值就认为快饿死了，需要进入 Buffering。
const LOCAL_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(120);
// 网络流模式下，音频进入“危险区”的阈值。
// 设得更高，好让播放器在真正 underflow 前提前停下来攒缓冲。
const NETWORK_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(250);

// 视频帧显示判定的时间容差。
// 播放位置距离帧时间戳只差一点点时，允许提前呈现，减少卡在临界点的抖动。
const VIDEO_PRESENT_TOLERANCE: Duration = Duration::from_millis(8);
// 当 demux 因缓存已满而暂时空转时，后台线程每次 sleep 的时长。
// 控制“既别忙等烧 CPU，也别睡太久导致恢复迟钝”之间的平衡。
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);

static FFMPEG_INIT: Once = Once::new();

static VIDEO_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

macro_rules! video_debug {
    ($($arg:tt)*) => {
        if video_debug_enabled() {
            eprintln!("[tgui-video] {}", format_args!($($arg)*));
        }
    };
}

fn video_debug_enabled() -> bool {
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

enum BackendCommand {
    Load(VideoSource),
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    SetBufferMemoryLimitBytes(u64),
    Shutdown,
}

pub(crate) struct FfmpegVideoBackend {
    command_tx: Sender<BackendCommand>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegVideoBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        let (command_tx, command_rx) = unbounded();
        let latest_frame = Arc::new(Mutex::new(None));
        let worker_frame = latest_frame.clone();
        let worker = thread::spawn(move || worker_main(command_rx, shared, worker_frame));
        Self {
            command_tx,
            latest_frame,
            worker: Mutex::new(Some(worker)),
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
        self.command_tx
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("video backend is unavailable".to_string()))
    }

    fn play(&self) {
        let _ = self.command_tx.send(BackendCommand::Play);
    }

    fn pause(&self) {
        let _ = self.command_tx.send(BackendCommand::Pause);
    }

    fn seek(&self, position: Duration) {
        let _ = self.command_tx.send(BackendCommand::Seek(position));
    }

    fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(BackendCommand::SetVolume(volume));
    }

    fn set_muted(&self, muted: bool) {
        let _ = self.command_tx.send(BackendCommand::SetMuted(muted));
    }

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        let _ = self
            .command_tx
            .send(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn current_frame(&self) -> Option<Arc<TextureFrame>> {
        self.latest_frame
            .lock()
            .expect("video frame lock poisoned")
            .clone()
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);
        if let Some(worker) = self.worker.lock().expect("worker lock poisoned").take() {
            let _ = worker.join();
        }
    }
}

fn worker_main(
    command_rx: Receiver<BackendCommand>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
) {
    FFMPEG_INIT.call_once(|| {
        let _ = ffmpeg::init();
    });

    let mut current_source: Option<VideoSource> = None;
    let mut current_position = Duration::ZERO;
    let mut should_play = false;
    let mut current_session: Option<PlaybackSession> = None;

    loop {
        let command_result = if should_play {
            match command_rx.try_recv() {
                Ok(command) => Ok(command),
                Err(TryRecvError::Empty) => Err(RecvTimeoutError::Timeout),
                Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
            }
        } else {
            command_rx.recv_timeout(COMMAND_POLL_INTERVAL)
        };

        match command_result {
            Ok(command) => match command {
                BackendCommand::Shutdown => break,
                BackendCommand::Load(source) => {
                    video_debug!("command load source={:?}", source);
                    current_position = Duration::ZERO;
                    should_play = false;
                    current_source = Some(source.clone());
                    clear_latest_frame(&latest_frame);
                    shared.reset_for_load();
                    current_session = Some(
                        match PlaybackSession::open(
                            source,
                            current_position,
                            &shared,
                            &latest_frame,
                        ) {
                            Ok(session) => session,
                            Err(error) => {
                                shared.set_error(error.to_string());
                                continue;
                            }
                        },
                    );
                }
                BackendCommand::Play => {
                    video_debug!("command play pos={:?}", current_position);
                    should_play = true;
                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        if session.can_start_playback() {
                            session.set_playing(true);
                            shared.playback_state.set(PlaybackState::Playing);
                        } else {
                            shared.playback_state.set(PlaybackState::Buffering);
                        }
                    }
                }
                BackendCommand::Pause => {
                    video_debug!("command pause");
                    should_play = false;
                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        let mut metrics = shared.metrics.get();
                        metrics.position = session.playback_position();
                        shared.metrics.set(metrics);
                        shared.playback_state.set(PlaybackState::Paused);
                    }
                }
                BackendCommand::Seek(position) => {
                    video_debug!("command seek target={:?}", position);
                    current_position = position;
                    if let Some(source) = current_source.clone() {
                        if let Some(existing) = current_session.as_mut() {
                            existing.set_playing(false);
                            existing.clear_audio_buffer();
                        }

                        // 不要先清空最后一帧，避免视觉闪一下
                        shared.playback_state.set(PlaybackState::Loading);

                        current_session = Some(
                            match PlaybackSession::open(
                                source,
                                current_position,
                                &shared,
                                &latest_frame,
                            ) {
                                Ok(mut session) => {
                                    session.set_playing(false);

                                    if should_play {
                                        if session.can_resume_playback() {
                                            session.set_playing(true);
                                            shared.playback_state.set(PlaybackState::Playing);
                                        } else {
                                            shared.playback_state.set(PlaybackState::Buffering);
                                        }
                                    } else {
                                        shared.playback_state.set(PlaybackState::Paused);
                                    }

                                    session
                                }
                                Err(error) => {
                                    shared.set_error(error.to_string());
                                    continue;
                                }
                            },
                        );
                    }
                }
                BackendCommand::SetVolume(volume) => {
                    let volume = volume.clamp(0.0, 1.0);
                    video_debug!("command volume {}", volume);
                    shared.volume.set(volume);
                    if let Some(session) = current_session.as_mut() {
                        session.set_volume(volume);
                    }
                }
                BackendCommand::SetMuted(muted) => {
                    video_debug!("command muted {}", muted);
                    shared.muted.set(muted);
                    if let Some(session) = current_session.as_mut() {
                        session.set_muted(muted);
                    }
                }
                BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                    video_debug!("command buffer memory limit {} MB", bytes / 1024 / 1024);
                    shared.buffer_memory_limit_bytes.set(bytes);
                    if let Some(session) = current_session.as_mut() {
                        session.set_buffer_memory_limit_bytes(bytes);
                    }
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if !should_play {
            continue;
        }

        let step_outcome = match current_session.as_mut() {
            Some(session) => session.step(&shared, &command_rx, &mut current_position),
            None => continue,
        };

        match step_outcome {
            StepOutcome::Continue => {}
            StepOutcome::Restart(position) => {
                video_debug!("step restart pos={:?}", position);
                current_position = position;
                if let Some(source) = current_source.clone() {
                    shared.playback_state.set(PlaybackState::Loading);

                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        session.clear_audio_buffer();
                    }

                    current_session = match PlaybackSession::open(
                        source,
                        current_position,
                        &shared,
                        &latest_frame,
                    ) {
                        Ok(mut reopened) => {
                            reopened.set_playing(false);

                            if should_play {
                                if reopened.can_resume_playback() {
                                    reopened.set_playing(true);
                                    shared.playback_state.set(PlaybackState::Playing);
                                } else {
                                    shared.playback_state.set(PlaybackState::Buffering);
                                }
                            } else {
                                shared.playback_state.set(PlaybackState::Paused);
                            }

                            Some(reopened)
                        }
                        Err(error) => {
                            shared.set_error(error.to_string());
                            None
                        }
                    };
                }
            }
            StepOutcome::Reload { source, position } => {
                video_debug!("step reload source={:?} pos={:?}", source, position);
                current_source = Some(source.clone());
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                clear_latest_frame(&latest_frame);
                shared.reset_for_load();
                current_session = Some(
                    match PlaybackSession::open(source, current_position, &shared, &latest_frame) {
                        Ok(session) => session,
                        Err(error) => {
                            shared.set_error(error.to_string());
                            continue;
                        }
                    },
                );
            }
            StepOutcome::Paused(position) => {
                video_debug!("step paused pos={:?}", position);
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                }
                shared.playback_state.set(PlaybackState::Paused);
            }
            StepOutcome::Ended(position) => {
                video_debug!("step ended pos={:?}", position);
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                let mut metrics = shared.metrics.get();
                metrics.position = position;
                shared.metrics.set(metrics);
                shared.playback_state.set(PlaybackState::Ended);
            }
            StepOutcome::Error(error) => {
                video_debug!("step error {}", error);
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                shared.set_error(error);
            }
            StepOutcome::Shutdown => break,
        }
    }
}

fn debug_log_due(last: &mut Option<Instant>, interval: Duration) -> bool {
    if !video_debug_enabled() {
        return false;
    }

    let now = Instant::now();
    match last {
        Some(previous) if now.duration_since(*previous) < interval => false,
        _ => {
            *last = Some(now);
            true
        }
    }
}

fn clear_latest_frame(latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>) {
    *latest_frame.lock().expect("video frame lock poisoned") = None;
}

enum StepOutcome {
    Continue,
    Restart(Duration),
    Reload {
        source: VideoSource,
        position: Duration,
    },
    Paused(Duration),
    Ended(Duration),
    Shutdown,
    Error(String),
}

enum ReceiveVideoOutcome {
    Position(Option<Duration>),
    Command(StepOutcome),
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
    position: Duration,
    end_position: Duration,
}

struct QueuedVideoFrame {
    position: Duration,
    end_position: Duration,
    texture: Arc<TextureFrame>,
    compressed_bytes: u64,
}

struct OpenedVideoDecoder {
    decoder: ffmpeg::decoder::Video,
    codec_id: codec::Id,
    decoder_name: String,
}

struct PlaybackSession {
    start_position: Duration,
    duration: Option<Duration>,
    video_frame_duration: Duration,
    input: format::context::Input,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_decoder: ffmpeg::decoder::Video,
    video_codec_id: codec::Id,
    video_decoder_name: String,
    audio_decoder: Option<ffmpeg::decoder::Audio>,
    scaler: Scaler,
    resampler: Option<Resampler>,
    video_time_base: ffmpeg::Rational,
    audio_time_base: Option<ffmpeg::Rational>,
    audio_output: Option<AudioOutput>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    pending_video_packets: VecDeque<QueuedVideoPacket>,
    ready_video_frames: VecDeque<QueuedVideoFrame>,
    buffering_profile: BufferingProfile,
    buffer_memory_limit_bytes: u64,
    pending_video_compressed_bytes: u64,
    pending_audio_compressed_bytes: u64,
    paused_position: Duration,
    last_presented_position: Duration,
    play_started_at: Option<Instant>,
    eof_sent: bool,
    last_debug_state_log_at: Option<Instant>,
    last_debug_wait_log_at: Option<Instant>,
    last_debug_empty_ready_log_at: Option<Instant>,
}

impl PlaybackSession {
    fn open(
        source: VideoSource,
        start_position: Duration,
        shared: &BackendSharedState,
        latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>,
    ) -> Result<Self, TguiError> {
        let source_url = match &source {
            VideoSource::File(path) => path
                .to_str()
                .ok_or_else(|| TguiError::Media("video path is not valid UTF-8".to_string()))?
                .to_string(),
            VideoSource::Url(url) => url.clone(),
        };
        video_debug!(
            "open source={:?} start_position={:?}",
            source,
            start_position
        );
        let buffering_profile = buffering_profile_for_source(&source);

        let mut input = open_input(&source, &source_url)
            .map_err(|error| TguiError::Media(format!("failed to open video source: {error}")))?;

        if !start_position.is_zero() {
            let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
            input.seek(timestamp, ..timestamp).map_err(|error| {
                TguiError::Media(format!("failed to seek video source: {error}"))
            })?;
        }

        let video_stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| TguiError::Media("video stream not found".to_string()))?;
        let video_stream_index = video_stream.index();
        let video_time_base = video_stream.time_base();
        let opened_video_decoder = open_video_decoder(&video_stream)?;
        let video_decoder = opened_video_decoder.decoder;
        let scaler = Scaler::get(
            video_decoder.format(),
            video_decoder.width(),
            video_decoder.height(),
            Pixel::RGBA,
            video_decoder.width(),
            video_decoder.height(),
            ScalingFlags::BILINEAR,
        )
        .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))?;

        let intrinsic_size =
            IntrinsicSize::from_pixels(video_decoder.width(), video_decoder.height());
        shared.video_size.set(VideoSize {
            width: video_decoder.width(),
            height: video_decoder.height(),
        });

        let duration = stream_duration(video_stream.duration(), video_time_base);
        let video_frame_duration =
            stream_frame_duration(&video_stream).unwrap_or(Duration::from_millis(33));
        let audio_stream = input.streams().best(media::Type::Audio);
        let (audio_stream_index, audio_decoder, audio_time_base, resampler, audio_output) =
            if let Some(audio_stream) = audio_stream {
                let audio_stream_index = audio_stream.index();
                let audio_time_base = audio_stream.time_base();
                let audio_context =
                    codec::context::Context::from_parameters(audio_stream.parameters()).map_err(
                        |error| TguiError::Media(format!("failed to open audio codec: {error}")),
                    )?;
                let mut audio_decoder = audio_context.decoder().audio().map_err(|error| {
                    TguiError::Media(format!("failed to create audio decoder: {error}"))
                })?;
                audio_decoder
                    .set_parameters(audio_stream.parameters())
                    .map_err(|error| {
                        TguiError::Media(format!("failed to configure audio decoder: {error}"))
                    })?;
                if audio_decoder.channel_layout().is_empty() {
                    audio_decoder.set_channel_layout(ffmpeg::ChannelLayout::default(
                        audio_decoder.channels().into(),
                    ));
                }
                let audio_output = AudioOutput::new(shared.volume.get(), shared.muted.get())
                    .map_err(|error| {
                        TguiError::Media(format!("failed to create audio output: {error}"))
                    })?;
                let resampler = Resampler::get(
                    audio_decoder.format(),
                    audio_decoder.channel_layout(),
                    audio_decoder.rate(),
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    ffmpeg::ChannelLayout::default(audio_output.channels().into()),
                    audio_output.sample_rate(),
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to create audio resampler: {error}"))
                })?;
                (
                    Some(audio_stream_index),
                    Some(audio_decoder),
                    Some(audio_time_base),
                    Some(resampler),
                    Some(audio_output),
                )
            } else {
                (None, None, None, None, None)
            };

        let mut session = Self {
            start_position,
            duration,
            video_frame_duration,
            input,
            video_stream_index,
            audio_stream_index,
            video_decoder,
            video_codec_id: opened_video_decoder.codec_id,
            video_decoder_name: opened_video_decoder.decoder_name,
            audio_decoder,
            scaler,
            resampler,
            video_time_base,
            audio_time_base,
            audio_output,
            latest_frame: latest_frame.clone(),
            pending_video_packets: VecDeque::new(),
            ready_video_frames: VecDeque::new(),
            buffering_profile,
            buffer_memory_limit_bytes: shared.buffer_memory_limit_bytes.get(),
            pending_video_compressed_bytes: 0,
            pending_audio_compressed_bytes: 0,
            paused_position: start_position,
            last_presented_position: start_position,
            play_started_at: None,
            eof_sent: false,
            last_debug_state_log_at: None,
            last_debug_wait_log_at: None,
            last_debug_empty_ready_log_at: None,
        };

        let preview_position = session.prime_first_frame(shared)?;

        video_debug!(
            "open ready preview_position={:?} duration={:?} video_size={}x{} ready={} packets={}",
            preview_position,
            duration,
            intrinsic_size.width as u32,
            intrinsic_size.height as u32,
            session.ready_video_frames.len(),
            session.pending_video_packets.len()
        );

        session.paused_position = preview_position;
        session.last_presented_position = preview_position;
        let mut metrics = shared.metrics.get();
        metrics.duration = duration;
        metrics.position = preview_position;
        metrics.buffered = Some(preview_position);
        metrics.video_width = intrinsic_size.width as u32;
        metrics.video_height = intrinsic_size.height as u32;
        shared.metrics.set(metrics);
        shared.error.set(None);
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size,
            texture: session
                .latest_frame
                .lock()
                .expect("video frame lock poisoned")
                .clone(),
            loading: false,
            error: None,
        });
        shared.playback_state.set(PlaybackState::Ready);

        Ok(session)
    }

    fn update_buffered_metrics(&self, shared: &BackendSharedState) {
        let current = self.playback_position();

        let audio_buffer_end = self
            .audio_output
            .as_ref()
            .map(|output| current.saturating_add(output.buffered_duration()));

        let video_buffer_end =
            furthest_video_buffer_end(&self.ready_video_frames, &self.pending_video_packets);

        let buffered = match (audio_buffer_end, video_buffer_end) {
            (Some(a), Some(v)) => Some(a.min(v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
            (None, None) => None,
        };

        let mut metrics = shared.metrics.get();
        metrics.position = current;
        metrics.buffered = buffered;
        metrics.video_width = self.video_decoder.width();
        metrics.video_height = self.video_decoder.height();
        shared.metrics.set(metrics);
    }

    fn maybe_log_state(&mut self, shared: &BackendSharedState, label: &str) {
        if !debug_log_due(
            &mut self.last_debug_state_log_at,
            Duration::from_millis(500),
        ) {
            return;
        }

        let playback_position = self.playback_position();
        let audio_buffered = self
            .audio_output
            .as_ref()
            .map(|output| output.buffered_duration())
            .unwrap_or(Duration::ZERO);
        let audio_clock = self
            .audio_output
            .as_ref()
            .map(|output| output.position())
            .unwrap_or(Duration::ZERO);
        let audio_started_clock = self
            .audio_output
            .as_ref()
            .map(|output| output.has_started_clock())
            .unwrap_or(self.play_started_at.is_some());
        let next_ready = self.ready_video_frames.front().map(|frame| frame.position);
        let next_gap = next_ready.map(|position| position.saturating_sub(playback_position));
        let pending_packet_memory = self.pending_video_packet_memory_bytes();
        let ready_frame_memory = self.ready_video_frame_memory_bytes();
        let audio_memory = self.audio_buffered_memory_bytes();
        let total_memory =
            total_buffered_memory_bytes(pending_packet_memory, ready_frame_memory, audio_memory);

        video_debug!(
            "{} state={:?} pos={:?} audio_clock={:?} audio_buf={:?} video_buf={:?} compressed_buf={} ({:.2} MiB, packets={} / {:.2} MiB, frames={} / {:.2} MiB, audio={} / {:.2} MiB) compressed_limit={} ({:.2} MiB) ready={} packets={} next_ready={:?} next_gap={:?} eof_sent={} can_resume={}",
            label,
            shared.playback_state.get(),
            playback_position,
            audio_clock,
            audio_buffered,
            self.video_buffered_duration(),
            total_memory,
            bytes_to_mib(total_memory),
            pending_packet_memory,
            bytes_to_mib(pending_packet_memory),
            ready_frame_memory,
            bytes_to_mib(ready_frame_memory),
            audio_memory,
            bytes_to_mib(audio_memory),
            self.buffer_memory_limit_bytes,
            bytes_to_mib(self.buffer_memory_limit_bytes),
            self.ready_video_frames.len(),
            self.pending_video_packets.len(),
            next_ready,
            next_gap,
            self.eof_sent,
            self.can_resume_playback()
        );

        if self.ready_video_frames.is_empty()
            && !self.pending_video_packets.is_empty()
            && debug_log_due(
                &mut self.last_debug_empty_ready_log_at,
                Duration::from_millis(300),
            )
        {
            video_debug!(
                "{} ready queue empty while packets pending packets={} audio_started_clock={}",
                label,
                self.pending_video_packets.len(),
                audio_started_clock
            );
        }
    }

    fn should_throttle_demux(&self) -> bool {
        should_throttle_demux(
            self.total_buffered_memory_bytes() >= self.buffer_memory_limit_bytes,
            self.audio_buffered_duration() >= self.buffering_profile.audio_queue_hard_water,
            self.ready_video_buffered_duration() >= self.buffering_profile.video_queue_hard_water,
            self.pending_video_packets.len() >= self.buffering_profile.video_max_packet_count,
        )
    }

    fn pending_video_packet_memory_bytes(&self) -> u64 {
        pending_video_packet_memory_bytes(&self.pending_video_packets)
    }

    fn ready_video_frame_memory_bytes(&self) -> u64 {
        ready_video_frame_memory_bytes(&self.ready_video_frames)
    }

    fn audio_buffered_memory_bytes(&self) -> u64 {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_memory_bytes())
            .unwrap_or(0)
    }

    fn total_buffered_memory_bytes(&self) -> u64 {
        total_buffered_memory_bytes(
            self.pending_video_packet_memory_bytes(),
            self.ready_video_frame_memory_bytes(),
            self.audio_buffered_memory_bytes(),
        )
    }

    fn buffer_memory_limit_reached(&self) -> bool {
        self.total_buffered_memory_bytes() >= self.buffer_memory_limit_bytes
    }

    fn estimated_next_video_frame_memory_bytes(&self) -> u64 {
        self.ready_video_frames
            .front()
            .map(|frame| frame.compressed_bytes)
            .filter(|bytes| *bytes > 0)
            .or_else(|| {
                let (sum, count) = self
                    .ready_video_frames
                    .iter()
                    .map(|frame| frame.compressed_bytes)
                    .filter(|bytes| *bytes > 0)
                    .fold((0u64, 0u64), |(sum, count), bytes| {
                        (sum.saturating_add(bytes), count + 1)
                    });
                (count > 0).then(|| sum / count)
            })
            .or_else(|| {
                self.pending_video_packets
                    .front()
                    .map(|packet| packet.packet.size() as u64)
            })
            .unwrap_or_else(|| {
                self.pending_video_compressed_bytes
            })
    }

    fn buffering_constrained_by_memory_limit(&self) -> bool {
        buffering_constrained_by_memory_limit(
            self.total_buffered_memory_bytes(),
            self.buffer_memory_limit_bytes,
            self.estimated_next_video_frame_memory_bytes(),
        )
    }

    fn queued_video_tail_position(&self) -> Option<Duration> {
        self.pending_video_packets
            .back()
            .map(|packet| packet.end_position)
            .or_else(|| {
                self.ready_video_frames
                    .back()
                    .map(|frame| frame.end_position)
            })
            .or(Some(self.last_presented_position))
    }

    fn decoded_video_tail_position(&self) -> Option<Duration> {
        self.ready_video_frames
            .back()
            .map(|frame| frame.end_position)
            .or(Some(self.last_presented_position))
    }

    fn queue_video_packet(&mut self, packet: ffmpeg::Packet) {
        let position = packet
            .pts()
            .or_else(|| packet.dts())
            .and_then(|timestamp| pts_to_duration(Some(timestamp), self.video_time_base))
            .unwrap_or_else(|| {
                self.queued_video_tail_position()
                    .unwrap_or(self.start_position)
            });
        let duration = packet_duration(packet.duration(), self.video_time_base)
            .unwrap_or(self.video_frame_duration);
        let is_key = packet.is_key();
        self.pending_video_packets.push_back(QueuedVideoPacket {
            packet,
            position,
            end_position: position.saturating_add(duration),
        });
        if video_debug_enabled() && (self.pending_video_packets.len() == 1 || is_key) {
            video_debug!(
                "queue packet pos={:?} end={:?} key={} packets={} ready={}",
                position,
                position.saturating_add(duration),
                is_key,
                self.pending_video_packets.len(),
                self.ready_video_frames.len()
            );
        }
    }

    fn fill_ready_video_frames(
        &mut self,
        shared: &BackendSharedState,
        command_rx: Option<&Receiver<BackendCommand>>,
        respect_buffer_memory_limit: bool,
    ) -> Result<ReceiveVideoOutcome, TguiError> {
        let mut last_position = None;
        let initial_packets = self.pending_video_packets.len();
        let initial_ready = self.ready_video_frames.len();
        let mut decode_budget = self.buffering_profile.ready_video_frame_count;

        while decode_budget > 0 && (!respect_buffer_memory_limit || !self.buffer_memory_limit_reached())
        {
            let Some(queued_packet) = self.pending_video_packets.pop_front() else {
                break;
            };

            if let Some(receiver) = command_rx {
                let mut command_position = queued_packet.position;
                if let Some(outcome) = self.process_commands(shared, receiver, &mut command_position)
                {
                    match outcome {
                        StepOutcome::Restart(_)
                        | StepOutcome::Reload { .. }
                        | StepOutcome::Paused(_)
                        | StepOutcome::Ended(_)
                        | StepOutcome::Shutdown => {
                            self.pending_video_packets.push_front(queued_packet);
                            return Ok(ReceiveVideoOutcome::Command(outcome));
                        }
                        StepOutcome::Continue => {}
                        StepOutcome::Error(error) => return Err(TguiError::Media(error)),
                    }
                }
            }

                self.video_decoder
                    .send_packet(&queued_packet.packet)
                    .map_err(|error| self.video_packet_send_error(error))?;
                self.pending_video_compressed_bytes = self
                    .pending_video_compressed_bytes
                    .saturating_add(queued_packet.packet.size() as u64);
                match self.receive_video_frames(
                    shared,
                    command_rx,
                respect_buffer_memory_limit,
                &mut decode_budget,
            )? {
                ReceiveVideoOutcome::Position(Some(position)) => last_position = Some(position),
                ReceiveVideoOutcome::Position(None) => {}
                ReceiveVideoOutcome::Command(outcome) => {
                    return Ok(ReceiveVideoOutcome::Command(outcome));
                }
            }
        }

        if decode_budget > 0
            && self.pending_video_packets.is_empty()
            && self.eof_sent
            && (!respect_buffer_memory_limit || !self.buffer_memory_limit_reached())
        {
            loop {
                match self.receive_video_frames(
                    shared,
                    command_rx,
                    respect_buffer_memory_limit,
                    &mut decode_budget,
                )? {
                    ReceiveVideoOutcome::Position(Some(position)) => last_position = Some(position),
                    ReceiveVideoOutcome::Position(None) => break,
                    ReceiveVideoOutcome::Command(outcome) => {
                        return Ok(ReceiveVideoOutcome::Command(outcome));
                    }
                }
                if decode_budget == 0
                    || (respect_buffer_memory_limit && self.buffer_memory_limit_reached())
                {
                    break;
                }
            }
        }

        if video_debug_enabled() {
            let decoded_frames = self.ready_video_frames.len().saturating_sub(initial_ready);
            let consumed_packets = initial_packets.saturating_sub(self.pending_video_packets.len());
            if consumed_packets > 0 || decoded_frames > 0 {
                let pending_packet_memory = self.pending_video_packet_memory_bytes();
                let ready_frame_memory = self.ready_video_frame_memory_bytes();
                let audio_memory = self.audio_buffered_memory_bytes();
                let total_memory = total_buffered_memory_bytes(
                    pending_packet_memory,
                    ready_frame_memory,
                    audio_memory,
                );
                video_debug!(
                    "fill ready consumed_packets={} decoded_frames={} ready={} packets={} compressed_buf={} ({:.2} MiB, packets={} / {:.2} MiB, frames={} / {:.2} MiB, audio={} / {:.2} MiB) last_position={:?} eof_sent={}",
                    consumed_packets,
                    decoded_frames,
                    self.ready_video_frames.len(),
                    self.pending_video_packets.len(),
                    total_memory,
                    bytes_to_mib(total_memory),
                    pending_packet_memory,
                    bytes_to_mib(pending_packet_memory),
                    ready_frame_memory,
                    bytes_to_mib(ready_frame_memory),
                    audio_memory,
                    bytes_to_mib(audio_memory),
                    last_position,
                    self.eof_sent
                );
            }
        }

        Ok(ReceiveVideoOutcome::Position(last_position))
    }

    fn prime_first_frame(&mut self, shared: &BackendSharedState) -> Result<Duration, TguiError> {
        loop {
            match self.fill_ready_video_frames(shared, None, false)? {
                ReceiveVideoOutcome::Position(_) => {
                    if let Some(position) = self.present_next_video_frame(shared) {
                        return Ok(position);
                    }
                }
                ReceiveVideoOutcome::Command(_) => {}
            }

            let next_packet = {
                let mut packets = self.input.packets();
                packets
                    .next()
                    .map(|(stream, packet)| (stream.index(), packet))
            };
            let Some((stream_index, packet)) = next_packet else {
                self.eof_sent = true;
                self.video_decoder.send_eof().map_err(|error| {
                    TguiError::Media(format!("failed to flush preview decoder: {error}"))
                })?;
                match self.fill_ready_video_frames(shared, None, false)? {
                    ReceiveVideoOutcome::Position(_) => {
                        if let Some(position) = self.present_next_video_frame(shared) {
                            return Ok(position);
                        }
                    }
                    ReceiveVideoOutcome::Command(_) => {}
                };
                break;
            };

            if stream_index != self.video_stream_index {
                continue;
            }

            self.queue_video_packet(packet);
        }

        Err(TguiError::Media(
            "video source does not contain a decodable frame".to_string(),
        ))
    }

    fn set_playing(&mut self, playing: bool) {
        let was_playing = self
            .audio_output
            .as_ref()
            .map(|output| output.playing())
            .unwrap_or(self.play_started_at.is_some());
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_playing(playing);
        }
        if self.audio_output.is_none() {
            if playing {
                if self.play_started_at.is_none() {
                    self.play_started_at = Some(Instant::now());
                }
            } else if let Some(started_at) = self.play_started_at.take() {
                self.paused_position = self.paused_position.saturating_add(started_at.elapsed());
            }
        }
        if video_debug_enabled() && was_playing != playing {
            video_debug!(
                "set_playing {} pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={}",
                playing,
                self.playback_position(),
                self.audio_buffered_duration(),
                self.video_buffered_duration(),
                self.ready_video_frames.len(),
                self.pending_video_packets.len()
            );
        }
    }

    fn clear_audio_buffer(&mut self) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.clear();
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_volume(volume);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_muted(muted);
        }
    }

    fn set_buffer_memory_limit_bytes(&mut self, bytes: u64) {
        self.buffer_memory_limit_bytes = bytes;
    }

    fn playback_position(&self) -> Duration {
        if let Some(audio_output) = self.audio_output.as_ref() {
            let elapsed = audio_output.position();
            return self.start_position.saturating_add(elapsed);
        }
        if let Some(started_at) = self.play_started_at {
            return self.paused_position.saturating_add(started_at.elapsed());
        }
        self.paused_position
    }

    fn maybe_enter_buffering(&self, shared: &BackendSharedState) {
        if self.should_keep_draining_eof() {
            return;
        }
        if !self.should_buffer() {
            return;
        }
        if !matches!(&shared.playback_state.get(), PlaybackState::Buffering) {
            video_debug!(
                "enter buffering pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={}",
                self.playback_position(),
                self.audio_buffered_duration(),
                self.video_buffered_duration(),
                self.ready_video_frames.len(),
                self.pending_video_packets.len()
            );
            shared.playback_state.set(PlaybackState::Buffering);
        }
    }

    fn should_buffer(&self) -> bool {
        let audio_starving = self
            .audio_output
            .as_ref()
            .map(|output| {
                output.buffered_duration() < self.buffering_profile.audio_starving_threshold
            })
            .unwrap_or(false);

        let video_starving = should_buffer_video(
            self.video_buffered_duration(),
            VIDEO_REBUFFER_ENTER_THRESHOLD,
            self.remaining_duration(),
        );

        should_buffer_for_rebuffer(
            audio_starving,
            video_starving,
            self.buffering_constrained_by_memory_limit(),
        )
    }

    fn present_next_video_frame(&mut self, shared: &BackendSharedState) -> Option<Duration> {
        let frame = self.ready_video_frames.pop_front()?;
        let position = frame.position;
        let texture = frame.texture;
        *self.latest_frame.lock().expect("video frame lock poisoned") = Some(texture.clone());
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: IntrinsicSize::from_pixels(
                self.video_decoder.width(),
                self.video_decoder.height(),
            ),
            texture: Some(texture),
            loading: false,
            error: None,
        });

        let mut metrics = shared.metrics.get();
        metrics.position = position;
        metrics.video_width = self.video_decoder.width();
        metrics.video_height = self.video_decoder.height();
        shared.metrics.set(metrics);
        self.last_presented_position = position;
        if self.play_started_at.is_none() {
            self.paused_position = position;
        }
        Some(position)
    }

    fn present_due_video_frames(&mut self, shared: &BackendSharedState) -> Option<Duration> {
        let mut last_position = None;
        let mut present_count = 0usize;
        let playback_before = self.playback_position();
        while let Some(frame) = self.ready_video_frames.front() {
            if !self.is_frame_due(frame.position) {
                break;
            }
            present_count += 1;
            last_position = self.present_next_video_frame(shared);
        }

        if video_debug_enabled() {
            if present_count > 1 {
                let pending_packet_memory = self.pending_video_packet_memory_bytes();
                let ready_frame_memory = self.ready_video_frame_memory_bytes();
                let audio_memory = self.audio_buffered_memory_bytes();
                let total_memory = total_buffered_memory_bytes(
                    pending_packet_memory,
                    ready_frame_memory,
                    audio_memory,
                );
                video_debug!(
                    "present burst count={} playback_before={:?} last_position={:?} ready_left={} packets_left={} compressed_buf={} ({:.2} MiB, packets={} / {:.2} MiB, frames={} / {:.2} MiB, audio={} / {:.2} MiB)",
                    present_count,
                    playback_before,
                    last_position,
                    self.ready_video_frames.len(),
                    self.pending_video_packets.len(),
                    total_memory,
                    bytes_to_mib(total_memory),
                    pending_packet_memory,
                    bytes_to_mib(pending_packet_memory),
                    ready_frame_memory,
                    bytes_to_mib(ready_frame_memory),
                    audio_memory,
                    bytes_to_mib(audio_memory)
                );
            } else if present_count == 0 {
                if let Some(next_frame) = self.ready_video_frames.front() {
                    let delta = next_frame.position.saturating_sub(playback_before);
                    if delta > Duration::from_millis(100)
                        && debug_log_due(
                            &mut self.last_debug_wait_log_at,
                            Duration::from_millis(250),
                        )
                    {
                        video_debug!(
                            "waiting next frame playback={:?} next_frame={:?} delta={:?} ready={} packets={} audio_started_clock={}",
                            playback_before,
                            next_frame.position,
                            delta,
                            self.ready_video_frames.len(),
                            self.pending_video_packets.len(),
                            self.audio_output
                                .as_ref()
                                .map(|output| output.has_started_clock())
                                .unwrap_or(self.play_started_at.is_some())
                        );
                    }
                }
            }
        }

        last_position
    }

    fn is_frame_due(&self, position: Duration) -> bool {
        if let Some(audio_output) = self.audio_output.as_ref() {
            if !audio_output.has_started_clock() {
                return false;
            }
            let playback_position = self.start_position.saturating_add(audio_output.position());
            return playback_position.saturating_add(VIDEO_PRESENT_TOLERANCE) >= position;
        }

        self.playback_position()
            .saturating_add(VIDEO_PRESENT_TOLERANCE)
            >= position
    }

    fn should_drop_video_preroll_frame(&self, position: Duration) -> bool {
        !self.start_position.is_zero()
            && position.saturating_add(VIDEO_SEEK_PREROLL_TOLERANCE) < self.start_position
    }

    fn video_buffered_duration_from(&self, baseline: Duration) -> Duration {
        furthest_video_buffer_end(&self.ready_video_frames, &self.pending_video_packets)
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    fn ready_video_buffered_duration(&self) -> Duration {
        let baseline = std::cmp::max(self.last_presented_position, self.playback_position());
        self.ready_video_frames
            .back()
            .map(|frame| frame.end_position.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    fn remaining_duration(&self) -> Option<Duration> {
        self.duration
            .map(|duration| duration.saturating_sub(self.playback_position()))
    }

    fn video_buffer_target_satisfied(&self, target: Duration) -> bool {
        video_buffer_target_satisfied(
            self.video_buffered_duration(),
            target,
            self.remaining_duration(),
            self.pending_video_packets.len() >= self.buffering_profile.video_max_packet_count,
        )
    }

    fn has_pending_media(&self) -> bool {
        !self.ready_video_frames.is_empty()
            || !self.pending_video_packets.is_empty()
            || self
                .audio_output
                .as_ref()
                .map(|output| !output.buffered_duration().is_zero())
                .unwrap_or(false)
    }

    fn should_keep_draining_eof(&self) -> bool {
        self.eof_sent && self.has_pending_media()
    }

    fn step(
        &mut self,
        shared: &BackendSharedState,
        command_rx: &Receiver<BackendCommand>,
        current_position: &mut Duration,
    ) -> StepOutcome {
        self.update_buffered_metrics(shared);

        self.maybe_log_state(shared, "step");

        let draining_eof = self.should_keep_draining_eof();

        if self.should_buffer() && !draining_eof {
            self.set_playing(false);
            if !matches!(shared.playback_state.get(), PlaybackState::Buffering) {
                video_debug!(
                    "step transition -> Buffering pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={}",
                    self.playback_position(),
                    self.audio_buffered_duration(),
                    self.video_buffered_duration(),
                    self.ready_video_frames.len(),
                    self.pending_video_packets.len()
                );
                shared.playback_state.set(PlaybackState::Buffering);
            }
        } else if matches!(
            shared.playback_state.get(),
            PlaybackState::Buffering | PlaybackState::Ready
        ) && (self.can_resume_playback() || draining_eof)
        {
            self.set_playing(true);

            video_debug!(
                "step transition -> Playing pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={} draining_eof={}",
                self.playback_position(),
                self.audio_buffered_duration(),
                self.video_buffered_duration(),
                self.ready_video_frames.len(),
                self.pending_video_packets.len(),
                draining_eof
            );

            shared.playback_state.set(PlaybackState::Playing);
        }

        if let Some(position) = self.present_due_video_frames(shared) {
            *current_position = position;
        }

        match self.fill_ready_video_frames(shared, Some(command_rx), true) {
            Ok(ReceiveVideoOutcome::Position(_)) => {}
            Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
            Err(error) => return StepOutcome::Error(error.to_string()),
        }

        if let Some(position) = self.present_due_video_frames(shared) {
            *current_position = position;
        }

        if self.should_throttle_demux() {
            if let Some(outcome) = self.process_commands(shared, command_rx, current_position) {
                return outcome;
            }
            thread::sleep(STEP_IDLE_SLEEP);
            return StepOutcome::Continue;
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets
                .next()
                .map(|(stream, packet)| (stream.index(), packet))
        };

        match next_packet {
            Some((stream_index, packet)) => {
                if let Some(outcome) = self.process_commands(shared, command_rx, current_position) {
                    return outcome;
                }

                if stream_index == self.video_stream_index {
                    self.queue_video_packet(packet);
                    match self.fill_ready_video_frames(shared, Some(command_rx), true) {
                        Ok(ReceiveVideoOutcome::Position(_)) => {}
                        Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                        Err(error) => return StepOutcome::Error(error.to_string()),
                    }
                } else if Some(stream_index) == self.audio_stream_index {
                    if let (
                        Some(audio_decoder),
                        Some(resampler),
                        Some(audio_time_base),
                        Some(audio_output),
                    ) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_time_base,
                        self.audio_output.as_ref(),
                    ) {
                        if let Err(error) = audio_decoder.send_packet(&packet) {
                            return StepOutcome::Error(format!(
                                "failed to send audio packet: {error}"
                            ));
                        }
                        self.pending_audio_compressed_bytes = self
                            .pending_audio_compressed_bytes
                            .saturating_add(packet.size() as u64);
                        if let Err(error) = receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_time_base,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        ) {
                            return StepOutcome::Error(error.to_string());
                        }
                    }
                }

                if let Some(position) = self.present_due_video_frames(shared) {
                    *current_position = position;
                    if !matches!(&shared.playback_state.get(), PlaybackState::Playing) {
                        shared.playback_state.set(PlaybackState::Playing);
                    }
                }
                self.maybe_enter_buffering(shared);

                StepOutcome::Continue
            }
            None => {
                if self.eof_sent {
                    match self.fill_ready_video_frames(shared, Some(command_rx), true) {
                        Ok(ReceiveVideoOutcome::Position(_)) => {}
                        Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                        Err(error) => return StepOutcome::Error(error.to_string()),
                    }
                    if self.pending_video_packets.is_empty()
                        && self.ready_video_frames.is_empty()
                        && self
                            .audio_output
                            .as_ref()
                            .map(|output| output.buffered_duration().is_zero())
                            .unwrap_or(true)
                    {
                        return StepOutcome::Ended(self.playback_position());
                    }
                    if let Some(position) = self.present_due_video_frames(shared) {
                        *current_position = position;
                    }
                    self.maybe_enter_buffering(shared);
                    thread::sleep(Duration::from_millis(4));
                    return StepOutcome::Continue;
                }

                if !self.pending_video_packets.is_empty() {
                    match self.fill_ready_video_frames(shared, Some(command_rx), true) {
                        Ok(ReceiveVideoOutcome::Position(_)) => {}
                        Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                        Err(error) => return StepOutcome::Error(error.to_string()),
                    }
                    if let Some(position) = self.present_due_video_frames(shared) {
                        *current_position = position;
                    }
                    self.maybe_enter_buffering(shared);
                    return StepOutcome::Continue;
                }

                self.eof_sent = true;
                if let Err(error) = self.video_decoder.send_eof() {
                    return StepOutcome::Error(format!("failed to flush video decoder: {error}"));
                }
                match self.fill_ready_video_frames(shared, Some(command_rx), true) {
                    Ok(ReceiveVideoOutcome::Position(_)) => {}
                    Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                    Err(error) => return StepOutcome::Error(error.to_string()),
                }

                if let (
                    Some(audio_decoder),
                    Some(resampler),
                    Some(audio_time_base),
                    Some(audio_output),
                ) = (
                    self.audio_decoder.as_mut(),
                    self.resampler.as_mut(),
                    self.audio_time_base,
                    self.audio_output.as_ref(),
                ) {
                    let _ = audio_decoder.send_eof();
                    if let Err(error) = receive_audio_frames(
                        audio_decoder,
                        resampler,
                        audio_time_base,
                        audio_output,
                        &mut self.pending_audio_compressed_bytes,
                    ) {
                        return StepOutcome::Error(error.to_string());
                    }
                    if let Err(error) = flush_audio_resampler(
                        resampler,
                        audio_output,
                        &mut self.pending_audio_compressed_bytes,
                    ) {
                        return StepOutcome::Error(error.to_string());
                    }
                }

                if let Some(position) = self.present_due_video_frames(shared) {
                    *current_position = position;
                }
                self.maybe_enter_buffering(shared);
                StepOutcome::Continue
            }
        }
    }

    fn process_commands(
        &mut self,
        shared: &BackendSharedState,
        command_rx: &Receiver<BackendCommand>,
        current_position: &mut Duration,
    ) -> Option<StepOutcome> {
        while let Ok(command) = command_rx.try_recv() {
            match command {
                BackendCommand::Play => {
                    if self.can_resume_playback() || self.should_keep_draining_eof() {
                        self.set_playing(true);

                        video_debug!(
                            "process play -> Playing pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={}",
                            self.playback_position(),
                            self.audio_buffered_duration(),
                            self.video_buffered_duration(),
                            self.ready_video_frames.len(),
                            self.pending_video_packets.len()
                        );

                        shared.playback_state.set(PlaybackState::Playing);
                    } else {
                        self.set_playing(false);

                        video_debug!(
                            "process play -> Buffering pos={:?} audio_buf={:?} video_buf={:?} ready={} packets={}",
                            self.playback_position(),
                            self.audio_buffered_duration(),
                            self.video_buffered_duration(),
                            self.ready_video_frames.len(),
                            self.pending_video_packets.len()
                        );

                        shared.playback_state.set(PlaybackState::Buffering);
                    }
                }
                BackendCommand::Pause => {
                    let position = self.playback_position();
                    video_debug!("process pause pos={:?}", position);
                    *current_position = position;
                    return Some(StepOutcome::Paused(position));
                }
                BackendCommand::Seek(position) => {
                    video_debug!("process seek restart target={:?}", position);
                    return Some(StepOutcome::Restart(position));
                }
                BackendCommand::SetVolume(volume) => self.set_volume(volume.clamp(0.0, 1.0)),
                BackendCommand::SetMuted(muted) => self.set_muted(muted),
                BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                    self.set_buffer_memory_limit_bytes(bytes)
                }
                BackendCommand::Load(source) => {
                    video_debug!("process load reload source={:?}", source);
                    video_debug!("process load reload source={:?}", source);
                    return Some(StepOutcome::Reload {
                        source,
                        position: Duration::ZERO,
                    });
                }
                BackendCommand::Shutdown => return Some(StepOutcome::Shutdown),
            }
        }
        None
    }

    fn receive_video_frames(
        &mut self,
        shared: &BackendSharedState,
        command_rx: Option<&Receiver<BackendCommand>>,
        respect_buffer_memory_limit: bool,
        decode_budget: &mut usize,
    ) -> Result<ReceiveVideoOutcome, TguiError> {
        let mut decoded = VideoFrame::empty();
        let mut last_position = None;
        let mut newly_decoded: Vec<QueuedVideoFrame> = Vec::new();

        while *decode_budget > 0
            && (!respect_buffer_memory_limit || !self.buffer_memory_limit_reached())
            && self.video_decoder.receive_frame(&mut decoded).is_ok()
        {
            let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                .unwrap_or_else(|| {
                    self.decoded_video_tail_position()
                        .unwrap_or(self.start_position)
                });

            if let Some(receiver) = command_rx {
                let mut command_position = position;
                if let Some(outcome) =
                    self.process_commands(shared, receiver, &mut command_position)
                {
                    match outcome {
                        StepOutcome::Restart(_)
                        | StepOutcome::Reload { .. }
                        | StepOutcome::Paused(_)
                        | StepOutcome::Ended(_)
                        | StepOutcome::Shutdown => {
                            return Ok(ReceiveVideoOutcome::Command(outcome));
                        }
                        StepOutcome::Continue => {}
                        StepOutcome::Error(error) => return Err(TguiError::Media(error)),
                    }
                }
            }

            if self.should_drop_video_preroll_frame(position) {
                continue;
            }

            if let Some(previous) = newly_decoded.last_mut() {
                if position > previous.position {
                    previous.end_position = position;
                    self.video_frame_duration = position.saturating_sub(previous.position);
                }
            } else if let Some(previous) = self.ready_video_frames.back_mut() {
                if position > previous.position {
                    previous.end_position = position;
                    self.video_frame_duration = position.saturating_sub(previous.position);
                }
            }

            let texture = Arc::new(video_frame_to_texture(&mut self.scaler, &decoded)?);
            newly_decoded.push(QueuedVideoFrame {
                position,
                end_position: position.saturating_add(self.video_frame_duration),
                texture,
                compressed_bytes: 0,
            });
            *decode_budget = (*decode_budget).saturating_sub(1);

            last_position = Some(position);
        }

        if !newly_decoded.is_empty() {
            let compressed_bytes = std::mem::take(&mut self.pending_video_compressed_bytes);
            distribute_video_compressed_bytes(&mut newly_decoded, compressed_bytes);
            self.ready_video_frames.extend(newly_decoded);
        }

        Ok(ReceiveVideoOutcome::Position(last_position))
    }

    fn audio_buffered_duration(&self) -> Duration {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    fn video_buffered_duration(&self) -> Duration {
        let baseline = std::cmp::max(self.last_presented_position, self.playback_position());
        self.video_buffered_duration_from(baseline)
    }

    fn startup_playback_blocked_by_memory_limit(&self) -> bool {
        startup_playback_blocked_by_memory_limit(
            self.buffering_constrained_by_memory_limit(),
            !self.ready_video_frames.is_empty(),
            self.audio_output.is_some(),
            self.audio_buffered_duration(),
        )
    }

    fn can_start_playback(&self) -> bool {
        let audio_ok = self.audio_output.is_none()
            || self.audio_buffered_duration() >= self.buffering_profile.start_buffer_target;
        let video_ok =
            self.video_buffer_target_satisfied(self.buffering_profile.video_start_buffer_target);
        (audio_ok && video_ok) || self.startup_playback_blocked_by_memory_limit()
    }

    fn can_resume_playback(&self) -> bool {
        let audio_ok = self.audio_output.is_none()
            || self.audio_buffered_duration() >= self.buffering_profile.rebuffer_target;
        let video_ok =
            self.video_buffer_target_satisfied(self.buffering_profile.video_resume_buffer_target);
        (audio_ok && video_ok) || self.startup_playback_blocked_by_memory_limit()
    }

    fn video_packet_send_error(&self, error: ffmpeg::Error) -> TguiError {
        if self.video_codec_id == codec::Id::AV1
            && matches!(
                error,
                ffmpeg::Error::Other {
                    errno: ffmpeg::error::ENOSYS
                }
            )
        {
            return TguiError::Media(format!(
                "AV1 is not supported by the linked FFmpeg build. The current decoder `{}` cannot decode this file on this platform. Rebuild/install FFmpeg with the `dav1d` or `aom` feature enabled in vcpkg.",
                self.video_decoder_name
            ));
        }

        TguiError::Media(format!("failed to send video packet: {error}"))
    }
}

fn buffering_profile_for_source(source: &VideoSource) -> BufferingProfile {
    match source {
        VideoSource::File(_) => LOCAL_BUFFERING_PROFILE,
        VideoSource::Url(_) => NETWORK_BUFFERING_PROFILE,
    }
}

fn stream_frame_duration(stream: &format::stream::Stream<'_>) -> Option<Duration> {
    rational_frame_duration(stream.avg_frame_rate())
        .or_else(|| rational_frame_duration(stream.rate()))
}

fn rational_frame_duration(rate: ffmpeg::Rational) -> Option<Duration> {
    let numerator = rate.numerator();
    let denominator = rate.denominator();
    if numerator <= 0 || denominator <= 0 {
        return None;
    }

    Some(Duration::from_secs_f64(
        denominator as f64 / numerator as f64,
    ))
}

fn http_input_options() -> ffmpeg::Dictionary<'static> {
    let mut options = ffmpeg::Dictionary::new();
    options.set("user_agent", concat!("tgui/", env!("CARGO_PKG_VERSION")));
    options.set("multiple_requests", "1");
    options.set("short_seek_size", "65536");
    options.set("reconnect", "1");
    options.set("reconnect_streamed", "1");
    options.set("reconnect_on_network_error", "1");
    options.set("reconnect_on_http_error", "4xx,5xx");
    options.set("reconnect_delay_max", "2");
    options.set("rw_timeout", "15000000");
    options
}

fn open_input(
    source: &VideoSource,
    source_url: &str,
) -> Result<format::context::Input, ffmpeg::Error> {
    match source {
        VideoSource::File(_) => format::input(&source_url),
        VideoSource::Url(_) => format::input_with_dictionary(&source_url, http_input_options()),
    }
}

fn open_video_decoder(
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
                    video_debug!(
                        "selected AV1 decoder name={} description={}",
                        codec.name(),
                        codec.description()
                    );
                    return Ok(OpenedVideoDecoder {
                        decoder,
                        codec_id,
                        decoder_name: codec.name().to_string(),
                    });
                }
                Err(error) => {
                    video_debug!(
                        "failed to open AV1 decoder name={} error={}",
                        codec.name(),
                        error
                    );
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
        video_debug!(
            "selected video decoder name={} description={}",
            codec.name(),
            codec.description()
        );
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

fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    _time_base: ffmpeg::Rational,
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

fn flush_audio_resampler(
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
    let total_samples = chunks.iter().map(|samples| samples.len() as u64).sum::<u64>().max(1);
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
        audio_output.push_samples(&samples, chunk_bytes);
    }
}

fn audio_frame_to_f32_if_any(frame: &AudioFrame) -> Option<Vec<f32>> {
    if frame.samples() == 0 {
        return None;
    }
    if !frame.is_packed() {
        return None;
    }

    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        Some(slice.to_vec())
    }
}

fn video_frame_to_texture(
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

fn pts_to_duration(timestamp: Option<i64>, time_base: ffmpeg::Rational) -> Option<Duration> {
    let timestamp = timestamp?;
    let numerator = time_base.numerator() as f64;
    let denominator = time_base.denominator() as f64;
    if denominator <= 0.0 {
        return None;
    }
    let seconds = timestamp as f64 * numerator / denominator;
    Some(Duration::from_secs_f64(seconds.max(0.0)))
}

fn packet_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

struct AudioOutput {
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
    fn new(volume: f32, muted: bool) -> Result<Self, TguiError> {
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

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::SeqCst);
        if !playing {
            self.shared.underflowing.store(false, Ordering::SeqCst);
        }
    }

    fn playing(&self) -> bool {
        self.shared.playing.load(Ordering::SeqCst)
    }

    fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::SeqCst);
    }

    fn clear(&self) {
        self.shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .clear();
        self.shared
            .compressed_chunks
            .lock()
            .expect("audio compressed queue lock poisoned")
            .clear();
        self.shared.played_frames.store(0, Ordering::SeqCst);
        self.shared.underflowing.store(false, Ordering::SeqCst);
    }

    fn push_samples(&self, samples: &[f32], compressed_bytes: u64) {
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

    fn position(&self) -> Duration {
        let played_frames = self.shared.played_frames.load(Ordering::SeqCst);
        Duration::from_secs_f64(played_frames as f64 / self.sample_rate as f64)
    }

    fn buffered_duration(&self) -> Duration {
        let buffered_samples = self
            .shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .len();
        let buffered_frames = buffered_samples / self.channels as usize;
        Duration::from_secs_f64(buffered_frames as f64 / self.sample_rate as f64)
    }

    fn buffered_memory_bytes(&self) -> u64 {
        self.shared
            .compressed_chunks
            .lock()
            .expect("audio compressed queue lock poisoned")
            .iter()
            .map(|chunk| chunk.compressed_bytes)
            .sum()
    }

    fn has_started_clock(&self) -> bool {
        self.shared.played_frames.load(Ordering::SeqCst) > 0
    }

    #[allow(dead_code)]
    fn is_underflowing(&self) -> bool {
        self.shared.underflowing.load(Ordering::SeqCst)
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

fn should_throttle_demux(
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

fn pending_video_packet_memory_bytes(pending_packets: &VecDeque<QueuedVideoPacket>) -> u64 {
    pending_packets
        .iter()
        .map(|packet| packet.packet.size() as u64)
        .sum()
}

fn ready_video_frame_memory_bytes(ready_frames: &VecDeque<QueuedVideoFrame>) -> u64 {
    ready_frames
        .iter()
        .map(|frame| frame.compressed_bytes)
        .sum()
}

fn total_buffered_memory_bytes(
    pending_video_packet_bytes: u64,
    ready_video_frame_bytes: u64,
    audio_buffered_bytes: u64,
) -> u64 {
    pending_video_packet_bytes
        .saturating_add(ready_video_frame_bytes)
        .saturating_add(audio_buffered_bytes)
}

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn startup_playback_blocked_by_memory_limit(
    buffering_constrained_by_memory_limit: bool,
    has_ready_video_frames: bool,
    has_audio_output: bool,
    audio_buffered_duration: Duration,
) -> bool {
    buffering_constrained_by_memory_limit
        && has_ready_video_frames
        && (!has_audio_output || !audio_buffered_duration.is_zero())
}

fn should_buffer_for_rebuffer(
    audio_starving: bool,
    video_starving: bool,
    buffering_constrained_by_memory_limit: bool,
) -> bool {
    audio_starving || (video_starving && !buffering_constrained_by_memory_limit)
}

fn buffering_constrained_by_memory_limit(
    total_buffered_memory_bytes: u64,
    buffer_memory_limit_bytes: u64,
    next_video_frame_memory_bytes: u64,
) -> bool {
    total_buffered_memory_bytes
        .saturating_add(next_video_frame_memory_bytes)
        > buffer_memory_limit_bytes
}

fn distribute_video_compressed_bytes(frames: &mut [QueuedVideoFrame], compressed_bytes: u64) {
    if frames.is_empty() {
        return;
    }
    let base = compressed_bytes / frames.len() as u64;
    let remainder = compressed_bytes % frames.len() as u64;
    for (index, frame) in frames.iter_mut().enumerate() {
        frame.compressed_bytes = base + u64::from(index < remainder as usize);
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

fn furthest_video_buffer_end(
    ready_frames: &VecDeque<QueuedVideoFrame>,
    pending_packets: &VecDeque<QueuedVideoPacket>,
) -> Option<Duration> {
    ready_frames
        .iter()
        .map(|frame| frame.end_position)
        .chain(pending_packets.iter().map(|packet| packet.end_position))
        .max()
}

fn video_buffer_target_satisfied(
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

fn should_buffer_video(
    buffered: Duration,
    threshold: Duration,
    remaining: Option<Duration>,
) -> bool {
    buffered < threshold
        && !remaining
            .map(|remaining| buffered.saturating_add(VIDEO_PRESENT_TOLERANCE) >= remaining)
            .unwrap_or(false)
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

    #[test]
    fn demux_keeps_running_below_memory_limit() {
        assert!(!should_throttle_demux(false, false, false, false));
    }

    #[test]
    fn demux_throttles_at_memory_limit() {
        assert!(should_throttle_demux(true, false, false, false));
    }

    #[test]
    fn demux_throttles_when_video_packet_fuse_trips() {
        assert!(should_throttle_demux(false, false, false, true));
    }

    #[test]
    fn demux_throttles_when_decoded_video_queue_is_too_deep() {
        assert!(should_throttle_demux(false, false, true, false));
    }

    #[test]
    fn demux_throttles_when_audio_queue_is_too_deep() {
        assert!(should_throttle_demux(false, true, false, false));
    }

    #[test]
    fn total_buffered_memory_counts_packets_frames_and_audio_samples() {
        let ready_frames = VecDeque::from([QueuedVideoFrame {
            position: Duration::from_millis(900),
            end_position: Duration::from_millis(1200),
            texture: Arc::new(TextureFrame::new(2, 2, vec![255; 16])),
            compressed_bytes: 16,
        }]);
        let mut packet = ffmpeg::Packet::copy(b"hello");
        packet.set_pts(Some(0));
        let pending_packets = VecDeque::from([QueuedVideoPacket {
            packet,
            position: Duration::ZERO,
            end_position: Duration::from_millis(33),
        }]);

        let total = total_buffered_memory_bytes(
            pending_video_packet_memory_bytes(&pending_packets),
            ready_video_frame_memory_bytes(&ready_frames),
            6,
        );

        assert_eq!(total, 27);
    }

    #[test]
    fn startup_playback_can_fall_back_when_memory_limit_prevents_more_buffering() {
        assert!(startup_playback_blocked_by_memory_limit(
            true,
            true,
            true,
            Duration::from_millis(1),
        ));
    }

    #[test]
    fn startup_playback_memory_fallback_requires_actual_playable_media() {
        assert!(!startup_playback_blocked_by_memory_limit(
            true,
            false,
            true,
            Duration::from_secs(1),
        ));
        assert!(!startup_playback_blocked_by_memory_limit(
            true,
            true,
            true,
            Duration::ZERO,
        ));
    }

    #[test]
    fn rebuffer_check_does_not_override_memory_limit_playback_fallback() {
        assert!(should_buffer_for_rebuffer(true, true, true));
        assert!(should_buffer_for_rebuffer(true, false, false));
        assert!(should_buffer_for_rebuffer(false, true, false));
        assert!(!should_buffer_for_rebuffer(false, true, true));
    }

    #[test]
    fn buffering_is_constrained_when_next_frame_would_overrun_limit() {
        assert!(buffering_constrained_by_memory_limit(
            165_888_000,
            167_772_160,
            3_686_400,
        ));
        assert!(!buffering_constrained_by_memory_limit(
            120_000_000,
            167_772_160,
            3_686_400,
        ));
    }

    #[test]
    fn then_like_estimate_fallback_does_not_divide_when_no_ready_frame_bytes_exist() {
        let ready_frames = VecDeque::from([QueuedVideoFrame {
            position: Duration::ZERO,
            end_position: Duration::from_millis(33),
            texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
            compressed_bytes: 0,
        }]);

        let (sum, count) = ready_frames
            .iter()
            .map(|frame| frame.compressed_bytes)
            .filter(|bytes| *bytes > 0)
            .fold((0u64, 0u64), |(sum, count), bytes| {
                (sum.saturating_add(bytes), count + 1)
            });

        assert_eq!((count > 0).then(|| sum / count), None);
    }

    #[test]
    fn video_buffer_target_accepts_remaining_tail() {
        assert!(video_buffer_target_satisfied(
            Duration::from_millis(900),
            Duration::from_secs(5),
            Some(Duration::from_millis(850)),
            false,
        ));
    }

    #[test]
    fn video_buffer_target_accepts_frame_cap_fallback() {
        assert!(video_buffer_target_satisfied(
            Duration::from_secs(2),
            Duration::from_secs(5),
            Some(Duration::from_secs(20)),
            true,
        ));
    }

    #[test]
    fn should_buffer_video_ignores_tail_section() {
        assert!(!should_buffer_video(
            Duration::from_millis(500),
            Duration::from_secs(2),
            Some(Duration::from_millis(450)),
        ));
        assert!(should_buffer_video(
            Duration::from_millis(500),
            Duration::from_secs(2),
            Some(Duration::from_secs(3)),
        ));
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
    fn url_sources_use_deeper_buffer_profile() {
        let profile = buffering_profile_for_source(&VideoSource::Url(
            "https://example.com/demo.mp4".to_string(),
        ));
        assert_eq!(profile, NETWORK_BUFFERING_PROFILE);
        assert_eq!(profile.video_start_buffer_target, Duration::from_secs(5));
        assert_eq!(profile.video_resume_buffer_target, Duration::from_secs(5));
        assert_eq!(profile.video_queue_high_water, Duration::from_secs(5));
        assert_eq!(profile.video_queue_hard_water, Duration::from_secs(6));
        assert!(profile.audio_queue_hard_water > LOCAL_BUFFERING_PROFILE.audio_queue_hard_water);
        assert!(
            profile.audio_starving_threshold > LOCAL_BUFFERING_PROFILE.audio_starving_threshold
        );
    }

    #[test]
    fn local_sources_use_shallower_video_startup_targets() {
        let profile = buffering_profile_for_source(&VideoSource::File("demo.mp4".into()));
        assert_eq!(profile, LOCAL_BUFFERING_PROFILE);
        assert_eq!(
            profile.video_start_buffer_target,
            Duration::from_millis(1500)
        );
        assert_eq!(
            profile.video_resume_buffer_target,
            Duration::from_millis(800)
        );
        assert_eq!(profile.video_queue_high_water, Duration::from_secs(3));
        assert_eq!(profile.video_queue_hard_water, Duration::from_secs(4));
        assert_eq!(profile.ready_video_frame_count, 4);
    }

    #[test]
    fn furthest_video_buffer_end_prefers_compressed_tail_over_small_ready_queue() {
        let ready_frames = VecDeque::from([QueuedVideoFrame {
            position: Duration::from_millis(900),
            end_position: Duration::from_millis(1200),
            texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
            compressed_bytes: 4,
        }]);
        let pending_packets = VecDeque::from([QueuedVideoPacket {
            packet: ffmpeg::Packet::empty(),
            position: Duration::from_millis(1200),
            end_position: Duration::from_millis(2600),
        }]);

        assert_eq!(
            furthest_video_buffer_end(&ready_frames, &pending_packets),
            Some(Duration::from_millis(2600))
        );
    }

    #[test]
    fn http_sources_enable_recovery_and_connection_reuse_options() {
        let options = http_input_options();
        assert_eq!(options.get("multiple_requests"), Some("1"));
        assert_eq!(options.get("short_seek_size"), Some("65536"));
        assert_eq!(options.get("reconnect"), Some("1"));
        assert_eq!(options.get("reconnect_streamed"), Some("1"));
        assert_eq!(options.get("reconnect_on_network_error"), Some("1"));
        assert_eq!(options.get("reconnect_on_http_error"), Some("4xx,5xx"));
        assert_eq!(options.get("rw_timeout"), Some("15000000"));
    }
}
