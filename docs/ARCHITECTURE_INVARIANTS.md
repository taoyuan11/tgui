# Architecture invariants review checklist

Every change must keep these checks answerable with “yes”:

- [x] Build, measure, layout, paint, and semantics code cannot mutate app state.
- [x] Input, animation, resource completion, and worker results enter a UI-thread transaction.
- [x] UI-tree and snapshot mutation is owned by the creating UI thread.
- [x] Worker messages carry a source `slot + generation` and observed revisions.
- [x] Reused arena slots invalidate every handle from the previous generation.
- [x] Layout, scene, resource references, and semantics replace the committed CPU snapshot atomically.
- [x] A rejected/failed candidate leaves the last committed snapshot untouched.
- [x] Revisions never regress, change only with their observable component, and are independently reusable.
- [x] Common nodes use generational IDs and dense storage rather than one heap allocation per node.
- [x] Every cache has a hard bound; committed or in-flight resources cannot be evicted.
- [x] Ordinary controls use the retained render pipeline and never use Native Host as an implementation shortcut.
- [x] Layout geometry remains in logical pixels; DPI is applied only by physical rendering/atlas stages.
- [x] Parent `self_flags` are never upgraded merely because a descendant is dirty.
- [x] Resource invalidation always reaches paint and reaches layout when intrinsic size changes.
- [x] Failed layout/measurement keeps both the committed CPU snapshot and dirty epoch retryable.
- [x] Incremental and forced-full layout produce identical geometry, clip, scroll, hit, and revision output.
- [x] Animation uses one UI-thread FrameClock/Timeline, never writes base State, and marks only its target property dirty.
- [x] An idle timeline requests no further frames; reduced-motion and cancellation complete deterministically.
- [x] VirtualList materialization is bounded to viewport plus overscan; item state, focus, and selection follow ItemKey.

P0–P5 unit tests and P6/P7 contract tests cover the mechanically enforceable
items. The release scripts and CI matrix are the reproducible entry points for
feature and target checks; a check mark means the invariant is implemented and
has a test or contract, not that every target has been run on this workstation.

## P6/P7 evidence

- `tests/p6_native_contract.rs`: Host generation/lifecycle, layout/DPI/z-order,
  focus/input/composition, capability and cost scheduling, renderer boundary,
  stale message rejection, and the ordinary-widget Native Host prohibition.
- `tests/p6_accessibility_contract.rs`: semantic snapshot commit/revision,
  keyed reorder NodeId stability, action routing, focus, stale/unsupported
  actions, VirtualList collection/item metadata, and optional AccessKit native
  subtree/action conversion.
- `tests/p7_contract.rs`: integrated Widget-to-compiler path, four-revision
  commit order, Native Host and semantics integration, fault matrix (compile,
  resource, budget, resize, stale completion), incremental/full equivalence, and
  headless wgpu device-loss recovery when an adapter is available.
- `examples/p7_headless.rs` and `examples/p7_desktop.rs` provide deterministic
  headless and optional `winit` real-window paths. `scripts/desktop-smoke.sh`
  has been run on macOS/aarch64; Windows/Linux interactive smoke remains a
  target-host responsibility.
- `scripts/release-check.sh --quick`, the 10-node retained-tree matrix smoke,
  and the six-scenario stress smoke have passed. The complete 50k benchmark and
  `--full` package/size report remain reproducible follow-up commands, not
  unrecorded measurements.
- GPU time, driver VRAM, and global heap allocation are not currently sampled
  by `FrameMetrics`; these are explicitly unavailable metrics, never fabricated
  as zero values.
