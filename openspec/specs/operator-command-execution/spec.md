## Purpose

Define a single process-local operator command service so that TUI and remote frontends produce identical orchestration transitions, side effects, and events for the same operator intent.

## Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application coordinator. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use one process-lifetime authoritative `EventSink` dispatch boundary shared by runner-local and orchestration-run producers, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each ordinary new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit plus authoritative outcome dispatch, exact revision capture, and scheduler preparation. Activity enabled by the command's later activation or wake MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation MUST leave no reducer, execution-mark, queue, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect. A command awaiting confirmed runtime termination MUST use the same coordinator's two-phase protocol and MUST NOT hold the application gate, authoritative dispatch transaction, or TUI event loop during that wait.

Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent. All transaction, mode, mark, and reservation coordination MUST remain process-local and MUST NOT become durable workflow authority.

#### Scenario: TUI and remote Start create equivalent targets

**Given**: TUI and remote frontends have the same process-local marks and lifecycle state
**When**: Each requests Start through the shared application transaction
**Then**: Both produce the same explicit target IDs and queue intent
**And**: Unmarked catalog or worktree entries are excluded
**And**: the accepted outcome projects Running before the scheduler can emit progress
**And**: the scheduler activates exactly once

#### Scenario: Empty Start has no partial effect

**Given**: No marked change is startable in Select or Stopped mode
**When**: TUI or remote Start enters the shared application transaction
**Then**: the outcome is no-op or failed rather than succeeded
**And**: process mode, reducer state, marks, queue, scheduler, and frontend projection remain unchanged

#### Scenario: Retry dispatches evidence-aware work atomically

**Given**: A target carries terminal Error or a resumable acceptance hold accepted by existing retry classification
**When**: TUI or remote retry is accepted
**Then**: the same retry route, mark, queue intent, and explicit-retry semantics are committed
**And**: Running is projected before one scheduler activation or wake
**And**: unsupported or failed dispatch preserves the original target evidence and every pre-command side effect count

#### Scenario: Resolve reservation and projection are coherent

**Given**: A valid merge-wait target can reserve the single resolver
**When**: TUI or remote resolve is accepted
**Then**: an active reservation projects resolve-pending, `is_resolving=true`, and Running in one outcome dispatch
**And**: scheduling starts or wakes exactly once after that dispatch
**And**: a queued reservation remains FIFO without a second scheduler dispatch
**And**: duplicate reservation is a no-op

#### Scenario: Lifecycle controls share one mode authority

**Given**: TUI and remote adapters address the same running process
**When**: stop, cancel-stop, or force-stop is requested
**Then**: the shared transaction validates one Core-owned process mode and one safe-boundary classification
**And**: accepted stop projects Stopping, accepted cancel-stop projects Running, force-stop waiting for cleanup projects Stopping, and settled force-stop emits Stopped
**And**: an invalid-mode request changes neither stop flags nor projection

#### Scenario: Lifecycle events keep Core mode admissible

**Given**: Core mode entered Running for an accepted run
**When**: typed run activation such as `ProcessingStarted`, authoritative `Stopping`, `Stopped`, global `Error`, guarded `AllCompleted`, or a typed persistent-idle Ready event is dispatched
**Then**: the same Core mode and every frontend projection apply that lifecycle transition
**And**: natural completion returns to Select and admits a later Start
**And**: Stopped admits resume and Error retains explicit retry semantics

#### Scenario: Confirmed termination does not monopolize admission

**Given**: stop-and-dequeue has issued cancellation and is waiting for confirmed termination
**When**: another valid force-stop or unrelated operator command is submitted
**Then**: the second command can execute and settle without waiting for the dequeue timeout
**And**: TUI rendering and authoritative event fan-out remain live
**And**: stop-and-dequeue revalidates the target after reacquiring the gate before it commits
**And**: exact replay does not issue cancellation or start another waiter
**And**: timeout or failed revalidation commits no dequeue reducer, event, or projection effect

#### Scenario: Scheduler preparation failure rolls back staged intent

**Given**: Start, retry, or active resolve has valid targets but scheduler preparation fails
**When**: the shared application transaction returns failure
**Then**: reducer status, marks, dynamic queue, explicit-retry edges, resolve reservations, process mode, hooks, and frontend projection equal their pre-command state
**And**: no scheduler event is emitted

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

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons. Bulk execution-mark classification MUST exclude an active-run-limited terminal-error row with `apply_iteration_limit_active` before mutation, choose one target state from the remaining eligible rows only, and update their marks plus Running queue intent atomically. A terminal-error queue addition that would route through `RetryError` MUST consult the same active typed Apply iteration-limit eligibility before changing reducer state, marks, queue state, hooks, or explicit-retry edges; while limited it MUST be rejected with the same stable reason as explicit retry.

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

#### Scenario: Bulk mark excludes active limited queue aliases before mutation

**Given**: An active-run-limited terminal-error row and unrelated eligible rows exist in one Running-mode bulk request
**When**: The service classifies and applies bulk execution marks
**Then**: It excludes the limited row with `apply_iteration_limit_active`
**And**: The limited row's mark and queue intent remain unchanged
**And**: It atomically applies one coherent target state and queue intent to the remaining eligible rows
**And**: The terminal-error alias guard cannot abort a partially applied bulk operation

#### Scenario: Queue intent cannot alias an active limited retry

