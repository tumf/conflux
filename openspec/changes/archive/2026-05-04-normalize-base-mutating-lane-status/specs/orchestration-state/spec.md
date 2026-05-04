## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, Conflux SHALL classify the post-archive state from current reducer, workspace, and git facts using a single decision table.

A non-terminal active `Resolving` or `Rejecting` change SHALL be treated as occupying the single base-mutating lane. If that lane is occupied by another change, the archived change SHALL enter `ResolveWait` and display `resolve pending` because the next operation for the archived change is merge/resolve work.

If merge readiness or merge attempt finds a concrete manual blocker, the archived change SHALL enter `MergeWait` and display `merge wait` with a deferral reason. Stable `MergeWait` after archive completion MUST be caused by a concrete manual merge deferral, such as `MergeDeferred(auto_resumable=false)`, or by explicit user retry state.

If there is no lane blocker and no manual merge blocker, the archived change SHALL enter active merge handling, display `resolving` while merge handling is active, and display `merged` after merge completion.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, `RejectWait`, or absent.

#### Scenario: active resolving lane enters resolve pending

**Given**: Change A is actively `Resolving`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic merge/resolve retry after A clears the base-mutating lane

#### Scenario: active rejecting lane enters resolve pending

**Given**: Change A is actively `Rejecting`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated for B
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic merge/resolve retry after A clears the base-mutating lane

#### Scenario: manual blocker enters merge wait

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**And**: merge readiness or merge attempt detects a manual blocker such as a dirty base working tree, dirty archive workspace, incomplete archive verification, or missing archive evidence
**When**: the merge path emits `MergeDeferred(auto_resumable=false)`
**Then**: B SHALL display as `merge wait`
**And**: B SHALL NOT be returned as normal queued work by scheduler queue reconciliation
**And**: B SHALL require explicit `ResolveMerge` retry after the blocker is resolved

#### Scenario: no blocker enters resolving then merged

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**And**: merge readiness finds no manual blocker
**When**: post-archive merge handling runs
**Then**: B SHALL display as `resolving` while merge handling is active
**And**: Conflux SHALL emit merge completion for B without requiring the user to press `M`
**And**: B's derived display status SHALL become `merged`

### Requirement: post-archive-merge-dispatch

Conflux SHALL model `Resolving` and `Rejecting` as mutually exclusive activities on a single base-mutating lane. At any point, among all non-terminal changes, at most one change may have activity `Resolving` or `Rejecting`.

A pending operation that needs this lane SHALL preserve the operation it intends to run next. Merge/resolve work waits as `ResolveWait` and displays `resolve pending`; rejection-review work waits as `RejectWait` and displays `reject pending`.

#### Scenario: resolving and rejecting cannot coexist

**Given**: Change A is actively `Resolving`
**And**: Change B needs to start rejection review
**When**: scheduler state is reduced
**Then**: A remains the only base-mutating lane occupant
**And**: B does not enter active `Rejecting`
**And**: B transitions to `RejectWait`
**And**: B's derived display status is `reject pending`

#### Scenario: rejecting and resolving cannot coexist

**Given**: Change A is actively `Rejecting`
**And**: Change B has archived and needs post-archive merge handling
**When**: post-archive dispatch is evaluated for B
**Then**: A remains the only base-mutating lane occupant
**And**: B does not enter active `Resolving`
**And**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`

#### Scenario: only one rejecting review can run

**Given**: Change A is actively `Rejecting`
**And**: Change B also needs rejection review
**When**: scheduler state is reduced
**Then**: A remains the only base-mutating lane occupant
**And**: B transitions to `RejectWait`
**And**: B's derived display status is `reject pending`

### Requirement: post-archive-merge-dispatch

The system SHALL treat rejection-review wait as reducer-owned scheduler intent, not as TUI-local display state and not as merge/resolve retry intent.

When a rejection review is ready to run but the base-mutating lane is occupied by another active `Resolving` or `Rejecting` change, the reducer SHALL represent the waiting change as `RejectWait` and keep it eligible for scheduler-owned automatic retry. The derived display status SHALL be `reject pending`.

`RejectWait` MUST be distinct from `ResolveWait` so the scheduler can start rejection review, not merge/resolve retry, after the lane clears.

#### Scenario: rejection review waits behind resolving

**Given**: Change A is actively `Resolving`
**And**: Change B produced `openspec/changes/<change_id>/REJECTED.md` and needs dedicated rejecting review
**When**: the scheduler handles B's rejection-review handoff
**Then**: B transitions to `RejectWait`
**And**: B's derived display status is `reject pending`
**And**: B is returned by reducer-owned reject-wait retry membership
**And**: B is not returned by reducer-owned resolve-wait retry membership

#### Scenario: reject pending promotes to rejecting after lane clears

**Given**: Change B is in `RejectWait`
**And**: the active base-mutating lane occupant completes or fails and no other lane occupant remains
**When**: scheduler-owned pending lane retry is evaluated
**Then**: B transitions from `RejectWait` to active `Rejecting`
**And**: B's derived display status becomes `rejecting`
**And**: no other change is active in `Resolving` or `Rejecting`

#### Scenario: rejection review completion clears reject wait intent

**Given**: Change B previously entered `RejectWait`
**And**: B later starts and completes rejection review
**When**: the reducer processes `RejectionReviewCompleted` or `RejectionReviewFailed` for B
**Then**: B is no longer returned by reject-wait retry membership
**And**: B does not regress to `reject pending` on later refresh
