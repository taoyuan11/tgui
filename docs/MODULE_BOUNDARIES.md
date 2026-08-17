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

The `Application`, `UpdateTxn`, window contexts, element storage, and snapshot
stores belong to their creating UI thread. `UiDispatcher` is the only P0 object
intended to cross threads; every message carries a source generation and the
revision tuple observed when the work was requested.
