## MODIFIED Requirements

### Requirement: merge-attempt-resolve-priority

Parallel merge handling SHALL distinguish auto-resumable deferrals from manual merge-wait deferrals in both reducer events and externally visible workspace/display status.

When a post-archive merge attempt is deferred with `auto_resumable=true`, the change SHALL remain scheduler-owned retry work and SHALL be represented as `resolve pending`, not manual `merge wait`. When a post-archive merge attempt is deferred with `auto_resumable=false`, the change SHALL be represented as manual `merge wait` and removed from scheduler-owned resolve retry membership until explicit retry intent is accepted.

<!-- Expected canonical result after archive: `parallel-merge` will require auto-resumable deferrals to avoid publishing contradictory manual `MergeWait` evidence. -->

#### Scenario: auto-resumable deferral is resolve pending

**Given**: change `alpha` is in post-archive merge handling
**When**: the merge attempt returns `MergeAttempt::Deferred` with `auto_resumable=true`
**Then**: the reducer-visible display status for `alpha` is `resolve pending`
**And**: `alpha` remains in scheduler-owned resolve retry membership
**And**: parallel merge handling does not publish manual `MergeWait` evidence for `alpha`

#### Scenario: manual deferral is merge wait

**Given**: change `alpha` is in post-archive merge handling
**When**: the merge attempt returns `MergeAttempt::Deferred` with `auto_resumable=false`
**Then**: the reducer-visible display status for `alpha` is `merge wait`
**And**: `alpha` is removed from scheduler-owned resolve retry membership
**And**: `alpha` is visible as requiring manual retry

#### Scenario: retry promotion preserves classification

**Given**: change `alpha` is in `resolve pending`
**When**: the base-mutating lane clears and the scheduler retries the deferred merge
**Then**: successful merge transitions `alpha` to `merged`
**And**: another `auto_resumable=true` deferral keeps `alpha` in `resolve pending`
**And**: an `auto_resumable=false` deferral changes `alpha` to `merge wait`