**Given**: A terminal-error change carries typed Apply iteration-limit evidence owned by the active run
**When**: A caller requests queue addition or `set_queue_intent=true`
**Then**: The service rejects the request with `apply_iteration_limit_active`
**And**: It does not apply `RetryError` or clear the retained error
**And**: It does not change marks, dynamic queue, explicit-retry edges, hooks, or scheduler state

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

Terminal error retry MUST use `ReducerCommand::RetryError`. Acceptance-stalled retry MUST reconcile the existing runtime hold and resume through the existing explicit acceptance retry path without rerunning apply. Before either route mutates state, the shared service MUST reject a target carrying typed Apply iteration-limit evidence owned by the active run. Unsupported, non-resumable, identity-mismatched, or active-run-limited targets MUST retain their evidence. Bulk retry MUST exclude such targets, dispatch other accepted targets once, and produce no scheduler effect when none remain.

#### Scenario: Valid acceptance hold resumes acceptance

**Given**: A versioned acceptance hold matches repository, change, worktree, and apply revision evidence
**When**: The operator requests retry for that change
**Then**: The hold is consumed through explicit retry
**And**: Workspace preparation occurs
**And**: Processing resumes at acceptance rather than apply

#### Scenario: Individual active-limit retry is mutation-free

**Given**: A terminal-error change carries typed Apply iteration-limit evidence owned by the active run
**When**: An operator requests individual retry
**Then**: The service rejects it with `apply_iteration_limit_active`
**And**: Reducer status, error detail, failed classification, execution mark, queue contents, and explicit-retry publications remain unchanged
**And**: No queue hook, scheduler notification, or scheduler start occurs

#### Scenario: Bulk retry skips only active limited targets

**Given**: One requested change is limited by its active run
**And**: Other requested changes carry ordinary retryable terminal-error or resumable acceptance evidence
**When**: The operator requests bulk retry
**Then**: The limited change retains all state and is not reported as accepted
**And**: Its `apply_iteration_limit_active` reason remains readable in the authoritative snapshot at the result revision
**And**: The other retryable changes are mutated and dispatched exactly once

#### Scenario: All-limited bulk retry is a no-op

**Given**: Every candidate in a bulk retry carries active-run Apply iteration-limit evidence
**When**: The operator requests bulk retry
**Then**: The service returns a no-op with no retryable target
**And**: No reducer, mark, queue, hook, explicit-retry, notification, or scheduler-start effect occurs

#### Scenario: Later boundary uses ordinary retry routing

**Given**: The boundary that owned typed iteration-limit evidence has closed and retired its gate
**And**: Workspace evidence still classifies the change as retryable
**When**: An operator requests retry
**Then**: The service applies the ordinary reconciled retry route
**And**: A new scheduler boundary may start with fresh active-run state

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

### Requirement: Accepted command outcomes converge every same-process frontend

Every changed operator command MUST produce one typed authoritative outcome dispatch containing the process-level decision facts that are not already represented by an exact existing execution event. TUI, Web, `/api/v2`, and lifecycle projections MUST consume that same process-lifetime dispatch and MUST NOT independently rederive admission mode, resolver ownership, or scheduler acceptance from a frontend cache. TUI submission MUST NOT await application-gate ownership or termination confirmation inside its event-processing/render loop.

Existing exact event meanings MUST be reused: graceful stop uses `Stopping`, settled force stop uses `Stopped`, and successful target dequeue uses `ChangeDequeued`. New outcome vocabulary MUST NOT synthesize `ProcessingStarted` or another change lifecycle event merely to signal command acceptance.

TUI row marks changed by operator commands MUST be projected by target delta from the shared `ExecutionMarkStore`. A command MUST NOT replace the complete store from frontend `selected` rows. Event-driven Error, Rejected, refresh, and dequeue revocation remains governed by the shared mark-reconciliation requirement.

#### Scenario: Remote command reaches the next TUI render

**Given**: `cflx tui` and `/api/v2` share one process, reducer, mark store, resolve ledger, and dispatch owner
**When**: an API mark, queue, run, resolve, stop, cancel-stop, force-stop, or dequeue command changes state
**Then**: the next TUI event-processing pass observes the same target delta, row status, process mode, and resolver state
**And**: Web and `/api/v2` observe the same authoritative dispatch

#### Scenario: Local command preserves unrelated remote marks

**Given**: the API changed the shared execution mark for `beta`
**And**: the TUI row cache for `beta` has not yet rendered that delta
**When**: the TUI changes the mark or queue intent for `alpha`
**Then**: only `alpha` is written by the local command outcome
**And**: `beta` retains the API-provided shared mark

#### Scenario: Queue presentation does not become mark authority

**Given**: a Running row is checked because it carries queue intent while an Error row retains hidden explicit retry intent
**When**: a command outcome is projected to TUI rows
**Then**: queue presentation and execution marks remain separate axes
**And**: broad synchronization does not turn hidden retry intent into a checked Error row

#### Scenario: Late command projection cannot overwrite terminal state

**Given**: scheduler progress or cancellation reaches Error, Stopping, Stopped, or completion after a command is accepted
**When**: command and scheduler events are delivered through the authoritative dispatch boundary
**Then**: staged decision commit and outcome dispatch are atomic
**And**: scheduler activity enabled by the command's activation or wake is ordered afterwards
**And**: duplicate or late outcome delivery cannot restore an earlier mode

#### Scenario: Restart discards command coordination

**Given**: a process has application-transaction state, marks, process mode, or resolve reservations in memory
**When**: the process restarts with the same workspace and Git state
**Then**: those process-local values are discarded
**And**: the next workflow action is derived from workspace and Git evidence alone
