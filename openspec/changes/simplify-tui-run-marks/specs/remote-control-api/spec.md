## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST continue to accept only the closed command set, including `set_execution_mark`, `set_all_execution_marks`, explicit queue commands, start/retry, stop, dequeue, and resolve controls. Accepted commands MUST execute through the same process-local application transaction used by the TUI.

`set_execution_mark` and `set_all_execution_marks` MUST represent process-local next-run target intent only. They MUST accept visible non-terminal targets independent of app mode, active/retry/wait status, Apply iteration-limit evidence, and current parallel eligibility, and MUST NOT mutate queue intent, active execution, cancellation, retry/resolve state, hooks, scheduler state, or process mode. Archived, merged, pushed, and rejected targets MUST settle as unchanged no-op outcomes with a stable terminal-target reason and no effects.

Start/retry MUST perform current reducer and worktree eligibility checks at final admission. A worktree-ineligible marked target MUST reject the complete request. Other non-startable statuses MUST be excluded with target-specific detail, and zero runnable targets MUST reject. Error-mode retry MUST route only marked retry-eligible error targets. Failed admission MUST NOT produce partial queue, scheduler, retry-edge, or projection effects.

#### Scenario: Single mark is lifecycle-independent and side-effect free

**Given**: A visible non-terminal change exists in any app mode or lifecycle status
**When**: `set_execution_mark` changes its mark
**Then**: the shared mark store and coherent snapshot reflect the new value
**And**: queue intent, runtime, cancellation, retry, resolve, hooks, scheduler, and mode remain unchanged

#### Scenario: Terminal single mark is a reasoned unchanged no-op

**Given**: A target is archived, merged, pushed, or rejected
**When**: `set_execution_mark` is submitted
**Then**: the command settles successfully as unchanged
**And**: the outcome identifies the terminal-target reason
**And**: no mark, queue, runtime, revision, or scheduler effect occurs

#### Scenario: Bulk mark changes only non-terminal marks

**Given**: Visible non-terminal and terminal changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from non-terminal rows only
**And**: It updates only execution marks atomically
**And**: It returns changed IDs without Running queue-intent effects

#### Scenario: Worktree-invalid Start is rejected atomically

**Given**: Marks include a worktree-ineligible target
**When**: Start is submitted
**Then**: the complete request is rejected
**And**: No scheduler is prepared, activated, or notified
**And**: no queue, mark, retry-edge, reservation, mode, hook, or projection effect survives
**And**: the command identifies the target and reason

#### Scenario: Mixed status Start admits runnable subset

**Given**: Marks include at least one runnable target and another currently non-startable status
**And**: no marked target violates the worktree eligibility fence
**When**: Start is submitted
**Then**: runnable targets are admitted
**And**: non-startable statuses are reported as excluded with target-specific detail

#### Scenario: Zero runnable targets is rejected

**Given**: Marks exist but no runnable target remains after status classification
**When**: Start or Retry is submitted
**Then**: no scheduler or queue effect occurs
**And**: the command rejects with actionable exclusion detail

### Requirement: Event mark changes share the authoritative state revision

When an existing typed failure, rejection, rejected or parallel-ineligible refresh, dequeue, target-scoped stop, or first `on_merged` hook-recovery event revokes an execution mark, `/api/v2` MUST publish the reconciled `execution_marked` value in the same authoritative state revision as that event's reducer/frontend transition. The first effective `ChangeArchived` transition MUST additionally revoke its target mark in that archive revision. The projection MUST read the shared `ExecutionMarkStore` after pre/post event reconciliation and MUST NOT wait for an unrelated refresh or create a second mark-only revision.

Duplicate or late delivery that changes neither reducer state nor execution marks MUST NOT advance another state revision. Event reconciliation MUST preserve unrelated marks in the same snapshot. A duplicate failure delivered after an explicit re-mark MUST preserve that fresh mark when it creates no new reducer transition. Process-level Stopped transitions MUST retain marked resume targets.

#### Scenario: failure event and cleared mark are coherent

- **GIVEN** `alpha` and `beta` are marked in the authoritative operator snapshot
- **WHEN** a typed event transitions `alpha` into change-level Error
- **THEN** the event revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true
- **AND** no intermediate revision exposes Error with the stale mark

#### Scenario: rejected or ineligible refresh clears mark in its refresh revision

- **GIVEN** `alpha` and `beta` are marked active changes
- **WHEN** one authoritative refresh introduces `alpha` as rejected or parallel-ineligible
- **THEN** that refresh revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true

#### Scenario: archive transition clears only its target

- **GIVEN** `alpha` and `beta` are marked
- **WHEN** the first effective `ChangeArchived(alpha)` transition is projected
- **THEN** that archive revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true
- **AND** later merged or pushed projection does not recreate the mark

#### Scenario: on_merged recovery and cleared mark are coherent

- **GIVEN** a marked change is in active merge handling
- **WHEN** its first `on_merged` hook failure enters reducer merge-wait recovery
- **THEN** that event revision reports the recovery row and `execution_marked: false` together

#### Scenario: duplicate revocation is revision-idempotent

- **GIVEN** the target mark is already false after a revoking event
- **WHEN** duplicate delivery produces no reducer or mark change
- **THEN** `/api/v2` does not advance another state revision
- **AND** unrelated marks remain unchanged

#### Scenario: duplicate failure preserves a fresh re-mark

- **GIVEN** a revoking event cleared a target and an operator explicitly re-marked its non-terminal steady recovery row
- **WHEN** the same failure event is delivered again without a reducer transition
- **THEN** the event revision retains `execution_marked: true`
- **AND** no mark-only correction revision is needed

#### Scenario: process stop retains marked resume targets

- **GIVEN** the snapshot contains execution-marked changes
- **WHEN** a process-level `Stopped` transition is published
- **THEN** the stopped revision retains those `execution_marked` values
- **AND** queue intent and reducer stop reconciliation remain separate from mark ownership

<!-- Expected canonical result after archive: `remote-control-api` will expose lifecycle-independent pure marks, deterministic terminal no-ops, precise final admission, and ChangeArchived as an additional coherent revocation edge. -->
