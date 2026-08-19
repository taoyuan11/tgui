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

P5 keeps `Timeline` and `VirtualList` UI-thread-owned. A window owns its timeline while
the application supplies one shared `FrameClock`; animation messages never write State
directly and only mark the animated `(ElementId, PropertyId)` as Paint or Layout dirty.
Presentation overlays are read by the retained layout/render collectors. VirtualList item
state, selection, focus, and cleanup are keyed by stable `ItemKey`, so scrolling can drop
Element declarations without dropping logical state; only viewport plus overscan is
materialized. Data-source and measurement revisions are validated before UI publication.

P6 keeps `Semantics`, `AccessibilityTree`, stable accessibility `NodeId`, and immutable
semantic snapshots in the headless core. A NodeId losslessly packs its Element slot and
generation within a window-local tree, so keyed reorder retains identity and slot reuse
cannot alias a stale action. `Application::layout_window` builds semantics from retained
Element declarations plus committed logical layout bounds only for semantics, focus,
layout-boundary, or accessible-scroll work, then atomically commits Layout/Scene/Resource/
Semantics together. Text widgets expose logical text rather than glyph-atlas state.

The optional `accessibility` feature converts committed snapshots to AccessKit and maps
validated AccessKit requests back into the normal window-scoped `UiEvent` transaction path.
Target-specific `accesskit_windows`, `accesskit_macos`, and `accesskit_unix` dependencies stay
behind target cfgs. Native hosts may expose one opaque node or graft an AccessKit subtree;
VirtualList converts its logical collection and materialized-item metadata without expanding
all items into the Element tree.

Native Host ownership is confined to `native::NativeHostManager` on the UI thread.
`NativeHostFactory` creates a host with a window and Element generation; the manager
owns lifecycle, layout, focus, input/IME forwarding, composition, error status, and
destruction. A host can only return `NativeHostOutput` values, which are wrapped in
generation- and window-scoped `NativeHostMessage` values and consumed by
`Application::consume_native_host_message`; it cannot mutate Widget or Element
storage. `NativeHostCapabilities` and `NativeHostCost` are inputs to
`NativeHostScheduler`, which emits explicit isolated-surface/offscreen boundaries
and costs. `NativeHostWidget` is the only retained-tree bridge and emits either a
`NativeSurface` command or an offscreen `DrawImage` reference. Ordinary built-in
controls are tested to contain neither form.

P7 keeps the orchestration boundary in `application`: one frame drains UI
transactions, reconciles and merges dirty state, builds pending Layout/Scene/
Resource/Semantics outputs, validates/compiles them, and atomically replaces the
CPU snapshot before asking a renderer to submit. `render::wgpu` owns device,
surface, resize/DPI, upload and delayed reclamation details; it never owns Widget
or Element state. `platform::winit` is optional and owns the real OS window/event
loop adapter, translating resize, scale-factor, close, and redraw events into
platform-neutral results. `WinitSurface` keeps the `Arc<Window>` and renderer
handles outside retained trees and exposes explicit device recovery.

`examples/p7_headless.rs` uses only the core/mock boundaries and is therefore the
deterministic acceptance path. `examples/p7_desktop.rs` enables `desktop` for a
real-window smoke path; `webview` and target-specific AccessKit adapters remain
optional feature/target dependencies. The benchmark and release scripts write
machine-specific reports under `target/`; they do not turn unavailable GPU time,
driver VRAM, or global heap allocation values into zeroes.
