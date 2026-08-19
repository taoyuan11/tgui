use std::fmt;
use std::hash::Hash;

/// Common behavior for a `slot + generation` identifier.
///
/// Generation zero is reserved as malformed. Arenas begin at generation one
/// and retire a slot at `u32::MAX`, so an old ID is never made valid by wraparound.
pub trait GenerationalId: Copy + Eq + Ord + Hash + fmt::Debug + 'static {
    /// Builds an ID from wire/storage parts.
    fn from_parts(slot: u32, generation: u32) -> Self;

    /// Zero-based arena slot.
    fn slot(self) -> u32;

    /// Slot generation. Values created by an arena are always non-zero.
    fn generation(self) -> u32;

    /// Returns whether this ID could have been issued by an arena.
    fn is_well_formed(self) -> bool {
        self.generation() != 0
    }

    /// Captures the parts required to reject stale worker results.
    fn stamp(self) -> GenerationStamp {
        GenerationStamp::new(self.slot(), self.generation())
    }
}

macro_rules! generational_id {
    ($name:ident) => {
        #[doc = concat!("Generational identifier for `", stringify!($name), "` objects.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            slot: u32,
            generation: u32,
        }

        impl $name {
            /// Creates an identifier from its stable wire representation.
            pub const fn from_parts(slot: u32, generation: u32) -> Self {
                Self { slot, generation }
            }

            /// Returns the zero-based slot.
            pub const fn slot(self) -> u32 {
                self.slot
            }

            /// Returns the slot generation.
            pub const fn generation(self) -> u32 {
                self.generation
            }

            /// Returns whether generation zero is not in use.
            pub const fn is_well_formed(self) -> bool {
                self.generation != 0
            }

            /// Returns a generation stamp suitable for a worker message.
            pub const fn stamp(self) -> GenerationStamp {
                GenerationStamp::new(self.slot, self.generation)
            }
        }

        impl GenerationalId for $name {
            fn from_parts(slot: u32, generation: u32) -> Self {
                Self::from_parts(slot, generation)
            }

            fn slot(self) -> u32 {
                self.slot
            }

            fn generation(self) -> u32 {
                self.generation
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("slot", &self.slot)
                    .field("generation", &self.generation)
                    .finish()
            }
        }
    };
}

generational_id!(ElementId);
generational_id!(RenderNodeId);
generational_id!(ResourceId);
generational_id!(ImageHandle);
generational_id!(FontHandle);
generational_id!(GlyphPageId);
generational_id!(WindowId);
generational_id!(AnimationId);
generational_id!(HostHandle);

/// Application-facing name for a window's generational handle.
pub type WindowHandle = WindowId;

/// Type-erased generation information carried across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationStamp {
    slot: u32,
    generation: u32,
}

impl GenerationStamp {
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_well_formed(self) -> bool {
        self.generation != 0
    }

    pub fn matches<I: GenerationalId>(self, id: I) -> bool {
        self.slot == id.slot() && self.generation == id.generation()
    }

    pub fn of<I: GenerationalId>(id: I) -> Self {
        id.stamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn identifiers_compare_hash_and_format_by_both_parts() {
        let a = ElementId::from_parts(7, 1);
        let b = ElementId::from_parts(7, 2);
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b);

        assert_eq!(set.len(), 2);
        assert_eq!(a.slot(), b.slot());
        assert_ne!(a.generation(), b.generation());
        assert_eq!(format!("{a:?}"), "ElementId { slot: 7, generation: 1 }");
    }

    #[test]
    fn generation_zero_is_malformed() {
        let id = ResourceId::from_parts(0, 0);
        assert!(!id.is_well_formed());
        assert!(!id.stamp().is_well_formed());
    }
}
