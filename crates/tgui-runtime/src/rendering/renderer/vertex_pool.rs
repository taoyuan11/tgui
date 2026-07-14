//! 逐帧顶点缓冲池。
//!
//! 旧实现里 `prepare_commands` 对每个 draw call 都 `create_buffer_init` 新建一个
//! `wgpu::Buffer`——一帧几百次 GPU 驱动级分配。绝大多数 draw call 的顶点数据极小
//! （矩形/纹理/文字 4-6 个顶点，~100-200 字节），为这么小的数据各建一个 buffer 浪费严重。
//!
//! 这里改为：一帧内把所有顶点数据 bump-allocate 进一段 CPU staging，帧末批量上传到
//! 池缓冲；每个 prepared command 只记录 `(offset, count)`。
//!
//! GPU 同步：池为 triple-buffered（3 个轮转缓冲，匹配 swapchain 在途帧数）。
//! 第 N 帧用的缓冲要等到第 N+3 帧才会被复用，此时 GPU 早已读完第 N 帧的数据，
//! 因此不存在“写入正在被 GPU 读取的内存”的竞态。
//!
//! flush 时把本帧 staging 与「该轮转缓冲上次写入的字节」
//! 分块做 diff，只 `write_buffer` 变化区间，相同则跳过。相近变化会合并，区间过多或
//! 脏数据占比过高时退回整段写，避免为省少量带宽制造过多 queue 操作。因为每个轮转缓冲
//! 只被自己 3 帧前的内容占用，对它做部分覆盖安全；且整写与区间写后缓冲内容逐字节一致，
//! 渲染结果不变。
//! 配合 scene splice，单属性改动时每帧只上传那一段顶点。

use std::ops::Range;

use smallvec::SmallVec;

/// 池中轮转缓冲的数量。3 足以覆盖典型的在途帧数（双/三缓冲 swapchain）。
const POOL_FRAME_COUNT: usize = 3;

/// 池缓冲的初始容量（字节）。约 64KB，足够容纳一帧数百个小四边形的顶点。
const INITIAL_CAPACITY: u64 = 64 * 1024;

/// 顶点对齐要求。wgpu 的 `set_vertex_buffer` 偏移必须满足 `COPY_BUFFER_ALIGNMENT`（4 字节），
/// 我们用 4 字节对齐每次分配，保证任意 offset 都是合法的顶点缓冲起点。
const ALIGNMENT: u64 = wgpu::COPY_BUFFER_ALIGNMENT;

/// 先按块比较，完整相同的块不再逐字节扫描。256B 足以摊薄 slice 比较开销，同时不会让
/// 单个脏块的精确扫描范围过大。
const DIFF_SCAN_BLOCK_BYTES: usize = 256;

/// 两段变化之间只有少量相同数据时合并上传。一次额外 `Queue::write_buffer` 的固定成本
/// 远高于复制几十字节，64B 是偏保守的折中。
const MERGE_GAP_BYTES: usize = 64;

/// 小量局部变化才值得拆成多次 queue 写；超过该数量直接整段上传。
const MAX_DIRTY_RANGES: usize = 8;

/// 对齐、合并后的脏字节达到整段的 1/2 时，整写通常比多次局部写更划算。
const FULL_UPLOAD_DIRTY_NUMERATOR: usize = 1;
const FULL_UPLOAD_DIRTY_DENOMINATOR: usize = 2;

type DirtyRanges = SmallVec<[Range<usize>; MAX_DIRTY_RANGES]>;

