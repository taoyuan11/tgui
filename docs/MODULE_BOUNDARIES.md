# Module boundaries

The dependency direction is one way. `core` is the foundation and may not depend
on another tgui subsystem. Contracts in `state`, `event`, `layout`, `render`,
`media`, and `accessibility` may depend on `core`. `application` composes those
contracts. `platform` adapters sit at the outer edge and may call into
`application`, never the reverse through a concrete backend.

Hot UI-tree data, dirty indexes, cache generations, and compiled scene details
remain crate-private. Public modules expose identifiers, immutable snapshots,
configuration, diagnostics, and headless testing facilities needed by an
application author.

The `Application`, `UpdateTxn`, window contexts, element storage, reactive
graph, event dispatcher, and snapshot stores belong to their creating UI
thread. `UiDispatcher` is the only object intended to cross threads; every
message carries a source generation and the relevant subset of the revision
tuple observed when the work was requested. Worker payloads re-enter through
`Application::consume_background_results`, which validates window,
generation, and revision metadata before staging accepted values in a UI
transaction.

P1 stores immutable `WidgetNode` declarations separately from the retained
`ElementTree`. Widget runtime identity uses Rust `TypeId`; the stable type name
is diagnostic only. Each window owns a distinct dependency namespace, so equal
Element slot/generation pairs in separate arenas cannot coalesce State
invalidations or hit targets. Application-facing code may inspect copied
Element diagnostics through `test_support`/`Application`, but cannot mutate
Element state or subscriptions directly.

Event routing consumes a window-scoped `CommittedHitTarget` from the last
committed layout revision, snapshots the full Element path, and shares one
`UpdateTxn` across capture, target, and bubble. Accessibility actions carry an
independent window scope for their semantic target. Reconciliation clears
removed owners and revalidates retained focus/pointer owners after enabled or
focusable metadata changes.

`Application::apply_transaction` is the common commit boundary for event,
worker, and direct UI updates. `UpdateTxn` computes every heterogeneous State
write against one pre-commit snapshot and publishes the complete staged set
only after command validation; its receipt then drives namespaced multi-window
rebuild and frame scheduling.

P2 keeps Taffy's `TaffyTree`, NodeId map, intrinsic measurement cache, and the
Dirty Tree crate-private. Immutable Widget declarations carry the public
`LayoutStyle`, optional identity-stable `MeasureSpec`, logical scroll offset,
hit-test participation, and boundary metadata; retained Elements copy those
values into their dense nodes. Taffy is built into the minimal headless path
with only std/tree/Flex/Grid/Block/content-size features and never calls a
platform or rendering API. The measurement cache has a 4,096-entry hard limit
and evicts the least-recently-used recomputable result.

All Taffy input and `LayoutSnapshot` geometry is expressed in logical pixels.
`DpiScale` is part of intrinsic measurement cache identity but does not multiply
layout rectangles. `LayoutSnapshot` owns immutable per-Element rectangles,
baselines, effective clips, clamped scroll geometry, hit bounds, and a stable
fingerprint. `Application::hit_test` only reads the atomically committed
snapshot and returns a window-scoped generation/revision target.

The Taffy 0.13 measurement callback exposes only measured size. Custom
`MeasureOutput::baseline` values are therefore retained for snapshot/diagnostic
output but cannot participate in Taffy's baseline-alignment algorithm; callers
must not rely on provider-specific baseline alignment until the layout backend
offers a baseline-aware callback.

The Dirty Tree stores separate `self_flags` and descendant-only
`subtree_flags`, reduces roots at Layout/Render/Hit/Semantics boundaries, and
retains an epoch until CPU snapshot commit succeeds. State phase invalidations,
reconciliation metadata, window events, focus changes, and validated resource
completion all enter this index. Unknown property dependencies use
`PropertyImpact::ALL`; ambiguous structure or stale topology requests a safe
full-layout rebuild. `Application::layout_window` commits the resulting layout
together with the previously reusable scene/resource/semantic components and
records phase/root/rebuild metrics.
