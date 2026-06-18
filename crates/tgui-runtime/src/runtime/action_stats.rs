//! 失效 action 命中计数器（度量护栏）。
//!
//! `request_redraw_if_dirty` 每次调用 `invalidate_cached_scene_for_dependencies` 后，
//! 会把它返回的 action 标签（`scene_subtree_patch` / `global_full_rebuild` /
//! `text_input_scene_patch` / …）记一次，用来统计单属性更新等场景下各失效路径的命中分布。
//!
//! 设计为「关闭时零成本」：不带 `bench-support` 时 `record` 编译成 `#[inline(always)]`
//! 空操作，热路径上不留任何分支或线程局部读取。带 `bench-support` 时用线程局部计数，
//! in-crate 测试可 `reset()` / `snapshot()` 读出各 action 命中分布。

#[cfg(not(feature = "bench-support"))]
mod imp {
    #[inline(always)]
    pub(crate) fn record(_action: &'static str) {}
}

#[cfg(feature = "bench-support")]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static COUNTS: RefCell<HashMap<&'static str, u64>> = RefCell::new(HashMap::new());
    }

    /// 记一次 action 命中。探针关闭时直接返回（仅一次线程局部读取）。
    #[inline]
    pub(crate) fn record(action: &'static str) {
        if !ENABLED.with(std::cell::Cell::get) {
            return;
        }
        COUNTS.with(|counts| {
            *counts.borrow_mut().entry(action).or_insert(0) += 1;
        });
    }

    /// 启用探针并清零累计。仅被测试使用，故按 `test` 门控，避免非测试的
    /// `cargo check --features bench-support` 报「未使用」警告。
    #[cfg(test)]
    pub(crate) fn reset() {
        ENABLED.with(|cell| cell.set(true));
        COUNTS.with(|counts| counts.borrow_mut().clear());
    }

    /// 读出各 action 命中次数并关闭探针。仅被测试使用，故按 `test` 门控。
    #[cfg(test)]
    pub(crate) fn snapshot() -> Vec<(&'static str, u64)> {
        let mut entries = COUNTS.with(|counts| {
            counts
                .borrow()
                .iter()
                .map(|(action, count)| (*action, *count))
                .collect::<Vec<_>>()
        });
        ENABLED.with(|cell| cell.set(false));
        // 次数降序、同次数按标签字典序，输出稳定可断言。
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        entries
    }
}

pub(crate) use imp::record;

#[cfg(all(feature = "bench-support", test))]
pub(crate) use imp::{reset, snapshot};