pub(super) struct VertexBufferPool {
    /// 轮转缓冲。每帧 `begin_frame` 推进 `current`。
    buffers: [wgpu::Buffer; POOL_FRAME_COUNT],
    /// 各缓冲的当前容量（字节）。增长时单独替换对应槽位。
    capacities: [u64; POOL_FRAME_COUNT],
    /// 当前帧使用的缓冲槽位。
    current: usize,
    /// 本帧的 CPU 端 staging，bump-allocate 写入，帧末一次性上传。
    staging: Vec<u8>,
    /// 各轮转缓冲「上次写入到 GPU 的有效字节内容」镜像。flush 时与本帧 staging 做
    /// 字节级 diff，只上传变化区间；缓冲扩容重建时强制整段上传。镜像也按相同区间 patch，
    /// 始终与 GPU 中下一帧会读取的有效前缀严格一致。
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
        self.allocate_aligned(bytes, ALIGNMENT)
    }

    /// 为场景 draw 分配顶点数据，并使起点同时对齐到该 pipeline 的顶点 stride。
    ///
    /// scene pass 只绑定一次完整池缓冲，各 draw 通过 `first_vertex = offset / stride`
    /// 定位到自己的子区间。因此这里的 stride 对齐是该等价变换的前置条件。
    pub(super) fn allocate_aligned(&mut self, bytes: &[u8], alignment: u64) -> u64 {
        debug_assert!(alignment >= ALIGNMENT);
        debug_assert_eq!(alignment % ALIGNMENT, 0);
        let offset = self.staging.len() as u64;
        let aligned = Self::aligned_offset(offset, alignment);
        if aligned > offset {
            self.staging.resize(aligned as usize, 0);
        }
        let start = self.staging.len() as u64;
        self.staging.extend_from_slice(bytes);
        start
    }

    fn aligned_offset(offset: u64, alignment: u64) -> u64 {
        offset.div_ceil(alignment) * alignment
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

        // 新建的缓冲 GPU 内容未定义，必须整段写；否则与该槽位三帧前的镜像分块 diff，
        // 上传少量对齐后的变化区间。无论整写还是区间写，写后缓冲 [0..write_len] 都与
        // staging 逐字节一致 —— 渲染结果不变。
        let dirty_ranges = if grew {
            Self::full_range(self.staging.len())
        } else {
            Self::plan_dirty_ranges(&self.staging, &self.last_uploaded[self.current])
        };
        for range in &dirty_ranges {
            queue.write_buffer(
                &self.buffers[self.current],
                range.start as u64,
                &self.staging[range.clone()],
            );
        }

        // 镜像也只 patch 相同区间，避免局部视觉变化时仍整段 CPU copy。变短但公共前缀
        // 未变化时没有 GPU 写，truncate 后镜像仍精确描述下次会被读取的有效前缀。
        let mirror = &mut self.last_uploaded[self.current];
        mirror.resize(self.staging.len(), 0);
        for range in dirty_ranges {
            mirror[range.clone()].copy_from_slice(&self.staging[range]);
        }
    }

    fn full_range(len: usize) -> DirtyRanges {
        let mut ranges = DirtyRanges::new();
        if len > 0 {
            ranges.push(0..len);
        }
        ranges
    }

    /// 规划 `new` 相对 `old` 的对齐上传区间。完整相同返回空；相隔较远的局部变化保留为
    /// 多段，相近变化合并；脏比例或区间数超过阈值则返回单个整段区间。
    ///
    /// `new` 是 flush 前已按 `COPY_BUFFER_ALIGNMENT` 补齐的 staging。`new` 更短时，旧 GPU
    /// 尾部不会被 draw 读取，无需清零；更长时新增尾部一定计入脏区间。
    fn plan_dirty_ranges(new: &[u8], old: &[u8]) -> DirtyRanges {
        debug_assert_eq!(new.len() % ALIGNMENT as usize, 0);
        if new.is_empty() {
            return DirtyRanges::new();
        }

        let common = new.len().min(old.len());
        let mut ranges = DirtyRanges::new();

        for block_start in (0..common).step_by(DIFF_SCAN_BLOCK_BYTES) {
            let block_end = (block_start + DIFF_SCAN_BLOCK_BYTES).min(common);
            if new[block_start..block_end] == old[block_start..block_end] {
                continue;
            }

            // 只在确定有变化的块内找精确连续 run；完整相同的绝大多数块走上面的快跳过。
            let mut index = block_start;
            while index < block_end {
                while index < block_end && new[index] == old[index] {
                    index += 1;
                }
                let start = index;
                while index < block_end && new[index] != old[index] {
                    index += 1;
                }
                if start < index && Self::push_dirty_range(&mut ranges, start..index, new.len()) {
                    return Self::full_range(new.len());
                }
            }
        }

        if new.len() > old.len() {
            // 新增尾部 [old.len()..new.len()) 必写，并允许与前一个局部变化合并。
            if Self::push_dirty_range(&mut ranges, old.len()..new.len(), new.len()) {
                return Self::full_range(new.len());
            }
        }

        let dirty_bytes = ranges.iter().map(|range| range.len()).sum::<usize>();
        if dirty_bytes.saturating_mul(FULL_UPLOAD_DIRTY_DENOMINATOR)
            >= new.len().saturating_mul(FULL_UPLOAD_DIRTY_NUMERATOR)
        {
            return Self::full_range(new.len());
        }

        ranges
    }

    /// 插入并对齐一段变化；返回 `true` 表示新增后会超过多区间上限，应整段上传。
    fn push_dirty_range(ranges: &mut DirtyRanges, range: Range<usize>, new_len: usize) -> bool {
        let alignment = ALIGNMENT as usize;
        let start = range.start / alignment * alignment;
        let end = range.end.div_ceil(alignment) * alignment;
        let end = end.min(new_len);
        if start >= end {
            return false;
        }

        if let Some(last) = ranges.last_mut() {
            if start <= last.end.saturating_add(MERGE_GAP_BYTES) {
                last.end = last.end.max(end);
                return false;
            }
        }

        if ranges.len() == MAX_DIRTY_RANGES {
            return true;
        }
        ranges.push(start..end);
        false
    }

    /// 当前帧的池缓冲。scene pass 整段绑定一次，效果 pass 可按需取 slice。
    pub(super) fn current_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.current]
    }
}

