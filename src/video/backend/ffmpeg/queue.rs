use super::*;

#[derive(Clone)]
pub(super) struct QueuedVideoFrame {
    pub(super) generation: u64,
    pub(super) position: Duration,
    pub(super) end_position: Duration,
    pub(super) texture: Arc<TextureFrame>,
    pub(super) compressed_bytes: u64,
}

#[derive(Default)]
pub(super) struct VideoQueueState {
    pub(super) frames: VecDeque<QueuedVideoFrame>,
    /// `frames` 中的总 compressed_bytes 之和，避免每次询问时再做线性扫描。
    pub(super) total_compressed_bytes: u64,
    /// `frames` 队尾帧的 end_position 缓存。frames 为空时为 `None`。
    pub(super) tail_end_position: Option<Duration>,
}

pub(super) struct SharedVideoQueue {
    accepted_generation: AtomicU64,
    pub(super) state: Mutex<VideoQueueState>,
}

impl SharedVideoQueue {
    pub(super) fn new() -> Self {
        Self {
            accepted_generation: AtomicU64::new(0),
            state: Mutex::new(VideoQueueState::default()),
        }
    }

    pub(super) fn replace_generation(&self, generation: u64) {
        self.accepted_generation
            .store(generation, Ordering::Release);
        self.clear_all();
    }

    pub(super) fn accepted_generation(&self) -> u64 {
        self.accepted_generation.load(Ordering::Acquire)
    }

    pub(super) fn clear_all(&self) {
        let mut state = self.state.lock();
        state.frames.clear();
        state.total_compressed_bytes = 0;
        state.tail_end_position = None;
    }

    pub(super) fn push_frames(&self, mut frames: Vec<QueuedVideoFrame>) {
        if frames.is_empty() {
            return;
        }

        let accepted_generation = self.accepted_generation();
        frames.retain(|frame| frame.generation == accepted_generation);
        if frames.is_empty() {
            return;
        }

        let mut state = self.state.lock();
        let accepted_generation = self.accepted_generation();
        for frame in frames {
            if frame.generation != accepted_generation {
                continue;
            }
            state.total_compressed_bytes = state
                .total_compressed_bytes
                .saturating_add(frame.compressed_bytes);
            state.tail_end_position = Some(frame.end_position);
            state.frames.push_back(frame);
        }
    }

    pub(super) fn pop_front_matching(&self, generation: u64) -> Option<QueuedVideoFrame> {
        let mut state = self.state.lock();
        match state.frames.front() {
            Some(frame) if frame.generation == generation => {
                let popped = state.frames.pop_front();
                if let Some(frame) = popped.as_ref() {
                    state.total_compressed_bytes = state
                        .total_compressed_bytes
                        .saturating_sub(frame.compressed_bytes);
                    if state.frames.is_empty() {
                        state.tail_end_position = None;
                    }
                }
                popped
            }
            _ => None,
        }
    }

    pub(super) fn front(&self, generation: u64) -> Option<QueuedVideoFrame> {
        let state = self.state.lock();
        match state.frames.front() {
            Some(frame) if frame.generation == generation => Some(frame.clone()),
            _ => None,
        }
    }

    pub(super) fn has_frames(&self, generation: u64) -> bool {
        if generation != self.accepted_generation() {
            return false;
        }
        let state = self.state.lock();
        state
            .frames
            .front()
            .is_some_and(|frame| frame.generation == generation)
    }

    pub(super) fn ready_frame_count(&self, generation: u64) -> usize {
        if generation != self.accepted_generation() {
            return 0;
        }
        let state = self.state.lock();
        state.frames.len()
    }

    pub(super) fn ready_memory_bytes(&self, generation: u64) -> u64 {
        if generation != self.accepted_generation() {
            return 0;
        }
        let state = self.state.lock();
        state.total_compressed_bytes
    }

    pub(super) fn tail_end_position(&self, generation: u64) -> Option<Duration> {
        if generation != self.accepted_generation() {
            return None;
        }
        let state = self.state.lock();
        state.tail_end_position
    }

    pub(super) fn head_frame_memory_bytes(&self, generation: u64) -> Option<u64> {
        if generation != self.accepted_generation() {
            return None;
        }
        let state = self.state.lock();
        let frame = state.frames.front()?;
        if frame.generation != generation {
            return None;
        }
        let bytes = frame.compressed_bytes;
        (bytes > 0).then_some(bytes)
    }
}

#[derive(Clone, Default)]
pub(super) struct SharedPlaybackClock {
    position_ns: Arc<AtomicU64>,
}

impl SharedPlaybackClock {
    pub(super) fn set_position(&self, position: Duration) {
        let nanos = position.as_nanos().min(u64::MAX as u128) as u64;
        self.position_ns.store(nanos, Ordering::Relaxed);
    }

    pub(super) fn position(&self) -> Duration {
        Duration::from_nanos(self.position_ns.load(Ordering::Relaxed))
    }
}

pub(super) fn clear_latest_frame(latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>) {
    *latest_frame.lock() = None;
}
