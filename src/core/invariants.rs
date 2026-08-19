/// Machine-readable review item mirrored in `docs/ARCHITECTURE_INVARIANTS.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchitectureInvariant {
    pub id: &'static str,
    pub rule: &'static str,
    pub review_check: &'static str,
}

pub const ARCHITECTURE_INVARIANTS: &[ArchitectureInvariant] = &[
    ArchitectureInvariant {
        id: "READ_ONLY_PHASES",
        rule: "build, measure, layout, paint, and semantics cannot publish application state",
        review_check: "phase dependency capture rejects State writes until the phase exits",
    },
    ArchitectureInvariant {
        id: "TRANSACTIONAL_INGRESS",
        rule: "input, animation, resource completion, and worker results re-enter on the UI thread",
        review_check: "every external result is generation stamped and routed through an event or transaction",
    },
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
        id: "KEYED_RECONCILIATION",
        rule: "Widget key and concrete type determine retained Element identity",
        review_check: "keyed reorder preserves ElementId, state, focus, and subscriptions",
    },
    ArchitectureInvariant {
        id: "DIRTY_SAFE_FALLBACK",
        rule: "dirty work is mergeable, retryable, and falls back to a full rebuild when ambiguous",
        review_check: "incremental output is compared with a forced full rebuild",
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
        id: "REVISION_CAUSALITY",
        rule: "layout, scene, resource, and semantic revisions are monotonic and output driven",
        review_check: "diagnostics retain per-frame dirty roots, chunk rebuilds, and all four revisions",
    },
    ArchitectureInvariant {
        id: "RETAINED_CONTROL_PIPELINE",
        rule: "ordinary controls traverse Element, Layout, Paint IR, and RenderCompiler",
        review_check: "built-in control commands contain no NativeSurface operations",
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
        assert!(ids.contains("READ_ONLY_PHASES"));
        assert!(ids.contains("TRANSACTIONAL_INGRESS"));
        assert!(ids.contains("KEYED_RECONCILIATION"));
        assert!(ids.contains("DIRTY_SAFE_FALLBACK"));
        assert!(ids.contains("NATIVE_HOST_ESCAPE_HATCH"));
        assert!(ids.contains("ATOMIC_CPU_SNAPSHOT"));
        assert!(ids.contains("REVISION_CAUSALITY"));
        assert!(ids.contains("RETAINED_CONTROL_PIPELINE"));
    }
}
