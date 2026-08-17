# Architecture invariants review checklist

Every change must keep these checks answerable with “yes”:

- [ ] Build, measure, layout, paint, and semantics code cannot mutate app state.
- [ ] Input, animation, resource completion, and worker results enter a UI-thread transaction.
- [ ] UI-tree and snapshot mutation is owned by the creating UI thread.
- [ ] Worker messages carry a source `slot + generation` and observed revisions.
- [ ] Reused arena slots invalidate every handle from the previous generation.
- [ ] Layout, scene, resource references, and semantics replace the committed CPU snapshot atomically.
- [ ] A rejected/failed candidate leaves the last committed snapshot untouched.
- [ ] Revisions never regress, change only with their observable component, and are independently reusable.
- [ ] Common nodes use generational IDs and dense storage rather than one heap allocation per node.
- [ ] Every cache has a hard bound; committed or in-flight resources cannot be evicted.
- [ ] Ordinary controls use the retained render pipeline and never use Native Host as an implementation shortcut.

P0 unit tests cover the mechanically enforceable items. Later phases must attach
tests to the remaining phase-specific checks before checking their task off.