#[cfg(test)]
mod dirty_range_tests {
    use super::{VertexBufferPool, ALIGNMENT, MAX_DIRTY_RANGES, MERGE_GAP_BYTES};
    use crate::rendering::renderer::{BrushVertex, MeshVertex, RectVertex, TextVertex};

    fn ranges(new: &[u8], old: &[u8]) -> Vec<std::ops::Range<usize>> {
        VertexBufferPool::plan_dirty_ranges(new, old).into_vec()
    }

    #[test]
    fn mixed_vertex_strides_produce_draw_addressable_offsets() {
        // 模拟 Rect/Brush/Mesh/Text 块交错分配。每块起点必须能被当前
        // pipeline 的 stride 整除，才能在整段 buffer 只绑定一次后用 first_vertex 定位。
        let rect_stride = std::mem::size_of::<RectVertex>() as u64;
        let brush_stride = std::mem::size_of::<BrushVertex>() as u64;
        let mesh_stride = std::mem::size_of::<MeshVertex>() as u64;
        let text_stride = std::mem::size_of::<TextVertex>() as u64;
        let blocks = [
            (6 * rect_stride, rect_stride),
            (6 * brush_stride, brush_stride),
            (18 * mesh_stride, mesh_stride),
            (6 * text_stride, text_stride),
            (6 * rect_stride, rect_stride),
        ];
        let mut len = 0_u64;
        let mut padding = 0_u64;
        for (byte_len, stride) in blocks {
            assert_eq!(stride % ALIGNMENT, 0);
            let offset = VertexBufferPool::aligned_offset(len, stride);
            assert_eq!(offset % stride, 0);
            padding += offset - len;
            len = offset + byte_len;
        }

        // 对齐只会在块之间插入小量零 padding，不会改变任何块的字节内容。
        assert!(padding > 0);
        assert!(padding < blocks.iter().map(|(_, stride)| *stride).sum());
    }

    #[test]
    fn aligned_padding_survives_dirty_upload_planning_rotation_and_growth() {
        let strides = [
            std::mem::size_of::<RectVertex>() as u64,
            std::mem::size_of::<BrushVertex>() as u64,
            std::mem::size_of::<MeshVertex>() as u64,
            std::mem::size_of::<TextVertex>() as u64,
        ];
        let mut mirrors = [Vec::new(), Vec::new(), Vec::new()];
        let mut gpu = [Vec::new(), Vec::new(), Vec::new()];

        for frame in 0..90_usize {
            let slot = (frame + 1) % 3;
            let mut staging = Vec::new();
            // 不同 frame 改变块数和大小，同时覆盖轮转复用、增长、缩短以及
            // 只改一小段的局部上传。块间 padding 使用 allocate_aligned 同样的零填充。
            let block_count = 4 + frame % 13;
            for block in 0..block_count {
                let stride = strides[(frame + block) % strides.len()];
                let offset = VertexBufferPool::aligned_offset(staging.len() as u64, stride);
                staging.resize(offset as usize, 0);
                let vertex_count = 3 + (frame + block * 5) % 19;
                let fill = ((block * 31 + frame / 3) & 0xff) as u8;
                staging.resize(staging.len() + stride as usize * vertex_count, fill);
                assert_eq!(offset % stride, 0);
            }
            let aligned_len = VertexBufferPool::aligned_offset(staging.len() as u64, ALIGNMENT);
            staging.resize(aligned_len as usize, 0);

            let planned = VertexBufferPool::plan_dirty_ranges(&staging, &mirrors[slot]);
            gpu[slot].resize(gpu[slot].len().max(staging.len()), 0);
            mirrors[slot].resize(staging.len(), 0);
            for range in planned {
                gpu[slot][range.clone()].copy_from_slice(&staging[range.clone()]);
                mirrors[slot][range.clone()].copy_from_slice(&staging[range]);
            }

            assert_eq!(&gpu[slot][..staging.len()], staging.as_slice());
            assert_eq!(mirrors[slot], staging);
        }
    }

