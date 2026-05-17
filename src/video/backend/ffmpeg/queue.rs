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
}

pub(super) struct SharedVideoQueue {
    accepted_generation: AtomicU64,
    pub(super) state: Mutex<VideoQueueState>,
    condvar: Condvar,
}

impl SharedVideoQueue {
    pub(super) fn new() -> Self {
        Self {
            accepted_generation: AtomicU64::new(0),
            state: Mutex::new(VideoQueueState::default()),
            condvar: Condvar::new(),
        }
    }

    pub(super) fn replace_generation(&self, generation: u64) {
        self.accepted_generation.store(generation, Ordering::SeqCst);
        self.clear_all();
    }

    pub(super) fn accepted_generation(&self) -> u64 {
        self.accepted_generation.load(Ordering::SeqCst)
    }

    pub(super) fn clear_all(&self) {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .clear();
        self.condvar.notify_all();
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

        let mut state = self.state.lock().expect("video queue lock poisoned");
        let accepted_generation = self.accepted_generation();
        state.frames.extend(
            frames
                .into_iter()
                .filter(|frame| frame.generation == accepted_generation),
        );
        drop(state);
        self.condvar.notify_all();
    }

    pub(super) fn pop_front_matching(&self, generation: u64) -> Option<QueuedVideoFrame> {
        let mut state = self.state.lock().expect("video queue lock poisoned");
        match state.frames.front() {
            Some(frame) if frame.generation == generation => state.frames.pop_front(),
            _ => None,
        }
    }

    pub(super) fn front(&self, generation: u64) -> Option<QueuedVideoFrame> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .find(|frame| frame.generation == generation)
            .cloned()
    }

    pub(super) fn has_frames(&self, generation: u64) -> bool {
        self.front(generation).is_some()
    }

    pub(super) fn ready_frame_count(&self, generation: u64) -> usize {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .filter(|frame| frame.generation == generation)
            .count()
    }

    pub(super) fn ready_memory_bytes(&self, generation: u64) -> u64 {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .filter(|frame| frame.generation == generation)
            .map(|frame| frame.compressed_bytes)
            .sum()
    }

    pub(super) fn tail_end_position(&self, generation: u64) -> Option<Duration> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .rev()
            .find(|frame| frame.generation == generation)
            .map(|frame| frame.end_position)
    }

    pub(super) fn head_frame_memory_bytes(&self, generation: u64) -> Option<u64> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .find(|frame| frame.generation == generation)
            .map(|frame| frame.compressed_bytes)
            .filter(|bytes| *bytes > 0)
    }
}

#[derive(Clone, Default)]
pub(super) struct SharedPlaybackClock {
    position_ns: Arc<AtomicU64>,
}

impl SharedPlaybackClock {
    pub(super) fn set_position(&self, position: Duration) {
        let nanos = position.as_nanos().min(u64::MAX as u128) as u64;
        self.position_ns.store(nanos, Ordering::SeqCst);
    }

    pub(super) fn position(&self) -> Duration {
        Duration::from_nanos(self.position_ns.load(Ordering::SeqCst))
    }
}

pub(super) fn clear_latest_frame(latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>) {
    *latest_frame.lock().expect("video frame lock poisoned") = None;
}
