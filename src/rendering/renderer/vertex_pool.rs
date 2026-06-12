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
//!
//! Phase 3（`incremental-upload`）：flush 时把本帧 staging 与「该轮转缓冲上次写入的字节」
//! 做 diff，只 `write_buffer` 变化区间，相同则跳过。因为每个轮转缓冲只被自己 3 帧前的
//! 内容占用，对它做部分覆盖安全；且整写与区间写后缓冲内容逐字节一致，渲染结果不变。
//! 配合 Phase 1 splice，单属性改动时每帧只上传那一段顶点。关闭特性时退化为整段上传（现状）。

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
    /// Phase 3：各轮转缓冲「上次写入到 GPU 的字节内容」镜像。flush 时与本帧 staging 做
    /// 字节级 diff，只上传变化区间。缓冲被扩容重建、或上次 flush 跳过时，对应镜像清空
    /// （视作全新缓冲，下次必然走整段上传），保证镜像与 GPU 内容严格一致。
    #[cfg(feature = "incremental-upload")]
    last_uploaded: [Vec<u8>; POOL_FRAME_COUNT],
}

impl VertexBufferPool {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let buffers = std::array::from_fn(|_| Self::create_buffer(device, INITIAL_CAPACITY));
        Self {
            buffers,
            capacities: [INITIAL_CAPACITY; POOL_FRAME_COUNT],
            current: 0,
            staging: Vec::with_capacity(INITIAL_CAPACITY as usize),
            #[cfg(feature = "incremental-upload")]
            last_uploaded: std::array::from_fn(|_| Vec::new()),
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
            // 空帧：不上传。镜像不变（缓冲内容也不变），下帧 diff 仍以镜像为基准。
            return;
        }
        // staging 长度可能不是 COPY_BUFFER_ALIGNMENT 的倍数；write_buffer 要求长度对齐。
        let write_len = self.staging.len() as u64;
        let write_len = write_len.div_ceil(ALIGNMENT) * ALIGNMENT;
        if write_len > self.staging.len() as u64 {
            self.staging.resize(write_len as usize, 0);
        }
        let mut grew = false;
        if write_len > self.capacities[self.current] {
            // 扩容到 2 的幂上界，避免抖动时反复重建。
            let new_cap = write_len.next_power_of_two().max(INITIAL_CAPACITY);
            self.buffers[self.current] = Self::create_buffer(device, new_cap);
            self.capacities[self.current] = new_cap;
            grew = true;
        }

        #[cfg(feature = "incremental-upload")]
        {
            // 新建的缓冲 GPU 内容未定义，必须整段写并重置镜像；否则与上次写入做字节级 diff，
            // 只上传变化区间（按 ALIGNMENT 对齐）。无论整写还是区间写，写后缓冲 [0..write_len]
            // 都与 staging 逐字节一致 —— 渲染结果不变。
            let dirty = if grew {
                Some((0u64, write_len))
            } else {
                Self::dirty_range(&self.staging, &self.last_uploaded[self.current]).map(
                    |(start, end)| {
                        let aligned_start = (start as u64) / ALIGNMENT * ALIGNMENT;
                        let aligned_end = (end as u64).div_ceil(ALIGNMENT) * ALIGNMENT;
                        (aligned_start, aligned_end.min(write_len))
                    },
                )
            };
            if let Some((start, end)) = dirty {
                if end > start {
                    queue.write_buffer(
                        &self.buffers[self.current],
                        start,
                        &self.staging[start as usize..end as usize],
                    );
                }
            }
            // 镜像精确记录我们保证写入 GPU 缓冲 [0..write_len] 的内容。
            self.last_uploaded[self.current].clear();
            self.last_uploaded[self.current].extend_from_slice(&self.staging);
            return;
        }

        #[cfg(not(feature = "incremental-upload"))]
        {
            let _ = grew;
            queue.write_buffer(&self.buffers[self.current], 0, &self.staging);
        }
    }

    /// 计算 `new` 相对 `old` 的变化字节区间 `[start, end)`（未对齐，相对 `new` 起点）。
    /// 完全相同（含长度相同）返回 `None`（调用方跳过上传）。长度不同的尾部计入变化区间：
    /// `new` 更长时尾部是新增内容必须写入；`new` 更短时多余的 GPU 旧字节不会被 draw 读取，
    /// 故 `end` 不超过 `new.len()`。纯函数，便于单测穷举边界。
    #[cfg(feature = "incremental-upload")]
    fn dirty_range(new: &[u8], old: &[u8]) -> Option<(usize, usize)> {
        let common = new.len().min(old.len());
        let mut first = None;
        let mut last = 0usize;
        for i in 0..common {
            if new[i] != old[i] {
                if first.is_none() {
                    first = Some(i);
                }
                last = i + 1;
            }
        }
        if new.len() > old.len() {
            // 新增尾部 [old.len()..new.len()) 必写。
            let start = first.unwrap_or(old.len());
            return Some((start, new.len()));
        }
        // 长度相等或变短：变化只可能落在 common 区间内。
        first.map(|start| (start, last))
    }

    /// 当前帧的池缓冲，draw 阶段按 `allocate` 返回的偏移取 slice。
    pub(super) fn current_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.current]
    }
}

#[cfg(all(test, feature = "incremental-upload"))]
mod dirty_range_tests {
    use super::VertexBufferPool;

    #[test]
    fn identical_same_length_is_none() {
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 2, 3, 4], &[1, 2, 3, 4]),
            None
        );
    }

    #[test]
    fn empty_old_writes_whole_new() {
        // 全新缓冲镜像为空：整段视作变化。
        assert_eq!(VertexBufferPool::dirty_range(&[1, 2, 3], &[]), Some((0, 3)));
    }

    #[test]
    fn single_middle_byte_change() {
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 2, 9, 4, 5], &[1, 2, 3, 4, 5]),
            Some((2, 3))
        );
    }

    #[test]
    fn span_covers_first_to_last_diff_inclusive() {
        // 变化在 index 1 和 4：区间 [1,5)，中间相同字节也被包含（单次连续写）。
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 9, 3, 4, 9, 6], &[1, 2, 3, 4, 5, 6]),
            Some((1, 5))
        );
    }

    #[test]
    fn growth_appends_tail_from_first_diff_or_old_len() {
        // 前缀相同、变长：从旧长度处起的新增尾部必写。
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 2, 3, 4, 5], &[1, 2, 3]),
            Some((3, 5))
        );
        // 前缀也有变化、且变长：从更靠前的首个 diff 起一直写到新末尾。
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 9, 3, 4, 5], &[1, 2, 3]),
            Some((1, 5))
        );
    }

    #[test]
    fn shrink_change_within_common_only() {
        // 变短：多余的旧 GPU 字节不会被 draw 读取，end 不超过 new.len()。
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 9, 3], &[1, 2, 3, 4, 5]),
            Some((1, 2))
        );
    }

    #[test]
    fn shrink_with_identical_common_prefix_is_none() {
        // 变短但公共前缀完全相同：new 区间内无变化 → 跳过上传（多余旧字节不被读取）。
        assert_eq!(
            VertexBufferPool::dirty_range(&[1, 2, 3], &[1, 2, 3, 4, 5]),
            None
        );
    }
}
