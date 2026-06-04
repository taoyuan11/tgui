//! 逐帧顶点缓冲池。
//!
//! 旧实现里 `prepare_commands` 对每个 draw call 都 `create_buffer_init` 新建一个
//! `wgpu::Buffer`——一帧几百次 GPU 驱动级分配。绝大多数 draw call 的顶点数据极小
//! （矩形/纹理/文字 4-6 个顶点，~100-200 字节），为这么小的数据各建一个 buffer 浪费严重。
//!
//! 这里改为：一帧内把所有顶点数据 bump-allocate 进一段 CPU staging，最后一次
//! `write_buffer` 整体上传到池缓冲；每个 prepared command 只记录 `(offset, count)`。
//!
//! GPU 同步：池为 triple-buffered（3 个轮转缓冲，匹配 swapchain 在途帧数）。
//! 第 N 帧用的缓冲要等到第 N+3 帧才会被复用，此时 GPU 早已读完第 N 帧的数据，
//! 因此不存在“写入正在被 GPU 读取的内存”的竞态。

/// 池中轮转缓冲的数量。3 足以覆盖典型的在途帧数（双/三缓冲 swapchain）。
const POOL_FRAME_COUNT: usize = 3;

/// 池缓冲的初始容量（字节）。约 64KB，足够容纳一帧数百个小四边形的顶点。
const INITIAL_CAPACITY: u64 = 64 * 1024;

/// 顶点对齐要求。wgpu 的 `set_vertex_buffer` 偏移必须满足 `COPY_BUFFER_ALIGNMENT`（4 字节），
/// 我们用 4 字节对齐每次分配，保证任意 offset 都是合法的顶点缓冲起点。
const ALIGNMENT: u64 = wgpu::COPY_BUFFER_ALIGNMENT;

pub(super) struct VertexBufferPool {
    /// 轮转缓冲。每帧 `begin_frame` 推进 `current`。
    buffers: [wgpu::Buffer; POOL_FRAME_COUNT],
    /// 各缓冲的当前容量（字节）。增长时单独替换对应槽位。
    capacities: [u64; POOL_FRAME_COUNT],
    /// 当前帧使用的缓冲槽位。
    current: usize,
    /// 本帧的 CPU 端 staging，bump-allocate 写入，帧末一次性上传。
    staging: Vec<u8>,
}

impl VertexBufferPool {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let buffers = std::array::from_fn(|_| Self::create_buffer(device, INITIAL_CAPACITY));
        Self {
            buffers,
            capacities: [INITIAL_CAPACITY; POOL_FRAME_COUNT],
            current: 0,
            staging: Vec::with_capacity(INITIAL_CAPACITY as usize),
        }
    }

    fn create_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tgui-vertex-pool"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// 开始新一帧：推进到下一个轮转缓冲并清空 staging。
    pub(super) fn begin_frame(&mut self) {
        self.current = (self.current + 1) % POOL_FRAME_COUNT;
        self.staging.clear();
    }

    /// 把一段顶点字节 bump-allocate 进本帧 staging，返回其在池缓冲中的字节偏移。
    /// 偏移按 `ALIGNMENT` 对齐，保证可直接用作 `set_vertex_buffer` 的起点。
    pub(super) fn allocate(&mut self, bytes: &[u8]) -> u64 {
        let offset = self.staging.len() as u64;
        let aligned = offset.div_ceil(ALIGNMENT) * ALIGNMENT;
        if aligned > offset {
            self.staging.resize(aligned as usize, 0);
        }
        let start = self.staging.len() as u64;
        self.staging.extend_from_slice(bytes);
        start
    }

    /// 帧末把整段 staging 上传到当前轮转缓冲。若 staging 超过缓冲容量则先扩容。
    pub(super) fn flush(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.staging.is_empty() {
            return;
        }
        // staging 长度可能不是 COPY_BUFFER_ALIGNMENT 的倍数；write_buffer 要求长度对齐。
        let write_len = self.staging.len() as u64;
        let write_len = write_len.div_ceil(ALIGNMENT) * ALIGNMENT;
        if write_len > self.staging.len() as u64 {
            self.staging.resize(write_len as usize, 0);
        }
        if write_len > self.capacities[self.current] {
            // 扩容到 2 的幂上界，避免抖动时反复重建。
            let new_cap = write_len.next_power_of_two().max(INITIAL_CAPACITY);
            self.buffers[self.current] = Self::create_buffer(device, new_cap);
            self.capacities[self.current] = new_cap;
        }
        queue.write_buffer(&self.buffers[self.current], 0, &self.staging);
    }

    /// 当前帧的池缓冲，draw 阶段按 `allocate` 返回的偏移取 slice。
    pub(super) fn current_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.current]
    }
}
