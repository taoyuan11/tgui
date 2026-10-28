use std::ops::{Deref, DerefMut};
use std::time::Instant;

use super::*;

// Bound the entire open/read operation, including FFmpeg reconnect backoff and
// Retry-After, rather than granting a fresh timeout for every reconnect attempt.
const NETWORK_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_INVALID_PACKETS: usize = 32;

pub(super) fn http_input_options(
    source: &VideoSource,
) -> Result<ffmpeg::Dictionary<'static>, TguiError> {
    let VideoSource::Url { headers, .. } = source else {
        return Ok(ffmpeg::Dictionary::new());
    };
    let mut options = ffmpeg_http_input_options("video", headers)?;
    // Authentication failures and missing resources cannot recover by retrying.
    options.set("reconnect_on_http_error", "408,429,500,502,503,504");
    options.set("reconnect_delay_max", "4");
    options.set("reconnect_max_retries", "4");
    options.set("reconnect_delay_total_max", "15");
    Ok(options)
}

struct InputInterrupt {
    queue: Arc<SharedVideoQueue>,
    generation: u64,
    deadline: Mutex<Instant>,
}

impl InputInterrupt {
    fn interrupted(&self) -> bool {
        self.queue.accepted_generation() != self.generation
            || Instant::now() >= *self.deadline.lock()
    }

    fn reset_deadline(&self) {
        *self.deadline.lock() = Instant::now() + NETWORK_OPERATION_TIMEOUT;
    }
}

pub(super) enum PacketRead {
    Packet(ffmpeg::Packet),
    Retry,
    Eof,
}

pub(super) struct VideoInput {
    input: format::context::Input,
    // Keep byte-backed input alive until after the FFmpeg context is closed.
    _resource: Option<TemporaryMediaFile>,
    interrupt: Arc<InputInterrupt>,
    retrying: bool,
    invalid_packets: usize,
}

impl Deref for VideoInput {
    type Target = format::context::Input;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl DerefMut for VideoInput {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.input
    }
}

impl VideoInput {
    pub(super) fn read_packet(&mut self) -> Result<PacketRead, TguiError> {
        if !self.retrying {
            self.interrupt.reset_deadline();
        }
        if self.interrupt.interrupted() {
            return Err(TguiError::Media(
                "video input cancelled or timed out".into(),
            ));
        }
        // PacketIter erases terminal I/O errors into None, which would falsely
        // flush the decoder and publish Ended after a network failure.
        let mut packet = ffmpeg::Packet::empty();
        let result = packet.read(&mut self.input);
        self.handle_read_result(result, packet)
    }

    fn handle_read_result(
        &mut self,
        result: Result<(), ffmpeg::Error>,
        packet: ffmpeg::Packet,
    ) -> Result<PacketRead, TguiError> {
        match result {
            Ok(()) => {
                self.retrying = false;
                self.invalid_packets = 0;
                Ok(PacketRead::Packet(packet))
            }
            Err(ffmpeg::Error::Eof) => Ok(PacketRead::Eof),
            Err(ffmpeg::Error::Other {
                errno: ffmpeg::error::EAGAIN,
            }) => {
                self.retrying = true;
                Ok(PacketRead::Retry)
            }
            Err(ffmpeg::Error::InvalidData) if self.invalid_packets < MAX_INVALID_PACKETS => {
                self.invalid_packets += 1;
                self.retrying = true;
                Ok(PacketRead::Retry)
            }
            Err(error) => Err(TguiError::Media(format!(
                "failed to read video packet: {error}"
            ))),
        }
    }
}

