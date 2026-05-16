use std::sync::{Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, unbounded, Receiver, Sender};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::util::frame::audio::Audio as AudioFrame;
use ffmpeg_next as ffmpeg;

use crate::foundation::error::TguiError;

use super::shared::{open_ffmpeg_input, AudioOutput, SharedAudioClock};
use super::{AudioBackend, BackendSharedState, DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES};
use crate::audio::{AudioMetrics, AudioSource, PlaybackState};

const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);
const METRICS_SYNC_INTERVAL: Duration = Duration::from_millis(100);

static FFMPEG_INIT: Once = Once::new();

pub(crate) struct FfmpegAudioBackend {
    command_tx: Sender<BackendCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegAudioBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg::init();
        });

        let (command_tx, command_rx) = unbounded();
        let worker = thread::spawn(move || {
            worker_main(command_rx, shared);
        });

        Self {
            command_tx,
            worker: Mutex::new(Some(worker)),
        }
    }
}

impl Drop for FfmpegAudioBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl AudioBackend for FfmpegAudioBackend {
    fn load(&self, source: AudioSource) -> Result<(), TguiError> {
        validate_audio_source(&source)?;
        self.command_tx
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("audio backend is unavailable".to_string()))
    }

    fn play(&self) {
        let _ = self.command_tx.send(BackendCommand::Play);
    }

    fn pause(&self) {
        let _ = self.command_tx.send(BackendCommand::Pause);
    }

    fn stop(&self) {
        let _ = self.command_tx.send(BackendCommand::Stop);
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

    fn set_looping(&self, looping: bool) {
        let _ = self.command_tx.send(BackendCommand::SetLooping(looping));
    }

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        let _ = self
            .command_tx
            .send(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);

        if let Some(worker) = self
            .worker
            .lock()
            .expect("audio worker lock poisoned")
            .take()
        {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Debug)]
enum BackendCommand {
    Load(AudioSource),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    SetLooping(bool),
    SetBufferMemoryLimitBytes(u64),
    Shutdown,
}

fn worker_main(command_rx: Receiver<BackendCommand>, shared: BackendSharedState) {
    let mut worker = AudioWorker::new(command_rx, shared);
    worker.run();
}

struct AudioWorker {
    command_rx: Receiver<BackendCommand>,
    shared: BackendSharedState,
    current_source: Option<AudioSource>,
    current_duration: Option<Duration>,
    should_play: bool,
    looping: bool,
    volume: f32,
    muted: bool,
    buffer_memory_limit_bytes: u64,
    session: Option<AudioSession>,
    last_metrics_sync_at: Option<Instant>,
}

impl AudioWorker {
    fn new(command_rx: Receiver<BackendCommand>, shared: BackendSharedState) -> Self {
        Self {
            command_rx,
            looping: shared.looping.get(),
            volume: shared.volume.get(),
            muted: shared.muted.get(),
            buffer_memory_limit_bytes: DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES,
            shared,
            current_source: None,
            current_duration: None,
            should_play: false,
            session: None,
            last_metrics_sync_at: None,
        }
    }

