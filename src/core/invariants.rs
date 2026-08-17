/// Machine-readable review item mirrored in `docs/ARCHITECTURE_INVARIANTS.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchitectureInvariant {
    pub id: &'static str,
    pub rule: &'static str,
    pub review_check: &'static str,
}

pub const ARCHITECTURE_INVARIANTS: &[ArchitectureInvariant] = &[
    ArchitectureInvariant {
        id: "UI_THREAD_ONLY",
        rule: "the UI tree is exclusively owned by its creating UI thread",
        review_check: "worker code only sends generation-stamped messages through UiDispatcher",
    },
    ArchitectureInvariant {
        id: "GENERATION_VALIDATION",
        rule: "slot reuse never makes a stale identifier resolve",
        review_check: "all arena and asynchronous result lookups compare slot and generation",
    },
    ArchitectureInvariant {
        id: "ATOMIC_CPU_SNAPSHOT",
        rule: "layout, scene, resources, and semantics commit as one CPU snapshot",
        review_check: "failed candidates leave AtomicSnapshotStore::committed unchanged",
    },
    ArchitectureInvariant {
        id: "BOUNDED_RESOURCES",
        rule: "cache and transient resources have observable hard limits",
        review_check: "in-flight and committed references are excluded from eviction",
    },
    ArchitectureInvariant {
        id: "NATIVE_HOST_ESCAPE_HATCH",
        rule: "ordinary controls never use Native Host",
        review_check: "Button, Text, List, and custom paint use the retained render pipeline",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn invariant_ids_are_unique_and_include_non_negotiable_rules() {
        let ids = ARCHITECTURE_INVARIANTS
            .iter()
            .map(|invariant| invariant.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), ARCHITECTURE_INVARIANTS.len());
        assert!(ids.contains("UI_THREAD_ONLY"));
        assert!(ids.contains("NATIVE_HOST_ESCAPE_HATCH"));
        assert!(ids.contains("ATOMIC_CPU_SNAPSHOT"));
    }
}