pub(super) fn open_input(
    source: &VideoSource,
    queue: Arc<SharedVideoQueue>,
    generation: u64,
) -> Result<VideoInput, TguiError> {
    let (source_url, resource) = match source {
        VideoSource::File(path) => (media_path_to_url("video", path)?, None),
        VideoSource::Url { url, .. } => (url.clone(), None),
        VideoSource::Bytes { bytes, extension } => {
            let file = create_temporary_media_file("video", bytes, extension.as_deref())?;
            (media_path_to_url("video", file.path())?, Some(file))
        }
    };
    let interrupt = Arc::new(InputInterrupt {
        queue,
        generation,
        deadline: Mutex::new(Instant::now() + NETWORK_OPERATION_TIMEOUT),
    });
    let callback = interrupt.clone();
    let input = format::input_with_interrupt_and_dictionary(
        &source_url,
        move || callback.interrupted(),
        http_input_options(source)?,
    )
    .map_err(|error| TguiError::Media(format!("failed to open video source: {error}")))?;
    Ok(VideoInput {
        input,
        _resource: resource,
        interrupt,
        retrying: false,
        invalid_packets: 0,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::thread::JoinHandle;

    use super::*;

    // PCM needs no external fixture, encoder executable, or audio device.
    fn wav_bytes() -> Vec<u8> {
        let data_len = 256_000u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(data_len + 36).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(b"\x02\0\x10\0data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend((0..data_len).map(|index| (index % 251) as u8));
        bytes
    }

    struct TestServer {
        url: String,
        stop: Arc<AtomicBool>,
        requests: Arc<AtomicU64>,
        worker: Option<JoinHandle<()>>,
    }

    impl TestServer {
        fn new(handler: impl Fn(TcpStream, usize, &AtomicBool) + Send + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}/video.wav", listener.local_addr().unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(AtomicU64::new(0));
            let stopped = stop.clone();
            let count = requests.clone();
            let worker = thread::spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            stream
                                .set_write_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let index = count.fetch_add(1, Ordering::AcqRel) as usize;
                            handler(stream, index, &stopped);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("test server: {error}"),
                    }
                }
            });
            Self {
                url,
                stop,
                requests,
                worker: Some(worker),
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    fn request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut byte = [0];
        while bytes.len() < 16_384 && !bytes.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            bytes.push(byte[0]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn serve_wav(mut stream: TcpStream, request: &str, truncate: bool) {
        let bytes = wav_bytes();
        let offset = request
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("range: bytes=")
                    .and_then(|range| range.split('-').next()?.parse::<usize>().ok())
            })
            .unwrap_or(0)
            .min(bytes.len());
        let header = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/wav\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            offset, bytes.len() - 1, bytes.len(), bytes.len() - offset,
        );
        let _ = stream.write_all(header.as_bytes());
        let end = if truncate {
            (offset + 65_536).min(bytes.len())
        } else {
            bytes.len()
        };
        let _ = stream.write_all(&bytes[offset..end]);
    }

    fn open_test_input(source: VideoSource) -> VideoInput {
        ensure_ffmpeg_initialized().unwrap();
        open_input(&source, Arc::new(SharedVideoQueue::new()), 0).unwrap()
    }

    #[test]
    fn packet_errors_are_distinct_from_eof_and_corruption_retries_are_bounded() {
        let mut input = open_test_input(VideoSource::bytes_with_extension(wav_bytes(), "wav"));
        assert!(matches!(
            input.handle_read_result(Err(ffmpeg::Error::Eof), ffmpeg::Packet::empty()),
            Ok(PacketRead::Eof)
        ));
        for error in [
            ffmpeg::Error::Exit,
            ffmpeg::Error::Other {
                errno: ffmpeg::error::ETIMEDOUT,
            },
            ffmpeg::Error::Other {
                errno: ffmpeg::error::EIO,
            },
        ] {
            assert!(input
                .handle_read_result(Err(error), ffmpeg::Packet::empty())
                .is_err());
        }
        assert!(matches!(
            input.handle_read_result(
                Err(ffmpeg::Error::Other {
                    errno: ffmpeg::error::EAGAIN
                }),
                ffmpeg::Packet::empty()
            ),
            Ok(PacketRead::Retry)
        ));
        *input.interrupt.deadline.lock() = Instant::now();
        assert!(
            input.read_packet().is_err(),
            "EAGAIN must not reset the retry deadline"
        );
        for _ in 0..MAX_INVALID_PACKETS {
            assert!(matches!(
                input.handle_read_result(Err(ffmpeg::Error::InvalidData), ffmpeg::Packet::empty()),
                Ok(PacketRead::Retry)
            ));
        }
        assert!(input
            .handle_read_result(Err(ffmpeg::Error::InvalidData), ffmpeg::Packet::empty())
            .is_err());
        assert!(matches!(
            input.handle_read_result(Ok(()), ffmpeg::Packet::empty()),
            Ok(PacketRead::Packet(_))
        ));
        assert!(!input.retrying);
        assert_eq!(input.invalid_packets, 0);
    }

    #[test]
    fn http_recovers_from_service_unavailable_and_preserves_headers() {
        let server = TestServer::new(|mut stream, attempt, _| {
            let request = request(&mut stream);
            assert!(request.contains("X-Video-Test: retained\r\n"));
            if attempt == 0 {
                stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                serve_wav(stream, &request, false);
            }
        });
        let mut input =
            open_test_input(VideoSource::url(&server.url).with_header("X-Video-Test", "retained"));
        assert!(matches!(
            input.read_packet().unwrap(),
            PacketRead::Packet(_)
        ));
        assert!(server.requests.load(Ordering::Acquire) >= 2);
    }

    #[test]
    fn http_disconnect_resumes_at_byte_offset_and_drains_to_real_eof() {
        let server = TestServer::new(|mut stream, attempt, _| {
            let request = request(&mut stream);
            serve_wav(stream, &request, attempt == 0);
        });
        let mut input = open_test_input(VideoSource::url(&server.url));
        let mut received_bytes = Vec::new();
        loop {
            match input.read_packet().unwrap() {
                PacketRead::Packet(packet) => {
                    received_bytes.extend_from_slice(packet.data().unwrap())
                }
                PacketRead::Retry => thread::sleep(STEP_IDLE_SLEEP),
                PacketRead::Eof => break,
            }
        }
        assert!(
            received_bytes == wav_bytes()[44..],
            "reconnect must neither lose nor replay media bytes"
        );
        assert!(server.requests.load(Ordering::Acquire) >= 2);
    }

    #[test]
    fn http_missing_resource_fails_without_retrying() {
        let server = TestServer::new(|mut stream, _, _| {
            request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        ensure_ffmpeg_initialized().unwrap();
        assert!(open_input(
            &VideoSource::url(&server.url),
            Arc::new(SharedVideoQueue::new()),
            0
        )
        .is_err());
        assert_eq!(server.requests.load(Ordering::Acquire), 1);
    }

    #[test]
    fn superseding_generation_interrupts_stalled_http_open() {
        let server = TestServer::new(|mut stream, _, stop| {
            request(&mut stream);
            while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(5));
            }
        });
        ensure_ffmpeg_initialized().unwrap();
        let queue = Arc::new(SharedVideoQueue::new());
        let callback_queue = queue.clone();
        let source = VideoSource::url(&server.url);
        let opener = thread::spawn(move || open_input(&source, callback_queue, 0).is_err());
        let started = Instant::now();
        while server.requests.load(Ordering::Acquire) == 0
            && started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        queue.replace_generation(1);
        assert!(opener.join().unwrap());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "cancel should bypass the 15-second socket timeout"
        );
    }

    #[test]
    fn controller_can_replace_stalled_network_source_and_play_video_to_end() {
        use crate::animation::AnimationCoordinator;
        use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
        use crate::video::VideoController;

        let server = TestServer::new(|mut stream, _, stop| {
            request(&mut stream);
            while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(5));
            }
        });
        let ctx = ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default());
        let controller = VideoController::new(&ctx);
        controller.load(VideoSource::url(&server.url)).unwrap();
        let started = Instant::now();
        while server.requests.load(Ordering::Acquire) == 0
            && started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(server.requests.load(Ordering::Acquire) > 0);

        // Tiny raw video exercises real decode/presentation without a window,
        // GPU, audio device, or external ffmpeg executable.
        let mut video = b"YUV4MPEG2 W2 H2 F25:1 Ip A1:1 C420jpeg\n".to_vec();
        for _ in 0..10 {
            video.extend_from_slice(b"FRAME\n\x80\x80\x80\x80\x80\x80");
        }
        controller
            .load(VideoSource::bytes_with_extension(video, "y4m"))
            .unwrap();
        let state = controller.playback_state();
        let replaced_at = Instant::now();
        while state.get() == VideoPlaybackState::Loading
            && replaced_at.elapsed() < Duration::from_secs(3)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(state.get(), VideoPlaybackState::Ready);
        assert_eq!(
            controller.video_size().get(),
            VideoSize {
                width: 2,
                height: 2
            }
        );
        controller.play();
        while state.get() != VideoPlaybackState::Ended
            && replaced_at.elapsed() < Duration::from_secs(3)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(state.get(), VideoPlaybackState::Ended);
        drop(controller);
    }
}
