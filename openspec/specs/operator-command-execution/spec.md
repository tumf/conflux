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