    fn run(&mut self) {
        loop {
            self.sync_metrics_if_due();

            let timeout = after(if self.session.is_some() {
                COMMAND_POLL_INTERVAL
            } else {
                Duration::from_millis(50)
            });

            select! {
                recv(self.command_rx) -> message => {
                    match message {
                        Ok(command) => {
                            if !self.handle_command(command) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                recv(timeout) -> _ => {}
            }

            let outcome = if let Some(session) = self.session.as_mut() {
                session.step(self.buffer_memory_limit_bytes)
            } else {
                continue;
            };

            match outcome {
                Ok(SessionStep::Continue) => {}
                Ok(SessionStep::Idle) => {
                    thread::sleep(STEP_IDLE_SLEEP);
                }
                Ok(SessionStep::EofDrained) => {
                    if self.looping {
                        let should_play = self.should_play;
                        self.reopen_current_source(Duration::ZERO, should_play);
                    } else {
                        self.should_play = false;
                        self.sync_metrics(true);
                        self.shared.playback_state.set(PlaybackState::Ended);
                    }
                }
                Err(error) => {
                    self.session = None;
                    self.last_metrics_sync_at = None;
                    self.shared.set_error(error.to_string());
                }
            }
        }
    }

    fn handle_command(&mut self, command: BackendCommand) -> bool {
        match command {
            BackendCommand::Load(source) => {
                self.current_source = Some(source.clone());
                self.should_play = false;
                self.reopen_source(source, Duration::ZERO, false);
            }
            BackendCommand::Play => {
                if self.shared.playback_state.get() == PlaybackState::Ended {
                    self.reopen_current_source(Duration::ZERO, true);
                    return true;
                }
                self.should_play = true;
                if let Some(session) = self.session.as_ref() {
                    session.set_playing(true);
                }
                if self.session.is_some() {
                    self.sync_metrics(true);
                    self.shared.playback_state.set(PlaybackState::Playing);
                }
            }
            BackendCommand::Pause => {
                self.should_play = false;
                if let Some(session) = self.session.as_ref() {
                    session.set_playing(false);
                    self.sync_metrics(true);
                    self.shared.playback_state.set(PlaybackState::Paused);
                }
            }
            BackendCommand::Stop => {
                self.should_play = false;
                self.current_duration = None;
                self.session = None;
                self.last_metrics_sync_at = None;
                self.shared.reset_for_stop();
            }
            BackendCommand::Seek(position) => {
                self.reopen_current_source(position, self.should_play);
            }
            BackendCommand::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(session) = self.session.as_ref() {
                    session.set_volume(self.volume);
                }
            }
            BackendCommand::SetMuted(muted) => {
                self.muted = muted;
                if let Some(session) = self.session.as_ref() {
                    session.set_muted(muted);
                }
            }
            BackendCommand::SetLooping(looping) => {
                self.looping = looping;
            }
            BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.buffer_memory_limit_bytes = bytes;
            }
            BackendCommand::Shutdown => return false,
        }

        true
    }

    fn reopen_current_source(&mut self, position: Duration, should_play: bool) {
        let Some(source) = self.current_source.clone() else {
            return;
        };
        self.reopen_source(source, position, should_play);
    }

    fn reopen_source(&mut self, source: AudioSource, position: Duration, should_play: bool) {
        self.should_play = should_play;
        match AudioSession::open(
            source.clone(),
            position,
            self.volume,
            self.muted,
            should_play,
        ) {
            Ok(session) => {
                self.current_duration = session.duration();
                self.session = Some(session);
                if should_play {
                    self.shared.playback_state.set(PlaybackState::Playing);
                } else if position.is_zero() {
                    self.shared.set_ready();
                } else {
                    self.shared.playback_state.set(PlaybackState::Paused);
                    self.shared.snapshot.set(crate::audio::AudioSnapshot {
                        loading: false,
                        error: None,
                    });
                    self.shared.error.set(None);
                }
            }
            Err(error) => {
                self.session = None;
                self.last_metrics_sync_at = None;
                self.shared.set_error(error.to_string());
            }
        }
    }

    fn sync_metrics_if_due(&mut self) {
        let should_sync = self
            .last_metrics_sync_at
            .map(|last| last.elapsed() >= METRICS_SYNC_INTERVAL)
            .unwrap_or(true);
        if should_sync {
            self.sync_metrics(false);
        }
    }

    fn sync_metrics(&mut self, force: bool) {
        if !self.shared.metrics_enabled() {
            return;
        }

        let Some(session) = self.session.as_ref() else {
            return;
        };

        if !force
            && self
                .last_metrics_sync_at
                .is_some_and(|last| last.elapsed() < METRICS_SYNC_INTERVAL)
        {
            return;
        }

        self.shared.metrics.set(AudioMetrics {
            duration: self.current_duration,
            position: session.position(),
            buffered: Some(session.buffered_position()),
        });
        self.last_metrics_sync_at = Some(Instant::now());
    }
}

enum SessionStep {
    Continue,
    Idle,
    EofDrained,
}

struct AudioSession {
    input: format::context::Input,
    start_position: Duration,
    duration: Option<Duration>,
    audio_stream_index: usize,
    audio_decoder: ffmpeg::decoder::Audio,
    resampler: Resampler,
    audio_output: AudioOutput,
    audio_clock: SharedAudioClock,
    eof_sent: bool,
    eof_drained: bool,
    queue_hard_water: Duration,
}

impl AudioSession {
    fn open(
        source: AudioSource,
        start_position: Duration,
        volume: f32,
        muted: bool,
        playing: bool,
    ) -> Result<Self, TguiError> {
        let source_url = match &source {
            AudioSource::File(path) => path
                .to_str()
                .ok_or_else(|| TguiError::Media("audio path is not valid UTF-8".to_string()))?
                .to_string(),
            AudioSource::Url { url, .. } => url.clone(),
        };
        let headers = match &source {
            AudioSource::File(_) => None,
            AudioSource::Url { headers, .. } => Some(headers.as_slice()),
        };
        let mut input = open_ffmpeg_input("audio", &source_url, headers)?;

        if !start_position.is_zero() {
            let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
            input.seek(timestamp, ..timestamp).map_err(|error| {
                TguiError::Media(format!("failed to seek audio source: {error}"))
            })?;
        }

        let audio_stream = input
            .streams()
            .best(media::Type::Audio)
            .ok_or_else(|| TguiError::Media("audio stream not found".to_string()))?;
        let audio_stream_index = audio_stream.index();
        let duration = stream_duration(audio_stream.duration(), audio_stream.time_base());
        let audio_context = codec::context::Context::from_parameters(audio_stream.parameters())
            .map_err(|error| TguiError::Media(format!("failed to open audio codec: {error}")))?;
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

        let audio_output = AudioOutput::new(volume, muted, "tgui-audio")
            .map_err(|error| TguiError::Media(format!("failed to create audio output: {error}")))?;
        let audio_clock = audio_output.clock_handle();
        let resampler = Resampler::get(
            audio_decoder.format(),
            audio_decoder.channel_layout(),
            audio_decoder.rate(),
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::default(audio_output.channels().into()),
            audio_output.sample_rate(),
        )
        .map_err(|error| TguiError::Media(format!("failed to create audio resampler: {error}")))?;

        let mut session = Self {
            input,
            start_position,
            duration,
            audio_stream_index,
            audio_decoder,
            resampler,
            audio_output,
            audio_clock,
            eof_sent: false,
            eof_drained: false,
            queue_hard_water: match source {
                AudioSource::File(_) => LOCAL_AUDIO_QUEUE_HARD_WATER,
                AudioSource::Url { .. } => NETWORK_AUDIO_QUEUE_HARD_WATER,
            },
        };
        session.audio_output.set_playing(playing);
        session.prime_initial_audio()?;
        Ok(session)
    }

