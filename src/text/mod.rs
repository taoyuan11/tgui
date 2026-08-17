//! Text-system boundary.
//!
//! Logical text layout remains separate from glyph resources. The optional
//! shaping backend is introduced in P4 behind `text`.

pub const BACKEND_ENABLED: bool = cfg!(feature = "text");
