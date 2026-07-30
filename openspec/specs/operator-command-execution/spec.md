## Purpose

Define a single process-local operator command service so that TUI and remote frontends produce identical orchestration transitions, side effects, and events for the same operator intent.

## Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote frontend orchestration actions through one process-local operator command service. Authoritative workflow transitions MUST use `ReducerCommand`, and notifications MUST use `EventSink`.

#### Scenario: TUI and service calls are equivalent

**Given**: The same lifecycle state and operator intent
**When**: One action enters through the TUI adapter and another equivalent action enters directly through the shared service
**Then**: Both produce the same reducer transition, runtime side effects, and events

### Requirement: Execution intent remains non-authoritative and process-local

Execution marks MUST remain distinct from queue intent, activity, hold state, terminal state, and display status. They MUST reset to false on process restart and MUST NOT become durable workflow-control evidence.

#### Scenario: Restart clears marks without changing workflow routing

**Given**: A process has marked changes and repository/worktree evidence is unchanged
**When**: The process restarts
**Then**: Every execution mark is false
**And**: The next workflow action remains derived from workspace and Git evidence

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, use queue intent for ordinary Running changes, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode.

#### Scenario: Dependency-blocked addition preserves queue intent

**Given**: A Running change has unresolved dependencies
**When**: The operator adds it to the queue
**Then**: Queue intent is retained
**And**: Its display status is `blocked`
**And**: The service does not expose `gated`

#### Scenario: Error mode requires retry

**Given**: The application is in Error mode
**When**: The operator requests execution-mark mutation
**Then**: The request is rejected without state change
**And**: `retry_change` or `retry_errors` remains the supported action

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
