## Purpose

Define a single process-local operator command service so that TUI and remote frontends produce identical orchestration transitions, side effects, and events for the same operator intent.

## Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator command service. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use `EventSink`, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical scheduler eligibility. CLI explicit targets MUST enter the same scheduler initial-target contract.

Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent.

#### Scenario: TUI and remote Start create equivalent targets

**Given**: TUI and remote frontends have the same process-local marks and lifecycle state
**When**: Each requests Start through the shared run-control boundary
**Then**: Both produce the same explicit target IDs
**And**: Unmarked catalog or worktree entries are excluded

#### Scenario: CLI explicit targets use the shared scheduler contract

**Given**: CLI run explicitly targets `alpha`
**And**: Unrelated preserved worktree `beta` exists
**When**: The parallel scheduler starts
**Then**: `alpha` is an eligible initial target
**And**: `beta` is not admitted by worktree discovery

#### Scenario: Refresh does not create operator intent

**Given**: `alpha` has no execution mark, queue intent, retry intent, or lane-wait intent
**When**: A frontend refreshes the active change catalog or workspace observations
**Then**: `alpha` may become visible in snapshots
**And**: It does not become eligible for ordinary execution

### Requirement: Execution intent remains non-authoritative and process-local

Execution marks MUST remain distinct from queue intent, activity, hold state, terminal state, and display status. They MUST reset to false on process restart and MUST NOT become durable workflow-control evidence.

#### Scenario: Restart clears marks without changing workflow routing

**Given**: A process has marked changes and repository/worktree evidence is unchanged
**When**: The process restarts
**Then**: Every execution mark is false
**And**: The next workflow action remains derived from workspace and Git evidence

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons. Bulk execution-mark mutation MUST choose one target state from eligible rows only and update eligible marks plus Running queue intent atomically.

#### Scenario: Eligibility refresh cleans invalid intent

**Given**: Marked or queued changes become ineligible under current repository and worktree evidence
**When**: The operator service applies catalog refresh or eligibility re-evaluation
**Then**: It clears those execution marks and queue presentation atomically
**And**: The outcome identifies each excluded change and reason

#### Scenario: Bulk mark updates one coherent target set

**Given**: Eligible and excluded changes exist in one admitted state
**When**: The operator requests bulk execution-mark mutation
**Then**: The service derives one target mark from eligible changes only
**And**: It updates eligible marks and Running queue intent atomically
**And**: Excluded changes retain coherent intent and receive stable reasons

### Requirement: Cancellation precedes active dequeue

For an active change, the service MUST request per-change cancellation and confirm task/process termination before applying dequeue state. It MUST preserve active state when the cancellation handle is absent, cancellation fails, or confirmation times out.

#### Scenario: Active change terminates before dequeue

**Given**: A change is active and has a registered cancellation handle
**When**: The operator requests stop-and-dequeue
**Then**: Cancellation is issued
**And**: Termination is confirmed
**And**: Only then is `ReducerCommand::DequeueChange` applied

#### Scenario: Missing cancellation handle fails safely

**Given**: A change is active but no cancellation handle exists
**When**: The operator requests stop-and-dequeue
**Then**: The request fails
**And**: The change remains active

### Requirement: Dynamic queue hooks reflect real mutations

The service MUST run `on_queue_add` and `on_queue_remove` exactly once after successful dynamic queue mutations. It MUST NOT run them for initial queue construction, failed requests, or no-op duplicate requests.

#### Scenario: Duplicate addition emits no hook

**Given**: A change is already in the dynamic queue
**When**: The operator requests another addition
**Then**: The request is a no-op
**And**: `on_queue_add` is not executed

### Requirement: Retry routing preserves reconciled evidence

Terminal error retry MUST use `ReducerCommand::RetryError`. Acceptance-stalled retry MUST reconcile the existing runtime hold and resume through the existing explicit acceptance retry path without rerunning apply. Unsupported, non-resumable, or identity-mismatched holds MUST be rejected without discarding blocker evidence.

#### Scenario: Valid acceptance hold resumes acceptance

**Given**: A versioned acceptance hold matches repository, change, worktree, and apply revision evidence
**When**: The operator requests retry for that change
**Then**: The hold is consumed through explicit retry
**And**: Workspace preparation occurs
**And**: Processing resumes at acceptance rather than apply

### Requirement: Event-driven execution mark reconciliation

