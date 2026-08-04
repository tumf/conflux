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

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Parallel mode changes MUST classify one coherent state, clear marks and queue presentation for newly ineligible changes, and report stable exclusion reasons. Bulk execution-mark mutation MUST choose one target state from eligible rows only and update eligible marks plus Running queue intent atomically.

#### Scenario: Running queue addition enables recovery

**Given**: `alpha` is Running-mode eligible and has a preserved recoverable worktree
**When**: TUI or remote operator service accepts queue addition for `alpha`
**Then**: Reducer `QueueIntent::Queued` is set
**And**: The shared scheduler may resolve `alpha` from its preserved workspace

#### Scenario: Queue removal prevents reacquisition

**Given**: `alpha` was previously queued and has a preserved worktree
**When**: The operator service accepts queue removal or successful stop-and-dequeue
**Then**: Ordinary execution eligibility is revoked
**And**: Catalog refresh and worktree discovery do not reacquire `alpha`
**And**: Explicit requeue or retry is required for later ordinary execution

#### Scenario: Error mode requires retry

**Given**: The application is in Error mode
**When**: The operator requests execution-mark mutation
**Then**: The request is rejected without state change
**And**: `retry_change` or `retry_errors` remains the supported action

#### Scenario: Parallel mode change cleans ineligible intent

**Given**: The application is in Select or Stopped mode and marked changes become ineligible in parallel mode
**When**: The operator enables parallel mode
**Then**: The service clears those execution marks and queue presentation atomically
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