    fn duration(&self) -> Option<Duration> {
        self.duration
    }

    fn position(&self) -> Duration {
        self.start_position
            .saturating_add(self.audio_clock.position())
    }

    fn buffered_position(&self) -> Duration {
        self.position()
            .saturating_add(self.audio_clock.buffered_duration())
    }

    fn set_playing(&self, playing: bool) {
        self.audio_output.set_playing(playing);
    }

    fn set_volume(&self, volume: f32) {
        self.audio_output.set_volume(volume);
    }

    fn set_muted(&self, muted: bool) {
        self.audio_output.set_muted(muted);
    }

    fn prime_initial_audio(&mut self) -> Result<(), TguiError> {
        while !self.eof_sent && self.audio_clock.buffered_duration() < Duration::from_millis(200) {
            match self.step(u64::MAX)? {
                SessionStep::Continue => {}
                SessionStep::Idle | SessionStep::EofDrained => break,
            }
        }
        Ok(())
    }

    fn step(&mut self, buffer_memory_limit_bytes: u64) -> Result<SessionStep, TguiError> {
        if self.eof_sent && self.audio_clock.buffered_duration().is_zero() {
            if self.eof_drained {
                return Ok(SessionStep::Idle);
            }
            self.eof_drained = true;
            return Ok(SessionStep::EofDrained);
        }

        if self.audio_clock.buffered_duration() >= self.queue_hard_water
            || self.audio_clock.buffered_memory_bytes() >= buffer_memory_limit_bytes
        {
            return Ok(SessionStep::Idle);
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets
                .next()
                .map(|(stream, packet)| (stream.index(), packet))
        };

        match next_packet {
            Some((stream_index, packet)) => {
                if stream_index == self.audio_stream_index {
                    self.audio_decoder.send_packet(&packet).map_err(|error| {
                        TguiError::Media(format!("failed to send audio packet: {error}"))
                    })?;
                    receive_audio_frames(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        packet.size() as u64,
                    )?;
                }
                Ok(SessionStep::Continue)
            }
            None => {
                if !self.eof_sent {
                    self.eof_sent = true;
                    let _ = self.audio_decoder.send_eof();
                    receive_audio_frames(
                        &mut self.audio_decoder,
                        &mut self.resampler,
                        &self.audio_output,
                        0,
                    )?;
                    flush_audio_resampler(&mut self.resampler, &self.audio_output)?;
                }
                Ok(if self.audio_clock.buffered_duration().is_zero() {
                    SessionStep::EofDrained
                } else {
                    SessionStep::Idle
                })
            }
        }
    }
}

fn validate_audio_source(source: &AudioSource) -> Result<(), TguiError> {
    match source {
        AudioSource::File(_) => Ok(()),
        AudioSource::Url { headers, .. } => {
            super::shared::validate_ffmpeg_headers("audio", headers)
        }
    }
}

fn receive_audio_frames(
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

fn flush_audio_resampler(
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

    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        Some(slice.to_vec())
    }
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

fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    fn test_shared(ctx: &ViewModelContext) -> BackendSharedState {
        BackendSharedState {
            playback_state: ctx.state(PlaybackState::Idle),
            metrics: ctx.state(AudioMetrics::default()),
            volume: ctx.state(1.0),
            muted: ctx.state(false),
            looping: ctx.state(false),
            metrics_observed: Arc::new(AtomicBool::new(false)),
            buffer_memory_limit_bytes: ctx.state(DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES),
            error: ctx.state(None),
            snapshot: ctx.state(crate::audio::AudioSnapshot::default()),
        }
    }

    #[test]
    fn play_after_ended_reopens_from_start_when_looping_disabled() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let (_tx, rx) = unbounded();
        let mut worker = AudioWorker::new(rx, shared.clone());
        worker.current_source = Some(AudioSource::File("demo.mp3".into()));
        worker.shared.playback_state.set(PlaybackState::Ended);

        assert!(worker.handle_command(BackendCommand::Play));

        assert!(worker.should_play);
    }

    #[test]
    fn stop_clears_session_and_resets_shared_state() {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let (_tx, rx) = unbounded();
        let mut worker = AudioWorker::new(rx, shared.clone());
        worker.current_source = Some(AudioSource::File("demo.mp3".into()));
        worker.current_duration = Some(Duration::from_secs(30));
        worker.should_play = true;
        worker.shared.playback_state.set(PlaybackState::Playing);

        assert!(worker.handle_command(BackendCommand::Stop));

        assert!(worker.session.is_none());
        assert_eq!(worker.shared.playback_state.get(), PlaybackState::Idle);
        assert_eq!(worker.shared.metrics.get(), AudioMetrics::default());
    }
}
