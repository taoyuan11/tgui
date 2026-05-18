use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Once, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScalingFlags};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::{audio::Audio as AudioFrame, video::Video as VideoFrame};
use ffmpeg_next as ffmpeg;
use parking_lot::Mutex;

#[cfg(test)]
use crate::audio::backend::shared::ffmpeg_http_input_options;
use crate::audio::backend::shared::{
    open_ffmpeg_input, validate_ffmpeg_headers, AudioOutput, SharedAudioClock,
};
use crate::foundation::error::TguiError;
use crate::media::{IntrinsicSize, RasterRequest, TextureFrame};
use crate::video::{PlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot};

use super::{BackendSharedState, VideoBackend};

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

static FFMPEG_INIT: Once = Once::new();
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
    command_tx: Sender<BackendCommand>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    present_worker: Mutex<Option<JoinHandle<()>>>,
    decode_worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegVideoBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg::init();
        });

        let (backend_tx, backend_rx) = unbounded();
        let (decode_tx, decode_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let latest_frame = Arc::new(Mutex::new(None));
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();

        let decode_queue = shared_queue.clone();
        let decode_clock = playback_clock.clone();
        let decode_worker = thread::spawn(move || {
            decode_main(decode_rx, event_tx, decode_queue, decode_clock);
        });

        let present_latest = latest_frame.clone();
        let present_queue = shared_queue;
        let present_clock = playback_clock;
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

        Self {
            command_tx: backend_tx,
            latest_frame,
            present_worker: Mutex::new(Some(present_worker)),
            decode_worker: Mutex::new(Some(decode_worker)),
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

    fn set_target_raster(&self, raster: Option<RasterRequest>) {
        let _ = self
            .command_tx
            .send(BackendCommand::SetTargetRaster(raster));
    }

    fn current_frame(&self) -> Option<Arc<TextureFrame>> {
        self.latest_frame.lock().clone()
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);

        if let Some(worker) = self.present_worker.lock().take() {
            let _ = worker.join();
        }

        if let Some(worker) = self.decode_worker.lock().take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Debug)]
enum BackendCommand {
    Load(VideoSource),
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
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
    SetBufferMemoryLimitBytes(u64),
    SetTargetRaster(Option<RasterRequest>),
    Shutdown,
}

#[derive(Clone)]
enum DecodeEvent {
    StreamOpened(StreamOpenedEvent),
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
    use super::*;

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
    fn queue_drops_stale_generation_frames() {
        let queue = SharedVideoQueue::new();
        queue.replace_generation(2);
        queue.push_frames(vec![
            QueuedVideoFrame {
                generation: 1,
                position: Duration::ZERO,
                end_position: Duration::from_millis(33),
                texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
                compressed_bytes: 4,
            },
            QueuedVideoFrame {
                generation: 2,
                position: Duration::from_millis(33),
                end_position: Duration::from_millis(66),
                texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
                compressed_bytes: 4,
            },
        ]);

        assert_eq!(queue.ready_frame_count(1), 0);
        assert_eq!(queue.ready_frame_count(2), 1);
    }
}
