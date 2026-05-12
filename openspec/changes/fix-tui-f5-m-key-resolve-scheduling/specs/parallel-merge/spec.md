## MODIFIED Requirements

### Requirement: merge-attempt-resolve-priority

Parallel merge handling SHALL distinguish auto-resumable deferrals from manual merge-wait deferrals in both reducer events and externally visible workspace/display status.

Merge attempt classification MUST evaluate active resolve/base-mutating lane occupancy before base/workspace dirty state. If another resolve/base-mutating operation is active, the deferral SHALL be `auto_resumable=true` even if the base/workspace appears dirty because of that active operation. Dirty checks SHALL classify a deferral as `auto_resumable=false` only after active resolve/base-mutating occupancy has been ruled out.

When a post-archive merge attempt or manual retry attempt is deferred with `auto_resumable=true`, the change SHALL remain scheduler-owned retry work and SHALL be represented as `resolve pending`, not manual `merge wait`. When the attempt is deferred with `auto_resumable=false`, the change SHALL be represented as manual `merge wait` and removed from scheduler-owned resolve retry membership until explicit retry intent is accepted.

<!-- Expected canonical result after archive: `parallel-merge` will define active resolve/base-mutating occupancy as higher-priority evidence than dirty state for auto_resumable classification. -->

#### Scenario: active resolve deferral is resolve pending even when dirty

**Given**: change `alpha` is resolving or owns the base-mutating lane
**And**: the base/workspace appears dirty because of `alpha`
**And**: change `beta` reaches post-archive merge or manual retry classification
**When**: the merge attempt is classified
**Then**: active resolve/base-mutating occupancy is evaluated before dirty state
**And**: `beta` is deferred with `auto_resumable=true`
**And**: the reducer-visible display status for `beta` is `resolve pending`
**And**: `beta` remains in scheduler-owned resolve retry membership

#### Scenario: manual deferral is merge wait

**Given**: no resolve/base-mutating operation is active
**And**: change `alpha` is in post-archive merge or manual retry handling
**And**: base/workspace state is dirty or manually blocked
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: the reducer-visible display status for `alpha` is `merge wait`
**And**: `alpha` is removed from scheduler-owned resolve retry membership
**And**: `alpha` is visible as requiring manual retry

#### Scenario: retry promotion preserves classification

**Given**: change `alpha` is in `resolve pending`
**When**: the base-mutating lane clears and the scheduler retries the deferred merge
**Then**: successful merge transitions `alpha` to `merged`
**And**: another deferral caused by active resolve/base-mutating occupancy keeps `alpha` in `resolve pending`
**And**: a deferral caused by dirty state with no active resolve/base-mutating occupancy changes `alpha` to `merge wait`

### Requirement: is-dirty-reason-auto-resumable

Dirty reason string parsing SHALL NOT determine whether a merge deferral is auto-resumable. Auto-resumable classification SHALL come from active resolve/base-mutating lane occupancy, resolve counters, or reducer-observed lane state. Dirty text is diagnostic evidence only after occupancy has been ruled out.

<!-- Expected canonical result after archive: `parallel-merge` will retain the prohibition on dirty-reason string parsing and add explicit priority ordering. -->

#### Scenario: dirty reason text does not override active resolve occupancy

**Given**: a dirty reason text includes uncommitted changes
**And**: another resolve/base-mutating operation is active
**When**: merge deferral classification runs
**Then**: the active operation determines `auto_resumable=true`
**And**: the dirty text is not used to force manual `merge wait`
