## MODIFIED Requirements

### Requirement: Execution Mode Determines Archive Terminal Semantics

In Serial mode, `ChangeArchived` SHALL set the terminal state to `Archived`.

In Parallel mode, `ChangeArchived` SHALL NOT by itself set `MergeWait`. Parallel archive completion SHALL enter post-archive merge handling according to reducer-owned base-mutating lane state:

- when another non-terminal change occupies the base-mutating lane with `Resolving` or `Rejecting`, the archived change SHALL become `ResolveWait` and remain scheduler-consumable;
- when no base-mutating lane blocker exists and no concrete manual blocker has been observed, the archived change SHALL become active `Resolving`;
- only concrete manual deferral evidence, such as `MergeDeferred(auto_resumable=false)`, SHALL set `MergeWait`.

<!-- Expected canonical result after archive: `orchestration-state` will no longer say parallel `ChangeArchived` unconditionally becomes `MergeWait`; it will describe resolving / resolve pending / merge wait as distinct reducer-owned outcomes. -->

#### Scenario: parallel archive without blocker enters resolving

**Given**: the orchestrator is running in Parallel execution mode
**And**: no other non-terminal change is `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `ActivityState::Resolving`
**And**: `alpha` does not have `WaitState::MergeWait`
**And**: the derived display status is `resolving`

#### Scenario: parallel archive waits behind active base-mutating lane

**Given**: the orchestrator is running in Parallel execution mode
**And**: change `beta` is non-terminal and actively `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `WaitState::ResolveWait`
**And**: `alpha` is returned by reducer-owned resolve-wait membership
**And**: the derived display status is `resolve pending`
**And**: `alpha` is not displayed as `merge wait`

#### Scenario: manual merge deferral enters merge wait

**Given**: change `alpha` is in post-archive merge handling
**When**: the reducer receives `MergeDeferred(alpha, auto_resumable=false)`
**Then**: `alpha` has `WaitState::MergeWait`
**And**: normal queue intent for `alpha` is removed
**And**: `alpha` is not returned by reducer-owned resolve-wait membership
**And**: the derived display status is `merge wait`

#### Scenario: auto-resumable merge deferral remains resolve pending

**Given**: change `alpha` is in post-archive merge handling
**When**: the reducer receives `MergeDeferred(alpha, auto_resumable=true)` while `alpha` is not already active
**Then**: `alpha` has `WaitState::ResolveWait`
**And**: `alpha` remains scheduler-consumable retry work
**And**: the derived display status is `resolve pending`
**And**: `alpha` is not classified as manual `merge wait`

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations and refresh-derived archive-complete evidence SHALL NOT regress reducer-owned active, pending, or terminal lifecycle states to `MergeWait` without concrete manual deferral evidence.

A `ChangesRefreshed` event containing a change in `merge_wait_ids` represents archived-but-not-yet-merged workspace evidence. That evidence MAY preserve or restore an already-established manual `MergeWait`, but it MUST NOT override `ActivityState::Resolving`, `WaitState::ResolveWait`, `WaitState::RejectWait`, `ActivityState::Rejecting`, or terminal states.

<!-- Expected canonical result after archive: `orchestration-state` will treat refresh-derived merge-wait evidence as lower precedence than reducer-owned active/pending/terminal state. -->

#### Scenario: refresh evidence does not regress resolving

**Given**: change `alpha` has `ActivityState::Resolving`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolving`
**And**: `alpha` is not changed to `merge wait`

#### Scenario: refresh evidence does not regress resolve pending

**Given**: change `alpha` has `WaitState::ResolveWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolve pending`
**And**: reducer-owned scheduler retry membership remains available

#### Scenario: refresh evidence can preserve concrete manual merge wait

**Given**: change `alpha` has already received concrete manual deferral evidence and is in `WaitState::MergeWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `merge wait`
**And**: no normal queue intent is reintroduced for `alpha`