`ExecutionMarkStore` MUST remain the process-local authoritative execution-mark set across TUI, Web, and shared run-control target resolution. When a typed execution event creates a mark-revoking transition, the system MUST compare reducer evidence immediately before and after applying the event, update only the affected shared mark, and complete that reconciliation before frontend fan-out. Reducer pre-state capture, event application, post-state classification, and mark reconciliation MUST be ordered under the same process-local mutation boundary used by operator mark actions.

Mark-revoking transitions MUST include change-level transition into Error, terminal Rejected, rejected marker rows discovered by refresh, refresh classification that makes a marked target parallel-ineligible, successful per-change dequeue/legacy stop, and the first `on_merged` hook-failure transition into merge-wait recovery. Reconciliation MUST be target-scoped and idempotent.

For system-driven events, TUI row selection MUST mirror the reconciled store and MUST NOT remain an independent mark authority. Reducer `queued` status MUST remain queue presentation and MUST NOT synthesize an execution mark. Operator mark actions MUST use target-scoped shared-service mutation and then mirror the store; they MUST NOT replace the whole store from a cached TUI row set.

The system MUST preserve marks for unrelated changes, blocked/stalled/dependency-wait changes, ordinary MergeWait/ResolveWait, successful archive/merge/push/completion, global fatal Error without a target, and process-level Stopped. A steady Error or `on_merged` recovery row that was explicitly re-marked MUST NOT be cleared by an unrelated or duplicate later event.

#### Scenario: change-level Error clears stale intent before projection

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **AND** `alpha` is not yet in reducer Error
- **WHEN** a processing, apply, acceptance, archive, push, or rejection-review failure transitions `alpha` into reducer Error
- **THEN** the shared execution mark for `alpha` is false before frontend fan-out
- **AND** `beta` remains marked
- **AND** TUI, Web, and Start target resolution observe the same result

#### Scenario: rejection and rejected refresh clear the target mark

- **GIVEN** a change is execution-marked
- **WHEN** `ChangeRejected` makes it terminal Rejected or `ChangesRefreshed` introduces it as a rejected marker row
- **THEN** only that change's shared mark is cleared
- **AND** the TUI rejected row and API snapshot both report it unmarked

#### Scenario: eligibility refresh clears invalid target intent

- **GIVEN** `alpha` and `beta` are execution-marked
- **AND** one refresh classifies `alpha` as parallel-ineligible while `beta` remains eligible
- **WHEN** that authoritative refresh dispatch is reconciled
- **THEN** only `alpha` loses its shared mark
- **AND** existing target-scoped queue cleanup remains coherent
- **AND** no whole-store row publication can remove `beta`

#### Scenario: explicit dequeue clears the shared target

- **GIVEN** a marked change completes stop-and-dequeue successfully
- **WHEN** `ChangeDequeued` or the legacy target-scoped stop event is projected
- **THEN** the shared mark and TUI row mark are false
- **AND** duplicate event delivery is a no-op

#### Scenario: queued presentation does not create a mark

- **GIVEN** a change has reducer queue intent but no execution mark
- **WHEN** a system event synchronizes its TUI row as queued
- **THEN** the row remains unmarked
- **AND** TUI and `/api/v2` continue to distinguish queue intent from execution marks

#### Scenario: duplicate failure after re-mark preserves fresh intent

- **GIVEN** a change-level Error or first `on_merged` recovery edge cleared the old mark
- **AND** an operator explicitly re-marked the steady recovery row through a supported route
- **WHEN** the same failure event is delivered again without creating a new reducer transition
- **THEN** the fresh shared mark remains true
- **AND** existing retry/start flow can consume it

#### Scenario: stale TUI rows cannot resurrect a revoked mark

- **GIVEN** TUI cached rows still show a target marked
- **AND** a system event revokes that target in `ExecutionMarkStore`
- **WHEN** a concurrent TUI operator action settles after the event
- **THEN** the action mutates only its requested target through the shared service
- **AND** it cannot replace the whole store or restore the revoked mark from stale row state

#### Scenario: wait and stop boundaries preserve marks

- **GIVEN** one or more changes are execution-marked
- **WHEN** they become dependency blocked, stalled, externally blocked, MergeWait, ResolveWait, archived, merged, pushed, or the process enters Stopped
- **THEN** their shared marks are preserved
- **AND** process-level Stopped can resume the same marked target set
