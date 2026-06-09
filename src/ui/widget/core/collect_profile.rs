//! 逐帧场景重收集（scroll/animation 热路径）的相位计时探针。
//!
//! 设计为「关闭时零成本」：不带 `bench-support` 时，`timed` / `record` / `record_node`
//! 编译成 `#[inline(always)]` 空操作（`timed` 直接调用闭包），热路径上不留任何分支或
//! 线程局部读取。带 `bench-support` 时才用线程局部累加器统计各相位**独占**耗时，
//! 用来在做结构性优化前定位真实开销分布。
//!
//! 用法：`reset()` → 跑若干次 `recollect_scene_only` → `snapshot()` 读出累计。

/// 单个相位标识。即便关闭探针也保留定义，使调用点无需 `cfg`。
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum Phase {
    /// `resolve_collect_visual_state`：taffy 查询 + 动画/样式解析。
    VisualState,
    /// `push_surface_primitives_and_base_hit_regions`：阴影/背景/边框/聚焦环 + 基础命中区。
    Surface,
    /// `collect_layout_media_kind` / `collect_control_kind`：含容器子节点递归（非独占）。
    KindBody,
    /// 文本臂的 `push_text_primitives`（字体解析 + 测量 + 内容生成），叶子独占。
    Text,
    /// `visual_contexts.insert` + `chunks.insert` + 可能的 `chunk_parts` 兜底克隆。
    Bookkeeping,
}

#[cfg(not(feature = "collect-profile"))]
mod imp {
    use super::Phase;

    #[inline(always)]
    pub(crate) fn timed<R>(_phase: Phase, body: impl FnOnce() -> R) -> R {
        body()
    }

    #[inline(always)]
    pub(crate) fn record_node() {}
}

#[cfg(feature = "collect-profile")]
mod imp {
    use super::Phase;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static VISUAL_STATE: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static SURFACE: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static KIND_BODY: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static TEXT: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static BOOKKEEPING: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static NODE_COUNT: Cell<u64> = const { Cell::new(0) };
    }

    fn slot(phase: Phase) -> &'static std::thread::LocalKey<Cell<Duration>> {
        match phase {
            Phase::VisualState => &VISUAL_STATE,
            Phase::Surface => &SURFACE,
            Phase::KindBody => &KIND_BODY,
            Phase::Text => &TEXT,
            Phase::Bookkeeping => &BOOKKEEPING,
        }
    }

    /// 计时一个相位闭包；探针关闭时直接执行（仅一次线程局部读取）。
    #[inline]
    pub(crate) fn timed<R>(phase: Phase, body: impl FnOnce() -> R) -> R {
        if !ENABLED.with(Cell::get) {
            return body();
        }
        let started = Instant::now();
        let result = body();
        slot(phase).with(|cell| cell.set(cell.get() + started.elapsed()));
        result
    }

    /// 统计一个被收集的节点。
    #[inline]
    pub(crate) fn record_node() {
        if !ENABLED.with(Cell::get) {
            return;
        }
        NODE_COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    /// 启用探针并清零累计。仅被 `#[ignore]` 的画像测试使用,故同时按 `test` 门控,
    /// 避免非测试的 `cargo check --features collect-profile` 报「未使用」警告。
    #[cfg(test)]
    pub(crate) fn reset() {
        ENABLED.with(|cell| cell.set(true));
        VISUAL_STATE.with(|cell| cell.set(Duration::ZERO));
        SURFACE.with(|cell| cell.set(Duration::ZERO));
        KIND_BODY.with(|cell| cell.set(Duration::ZERO));
        TEXT.with(|cell| cell.set(Duration::ZERO));
        BOOKKEEPING.with(|cell| cell.set(Duration::ZERO));
        NODE_COUNT.with(|cell| cell.set(0));
    }

    /// 读出累计并关闭探针。仅被 `#[ignore]` 的画像测试使用,故同时按 `test` 门控,
    /// 避免非测试的 `cargo check --features collect-profile` 报「未使用」警告。
    #[cfg(test)]
    pub(crate) fn snapshot() -> super::PhaseBreakdown {
        let ms = |cell: &'static std::thread::LocalKey<Cell<Duration>>| {
            cell.with(|cell| cell.get().as_secs_f64() * 1000.0)
        };
        let breakdown = super::PhaseBreakdown {
            visual_state_ms: ms(&VISUAL_STATE),
            surface_ms: ms(&SURFACE),
            kind_body_ms: ms(&KIND_BODY),
            text_ms: ms(&TEXT),
            bookkeeping_ms: ms(&BOOKKEEPING),
            node_count: NODE_COUNT.with(Cell::get),
        };
        ENABLED.with(|cell| cell.set(false));
        breakdown
    }
}

pub(crate) use imp::{record_node, timed};

#[cfg(all(feature = "collect-profile", test))]
pub(crate) use imp::{reset, snapshot};

/// 各相位累计（毫秒）+ 被收集节点数。仅被 `#[ignore]` 的画像测试使用,故同时按
/// `test` 门控,避免非测试的 `cargo check --features collect-profile` 报「未使用」警告。
#[cfg(all(feature = "collect-profile", test))]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PhaseBreakdown {
    pub visual_state_ms: f64,
    pub surface_ms: f64,
    pub kind_body_ms: f64,
    pub text_ms: f64,
    pub bookkeeping_ms: f64,
    pub node_count: u64,
}
