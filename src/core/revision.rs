use std::error::Error as StdError;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RevisionError {
    pub revision: &'static str,
}

impl fmt::Display for RevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} revision exhausted u64", self.revision)
    }
}

impl StdError for RevisionError {}

pub trait RevisionValue: Copy + Eq + Ord + fmt::Debug + Default {
    const NAME: &'static str;
    const ZERO: Self;

    fn from_u64(value: u64) -> Self;
    fn get(self) -> u64;

    fn checked_next(self) -> Result<Self, RevisionError> {
        self.get()
            .checked_add(1)
            .map(Self::from_u64)
            .ok_or(RevisionError {
                revision: Self::NAME,
            })
    }

    fn advance(&mut self) -> Result<Self, RevisionError> {
        *self = self.checked_next()?;
        Ok(*self)
    }
}

macro_rules! revision {
    ($name:ident, $display:literal) => {
        #[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub const ZERO: Self = Self(0);

            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }

            pub fn checked_next(self) -> Result<Self, RevisionError> {
                <Self as RevisionValue>::checked_next(self)
            }

            pub fn advance(&mut self) -> Result<Self, RevisionError> {
                <Self as RevisionValue>::advance(self)
            }
        }

        impl RevisionValue for $name {
            const NAME: &'static str = $display;
            const ZERO: Self = Self::ZERO;

            fn from_u64(value: u64) -> Self {
                Self(value)
            }

            fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

revision!(LayoutRevision, "layout");
revision!(SceneRevision, "scene");
revision!(ResourceRevision, "resource");
revision!(SemanticRevision, "semantic");

/// The independently reusable revision tuple for one window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RevisionSet {
    pub layout: LayoutRevision,
    pub scene: SceneRevision,
    pub resource: ResourceRevision,
    pub semantic: SemanticRevision,
}

impl RevisionSet {
    pub const ZERO: Self = Self {
        layout: LayoutRevision::ZERO,
        scene: SceneRevision::ZERO,
        resource: ResourceRevision::ZERO,
        semantic: SemanticRevision::ZERO,
    };

    pub const fn new(
        layout: LayoutRevision,
        scene: SceneRevision,
        resource: ResourceRevision,
        semantic: SemanticRevision,
    ) -> Self {
        Self {
            layout,
            scene,
            resource,
            semantic,
        }
    }

    /// Advances only the outputs declared observable by `changes`.
    pub fn advance(&mut self, changes: RevisionChanges) -> Result<Self, RevisionError> {
        // Calculate first so exhaustion cannot partially change the tuple.
        let next = Self {
            layout: if changes.layout {
                self.layout.checked_next()?
            } else {
                self.layout
            },
            scene: if changes.scene {
                self.scene.checked_next()?
            } else {
                self.scene
            },
            resource: if changes.resource {
                self.resource.checked_next()?
            } else {
                self.resource
            },
            semantic: if changes.semantic {
                self.semantic.checked_next()?
            } else {
                self.semantic
            },
        };
        *self = next;
        Ok(next)
    }

    pub const fn does_not_regress_from(self, older: Self) -> bool {
        self.layout.get() >= older.layout.get()
            && self.scene.get() >= older.scene.get()
            && self.resource.get() >= older.resource.get()
            && self.semantic.get() >= older.semantic.get()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RevisionChanges {
    pub layout: bool,
    pub scene: bool,
    pub resource: bool,
    pub semantic: bool,
}

impl RevisionChanges {
    pub const NONE: Self = Self {
        layout: false,
        scene: false,
        resource: false,
        semantic: false,
    };

    pub const ALL: Self = Self {
        layout: true,
        scene: true,
        resource: true,
        semantic: true,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_observable_components_advance() {
        let mut revisions = RevisionSet::ZERO;
        revisions
            .advance(RevisionChanges {
                layout: true,
                scene: false,
                resource: true,
                semantic: false,
            })
            .unwrap();

        assert_eq!(revisions.layout, LayoutRevision::new(1));
        assert_eq!(revisions.scene, SceneRevision::ZERO);
        assert_eq!(revisions.resource, ResourceRevision::new(1));
        assert_eq!(revisions.semantic, SemanticRevision::ZERO);
    }

    #[test]
    fn exhaustion_is_atomic() {
        let mut revisions = RevisionSet::new(
            LayoutRevision::new(u64::MAX),
            SceneRevision::new(4),
            ResourceRevision::ZERO,
            SemanticRevision::ZERO,
        );
        let before = revisions;
        assert!(revisions.advance(RevisionChanges::ALL).is_err());
        assert_eq!(revisions, before);
    }
}