    #[test]
    fn identical_same_length_has_no_uploads() {
        assert!(ranges(&[1, 2, 3, 4], &[1, 2, 3, 4]).is_empty());
    }

    #[test]
    fn empty_old_writes_whole_new() {
        // 全新缓冲镜像为空：整段视作变化。
        assert_eq!(ranges(&[1, 2, 3, 0], &[]), vec![0..4]);
    }

    #[test]
    fn single_middle_byte_change_is_copy_aligned() {
        let old = vec![0; 1024];
        let mut new = old.clone();
        new[513] = 9;
        assert_eq!(ranges(&new, &old), vec![512..516]);
    }

    #[test]
    fn distant_local_changes_stay_as_multiple_ranges() {
        let old = vec![0; 4096];
        let mut new = old.clone();
        new[9] = 1;
        new[3074] = 2;
        assert_eq!(ranges(&new, &old), vec![8..12, 3072..3076]);
    }

    #[test]
    fn nearby_changes_merge_but_larger_gaps_do_not() {
        let old = vec![0; 1024];
        let mut new = old.clone();
        new[8] = 1;
        new[12 + MERGE_GAP_BYTES] = 2;
        new[512] = 3;
        assert_eq!(ranges(&new, &old), vec![8..80, 512..516]);
    }

    #[test]
    fn growth_writes_new_tail_and_keeps_distant_prefix_change_separate() {
        let old = vec![0; 4096];
        let mut new = vec![0; 4608];
        new[8] = 1;
        assert_eq!(ranges(&new, &old), vec![8..12, 4096..4608]);
    }

    #[test]
    fn shrink_with_identical_common_prefix_is_none() {
        // 变短但公共前缀完全相同：new 区间内无变化 → 跳过上传（多余旧字节不被读取）。
        assert!(ranges(&[1, 2, 3, 4], &[1, 2, 3, 4, 5, 6, 7, 8]).is_empty());
    }

    #[test]
    fn high_dirty_ratio_falls_back_to_whole_upload() {
        let old = vec![0; 1024];
        let new = vec![1; 1024];
        assert_eq!(ranges(&new, &old), vec![0..1024]);
    }

    #[test]
    fn too_many_ranges_fall_back_to_whole_upload() {
        let old = vec![0; 8192];
        let mut new = old.clone();
        for index in 0..=MAX_DIRTY_RANGES {
            new[index * 768] = 1;
        }
        assert_eq!(ranges(&new, &old), vec![0..8192]);
    }

    #[test]
    fn planned_writes_are_aligned_disjoint_and_reconstruct_new_prefix() {
        // 无外部随机依赖的确定性 LCG，覆盖长度变化、稀疏/密集修改和三槽轮转。
        let mut seed = 0x4d59_5df4_d0f3_3173_u64;
        let mut mirrors = [Vec::new(), Vec::new(), Vec::new()];
        let mut gpu = [Vec::new(), Vec::new(), Vec::new()];

        for frame in 0..600 {
            let slot = (frame + 1) % 3;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let word_len = (seed as usize >> 16) % 1025;
            let mut new = vec![0_u8; word_len * ALIGNMENT as usize];
            for byte in &mut new {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (seed >> 56) as u8;
            }

            // 每隔几帧从旧镜像出发只做稀疏修改，确保多区间路径也被反复覆盖。
            if frame % 3 == 0 {
                new = mirrors[slot].clone();
                new.resize(word_len * ALIGNMENT as usize, 0);
                for _ in 0..5 {
                    if new.is_empty() {
                        break;
                    }
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let index = seed as usize % new.len();
                    new[index] = new[index].wrapping_add(1);
                }
            }

            let planned = VertexBufferPool::plan_dirty_ranges(&new, &mirrors[slot]);
            for (index, range) in planned.iter().enumerate() {
                assert_eq!(range.start % ALIGNMENT as usize, 0);
                assert_eq!(range.end % ALIGNMENT as usize, 0);
                assert!(range.start < range.end && range.end <= new.len());
                if index > 0 {
                    assert!(planned[index - 1].end < range.start);
                }
            }

            gpu[slot].resize(gpu[slot].len().max(new.len()), 0);
            mirrors[slot].resize(new.len(), 0);
            for range in planned {
                gpu[slot][range.clone()].copy_from_slice(&new[range.clone()]);
                mirrors[slot][range.clone()].copy_from_slice(&new[range]);
            }
            assert_eq!(&gpu[slot][..new.len()], new.as_slice());
            assert_eq!(mirrors[slot], new);
        }
    }
}
