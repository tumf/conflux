## ADDED Requirements

### Requirement: Global stopped event reconciles interrupted runtime state

After an operator-requested run reaches its cancellation-safe terminal boundary, process-level `ExecutionEvent::Stopped` SHALL reconcile reducer-owned transient state before frontend projection. Every non-terminal change that still has active activity, queued intent, or a wait/hold owned by the ending run MUST become idle, `NotQueued`, non-waiting, non-terminal, and protected from stale lifecycle reactivation until explicit requeue. The transition MUST clear blocker metadata, commit-phase presentation, and scheduler-owned resolve/reject/stall membership associated with the stopped run.

The global event MUST NOT assign per-change `TerminalState::Stopped`. It MUST preserve execution marks, workspace and Git evidence, task progress, fresh idle rows, and existing terminal outcomes. All state introduced by this reconciliation SHALL remain process-local and MUST NOT become restart-routing evidence.

#### Scenario: Accepting change returns to resumable not queued state

- **GIVEN** change `alpha` is non-terminal, execution-marked, and reducer-visible as `accepting`
- **WHEN** the reducer receives process-level `Stopped` after scheduler cleanup completes
- **THEN** `alpha` has idle activity, `NotQueued` intent, no wait or blocker metadata, and no per-change terminal outcome
- **AND** its derived display status is `not queued`
- **AND** its execution mark remains set

#### Scenario: Queued and waiting work is released from the stopped run

- **GIVEN** non-terminal changes carry queued intent, dependency or external blocker state, stalled state, merge wait, resolve pending, reject pending, or another active execution stage
- **WHEN** process-level `Stopped` is reduced
- **THEN** each row owned by the ending run becomes idle `not queued`
- **AND** scheduler-owned resolve, reject, and stall membership from that run is cleared
- **AND** no row becomes terminal `stopped`

#### Scenario: Terminal and unrelated fresh rows are preserved

- **GIVEN** one row is already terminal `error`, `merged`, `pushed`, or `rejected`
- **AND** another row is fresh idle `not queued` with no wait or activity
- **WHEN** process-level `Stopped` is reduced
- **THEN** both rows remain unchanged
- **AND** the stop does not erase a change outcome or claim ownership of unrelated idle work

#### Scenario: Same-process reactivation input cannot resurrect stopped work

- **GIVEN** `alpha` was reconciled by process-level `Stopped`
- **WHEN** a late activity event arrives or `ChangesRefreshed.merge_wait_ids` observes its archived workspace in the same process
- **THEN** `alpha` remains `not queued` and does not re-enter `MergeWait`
- **WHEN** the operator later explicitly queues the marked change
- **THEN** the dequeue guard is released and ordinary workspace-derived execution becomes eligible again
- **WHEN** the process instead restarts with the same workspace evidence
- **THEN** startup may re-derive `MergeWait` because the process-local guard is not durable routing state

#### Scenario: Frontends publish one reconciled state

- **GIVEN** TUI and `/api/v2` consume an authoritative dispatch for process-level `Stopped`
- **WHEN** the event is projected and a later change refresh occurs
- **THEN** both surfaces report the interrupted row as `not queued` with `NotQueued` queue intent
- **AND** neither surface restores the preceding active status
- **AND** duplicate `Stopped` delivery does not create another state transition
