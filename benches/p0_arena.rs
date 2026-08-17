use std::hint::black_box;
use tgui::test_support::default_arena_benchmark;

fn main() {
    for result in default_arena_benchmark() {
        black_box(&result);
        println!(
            "nodes={} alloc={:?} traverse={:?} release={:?} allocations={} reuse={} peak={} reserved={}",
            result.node_count,
            result.allocation,
            result.traversal,
            result.release,
            result.allocations,
            result.slot_reuses,
            result.peak_live,
            result.estimated_reserved_bytes,
        );
    }
}
