## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge immediately unless another non-terminal change is actively occupying the automatic retry blocker lane. The only lifecycle activities that occupy that lane are `Resolving` and `Rejecting` on another change.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, or absent.

Manual/user resolve intent for an existing `MergeWait` row remains valid and may still transition that row to `ResolveWait` through the reducer-owned `ResolveMerge` command.

#### Scenario: archive completes while another change is resolving

**Given**: Change A is in active `Resolving` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's resolve completes

#### Scenario: archive completes while another change is rejecting

**Given**: Change A is in active `Rejecting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's rejection review completes or fails

#### Scenario: archive completes while another change is applying

**Given**: Change A is in active `Applying` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: archive completes while another change is accepting

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

#### Scenario: no active blocker starts immediate merge path

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated
**Then**: the orchestrator attempts the immediate merge/resolve path for B instead of recording automatic `ResolveWait`
