## MODIFIED Requirements

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

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry.

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
