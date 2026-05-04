## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator SHALL attempt to merge the archived workspace immediately unless another non-terminal change is actively occupying the automatic retry blocker lane. The only lifecycle activities that occupy that lane are `Resolving` and `Rejecting` on another change.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, or absent.

Stable `MergeWait` after archive completion MUST be caused by a concrete manual merge deferral, such as `MergeDeferred(auto_resumable=false)`, or by explicit user retry state. Archive completion with no active blocker MUST NOT by itself settle the change into `MergeWait` instead of attempting merge.

Manual/user resolve intent for an existing `MergeWait` row remains valid and may still transition that row to `ResolveWait` through the reducer-owned `ResolveMerge` command.

#### Scenario: no active blocker attempts immediate merge instead of merge wait

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated
**Then**: the orchestrator SHALL attempt the immediate merge path for B
**And**: B SHALL NOT remain in stable `MergeWait` unless that merge attempt emits `MergeDeferred(auto_resumable=false)`

#### Scenario: no active blocker merge success reaches merged without user retry

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**And**: B's archive workspace is mergeable into the base branch
**When**: post-archive merge handling runs
**Then**: Conflux SHALL emit merge completion for B without requiring the user to press `M`
**And**: B's derived display status SHALL become `merged`

#### Scenario: manual deferral is the source of merge wait

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**And**: the immediate merge attempt detects a manual blocker such as a dirty base working tree
**When**: the merge attempt emits `MergeDeferred(auto_resumable=false)`
**Then**: B SHALL display as `merge wait`
**And**: B SHALL NOT be returned as normal queued work by scheduler queue reconciliation
**And**: B SHALL require explicit `ResolveMerge` retry after the blocker is resolved

#### Scenario: active resolving blocker remains auto-resumable

**Given**: Change A is in active `Resolving` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's resolve completes

#### Scenario: active rejecting blocker remains auto-resumable

**Given**: Change A is in active `Rejecting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's rejection review completes or fails

#### Scenario: applying does not create resolve pending

**Given**: Change A is in active `Applying` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: accepting does not create resolve pending

**Given**: Change A is in active `Accepting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: terminal rejected change does not create resolve pending

**Given**: Change A is terminal `Rejected`
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded
