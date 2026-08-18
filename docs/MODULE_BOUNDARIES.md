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
