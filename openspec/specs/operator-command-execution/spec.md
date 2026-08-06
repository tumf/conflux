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

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls; pre-run Select MUST continue to expose only Start. Web MUST expose Start, graceful stop, and force stop directly. TUI MUST expose Start plus its existing first-Esc graceful-stop hint, and after that request its existing second-Esc force-stop progression. The fact is presentation-only: shared run control MUST independently revalidate the existing scheduler liveness authority before executing each command.

Execution-mark mutations MUST remain Select-mode mark-only mutations. Accepted Start MUST resolve the authoritative marked target set, apply existing reducer queue intent, and notify the same live scheduler without spawning another scheduler task. Accepted graceful stop and force stop MUST continue to address that live scheduler; graceful stop MUST wake the idle wait after recording the stop request so the scheduler can reach its existing stop boundary.

A Start that only notifies the idle scheduler MUST NOT project Running or clear `persistent_scheduler_idle` by itself. Existing typed workspace or base-lane work-start evidence MUST clear the fact when execution actually begins, project Running from Select, and preserve Stopping when a graceful-stop request arrived first. Cancel-stop MUST remain valid only after graceful stop has projected Stopping; it MUST restore Ready when the idle-episode fact remains true and Running when admitted work already cleared the fact.

#### Scenario: Start wakes the existing idle scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **AND** an eligible change is execution-marked
- **WHEN** Start is accepted through shared run control
- **THEN** existing reducer queue intent is added for the marked target
- **AND** the live scheduler is notified
- **AND** no second scheduler task is spawned
- **AND** Ready remains visible until a typed work-start event arrives

#### Scenario: idle Ready marks remain mark-only

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **WHEN** an operator changes one or all execution marks
- **THEN** the process-local mark set changes under existing Select-mode rules
- **AND** no Running-mode queue mutation is synthesized until Start is accepted

#### Scenario: idle Ready exposes live-scheduler controls

- **GIVEN** `app_mode` is `select` with `persistent_scheduler_idle: true`
- **WHEN** TUI or Web renders lifecycle controls
- **THEN** Web exposes Start, graceful stop, and force stop
- **AND** TUI exposes Start and a first-Esc graceful-stop hint
- **AND** after graceful stop, TUI retains its second-Esc force-stop progression
- **AND** ordinary pre-run Select without the idle fact continues to expose only Start

#### Scenario: graceful stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive in its event-driven wait
- **WHEN** graceful stop is accepted
- **THEN** the existing graceful-stop request is recorded
- **AND** the idle scheduler is notified to reach its stop boundary
- **AND** the frontend projects Stopping while retaining `persistent_scheduler_idle: true`

#### Scenario: cancel stop returns to idle Ready

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** `persistent_scheduler_idle` remains true while the frontend is Stopping
- **WHEN** cancel-stop is accepted
- **THEN** the graceful-stop request is withdrawn
- **AND** the frontend returns to Ready / `app_mode: select`
- **AND** it does not claim Running without typed work-start evidence

#### Scenario: work start wins before cancel stop

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** a typed work-start event arrives while the frontend is Stopping
- **WHEN** that event is projected
- **THEN** Stopping is preserved
- **AND** `persistent_scheduler_idle` becomes false
- **AND** a later accepted cancel-stop returns the frontend to Running rather than Ready

#### Scenario: force stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **WHEN** force stop is accepted
- **THEN** the same scheduler task is cancelled
- **AND** existing stop classification and shutdown-barrier behavior remain authoritative
- **AND** the terminal Stopped or Error projection clears `persistent_scheduler_idle`
