## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, Conflux SHALL classify the post-archive state from current reducer, workspace, and git facts using a single decision table:

1. If another merge/resolve lane is actively occupied, the archived change SHALL enter `ResolveWait` and display `resolve pending`.
2. If merge readiness or merge attempt finds a concrete manual blocker, the archived change SHALL enter `MergeWait` and display `merge wait` with a deferral reason.
3. Otherwise, the archived change SHALL enter active merge handling, display `resolving` while merge handling is active, and display `merged` after merge completion.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, or absent.

Stable `MergeWait` after archive completion MUST be caused by a concrete manual merge deferral, such as `MergeDeferred(auto_resumable=false)`, or by explicit user retry state. Archive completion with no active blocker MUST NOT by itself settle the change into `MergeWait` instead of attempting merge.

Manual/user resolve intent for an existing `MergeWait` row remains valid and may still transition that row to `ResolveWait` through the reducer-owned `ResolveMerge` command.

#### Scenario: active merge lane enters resolve pending

**Given**: Change A is actively merging or resolving
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's merge/resolve lane clears

#### Scenario: active rejecting blocker remains auto-resumable

**Given**: Change A is in active `Rejecting` state
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's rejection review completes or fails

#### Scenario: manual blocker enters merge wait

**Given**: no other merge/resolve lane is actively occupied
**And**: Change B has just been archived in parallel mode
**And**: merge readiness or merge attempt detects a manual blocker such as a dirty base working tree, dirty archive workspace, incomplete archive verification, or missing archive evidence
**When**: the merge path emits `MergeDeferred(auto_resumable=false)`
**Then**: B SHALL display as `merge wait`
**And**: B SHALL NOT be returned as normal queued work by scheduler queue reconciliation
**And**: B SHALL require explicit `ResolveMerge` retry after the blocker is resolved

#### Scenario: no blocker enters resolving then merged

**Given**: no other merge/resolve lane is actively occupied
**And**: Change B has just been archived in parallel mode
**And**: merge readiness finds no manual blocker
**When**: post-archive merge handling runs
**Then**: B SHALL display as `resolving` while merge handling is active
**And**: Conflux SHALL emit merge completion for B without requiring the user to press `M`
**And**: B's derived display status SHALL become `merged`

#### Scenario: applying does not create resolve pending

**Given**: Change A is in active `Applying` state
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: accepting does not create resolve pending

**Given**: Change A is in active `Accepting` state
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: terminal rejected change does not create resolve pending

**Given**: Change A is terminal `Rejected`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

### Requirement: post-archive-status-idempotency

Parallel post-archive status updates SHALL be idempotent and monotonic with respect to final merge completion. Once a change reaches `Merged`, later archive milestones, workspace refreshes, cleanup events, or archived workspace observations MUST NOT regress its derived display status to `archived`, `merge wait`, `resolve pending`, or `resolving`.

#### Scenario: merged does not oscillate with archived

**Given**: Change B has reached terminal `Merged` in parallel mode
**When**: a later `ChangeArchived` event, `ChangesRefreshed` event, archived workspace observation, or cleanup event is processed for B
**Then**: B SHALL remain terminal `Merged`
**And**: B's derived display status SHALL remain `merged`
**And**: the UI SHALL NOT alternate B between `merged` and `archived`

#### Scenario: no-blocker merge wait does not oscillate before merged

**Given**: no other merge/resolve lane is actively occupied
**And**: Change B has just been archived in parallel mode
**And**: no manual merge blocker has been detected
**When**: post-archive events and refreshes are processed before merge completion
**Then**: B SHALL NOT display `merge wait`
**And**: B SHALL either display active merge handling as `resolving` or final completion as `merged`
