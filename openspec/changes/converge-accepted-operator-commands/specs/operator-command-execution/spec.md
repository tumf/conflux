## MODIFIED Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application transaction. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use the process-wide authoritative `EventSink` dispatch owner, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit, authoritative outcome dispatch, and scheduler activation. A scheduler started or woken by the command MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation or activation MUST leave no reducer, execution-mark, queue, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect.

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

#### Scenario: Scheduler launch failure rolls back staged intent

**Given**: Start, retry, or active resolve has valid targets but scheduler preparation or activation fails
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

## ADDED Requirements

### Requirement: Accepted command outcomes converge every same-process frontend

Every changed operator command MUST produce one typed authoritative outcome dispatch containing the process-level decision facts that are not already represented by an exact existing execution event. TUI, Web, `/api/v2`, and lifecycle projections MUST consume that same dispatch and MUST NOT independently rederive admission mode, resolver ownership, or scheduler acceptance from a frontend cache.

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
**When**: command and scheduler events are delivered through the authoritative dispatch owner
**Then**: the accepted command effect is ordered before scheduler activation
**And**: duplicate or late outcome delivery cannot restore an earlier mode

#### Scenario: Restart discards command coordination

**Given**: a process has application-transaction state, marks, process mode, or resolve reservations in memory
**When**: the process restarts with the same workspace and Git state
**Then**: those process-local values are discarded
**And**: the next workflow action is derived from workspace and Git evidence alone
