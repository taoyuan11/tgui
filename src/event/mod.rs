//! Input/event contract boundary.
//!
//! Platform adapters translate native input into this layer. Event dispatch and
//! capture/target/bubble semantics are implemented in P1; mutation must be
//! expressed through a UI-thread [`crate::state::UpdateTxn`].
