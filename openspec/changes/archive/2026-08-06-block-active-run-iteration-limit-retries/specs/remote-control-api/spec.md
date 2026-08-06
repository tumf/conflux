## MODIFIED Requirements

### Requirement: Authoritative operator snapshot

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs. For each change whose command-capable run owns typed Apply iteration-limit evidence and whose scheduler task reports live, the snapshot MUST block `retry_change` with `apply_iteration_limit_active` at the same state revision. Projection and command admission MUST consult the same scheduler-liveness authority. A live-to-exited scheduler transition MUST publish the changed authoritative action snapshot without waiting for unrelated repository activity. Record presence without live ownership MUST NOT remain an action blocker. A headless `cflx run` process with no bound command executor or scheduler-liveness authority MUST omit this process-local blocked reason; command submission remains unavailable through the existing unbound-runtime lifecycle contract.

#### Scenario: Client discovers and snapshots operator state

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot
**And**: The snapshot includes distinct execution mark and queue intent, attention state, blocker and error details, action and parallel eligibility, timing, latest activity, and change-to-worktree relation when applicable

#### Scenario: Replay gap restores operator decisions

**Given**: A client loses retained event history
**When**: It replaces local authoritative data with `GET /api/v2/state`
**Then**: Every server-authoritative operator decision field is restored from the snapshot
**And**: The client does not infer missing state from logs, display strings, paths, or prior events

#### Scenario: Restart preserves workspace-derived authority

**Given**: A process exposes marked changes and attention state
**When**: The process restarts with unchanged workspace and Git evidence
**Then**: Ephemeral operator state is cleared or recomputed
**And**: Workflow routing remains derived from the workspace rather than the prior API snapshot

#### Scenario: Active iteration limit is projected as typed eligibility

**Given**: A command-capable run owns `ApplyIterationLimit` for change `alpha` with attempts 50 and max 50
**And**: The owning scheduler task reports live
**When**: A client reads `/api/v2/state`
**Then**: `alpha.actions.retry_change.allowed` is false
**And**: Its blocked reason is `apply_iteration_limit_active`
**And**: No client must parse the error detail, display status, iteration count, or logs

#### Scenario: Scheduler-task exit removes the active action block

**Given**: The finish-hook owner observed `alpha`'s typed iteration-limit evidence
**When**: The owning scheduler task exits while the old record remains in shared state
**Then**: The liveness transition publishes a new authoritative revision
**And**: That snapshot does not block `alpha` with `apply_iteration_limit_active`
**And**: Retry eligibility is derived from `alpha`'s remaining current evidence

#### Scenario: Headless read-only projection does not retain an actionable block

**Given**: `cflx run` serves `/api/v2` without a bound command executor
**And**: Its old shared state retains typed iteration-limit evidence after the run
**When**: A client reads the subsequent snapshot
**Then**: The snapshot does not expose `apply_iteration_limit_active` as a current action block
**And**: A submitted command is refused by the existing unbound-runtime lifecycle contract

### Requirement: Shared lifecycle scheduling semantics

Start, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, refuse an active-run Apply iteration limit before mutation, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification. `retry_change`, `retry_errors`, and a terminal-error `set_queue_intent=true` alias MUST share the same typed limit guard. An all-limited command MUST settle truthfully without notifying or starting a scheduler.

#### Scenario: Retry dispatches reconciled work

**Given**: A marked error, stalled acceptance hold, or resumable external blocker is valid for retry
**When**: Retry is accepted
**Then**: The shared service applies the correct retry route
**And**: The scheduler is notified or started
**And**: Unsupported holds retain their blocker evidence

#### Scenario: Resolve queues behind an active resolver

**Given**: One merge resolution is active
**When**: Another valid merge-wait change is submitted for resolve
**Then**: It is reserved once in FIFO order
**And**: Duplicate submission does not create another queue entry

#### Scenario: V2 individual retry reports active limit refusal

**Given**: The authoritative snapshot blocks `alpha` retry with `apply_iteration_limit_active`
**When**: A client submits `retry_change` or terminal-error `set_queue_intent=true` for `alpha` at the current revision
**Then**: The shared service rejects the command with a typed target-ineligible result
**And**: The command record does not claim a scheduler effect
**And**: Reducer, mark, queue, hook, explicit-retry, and scheduler state remain unchanged

#### Scenario: V2 bulk retry remains partial

**Given**: `alpha` is active-run limited and `beta` is ordinarily retryable
**When**: A client submits `retry_errors` for both at the current revision
**Then**: `beta` is retried and dispatched exactly once
**And**: The result does not claim that `alpha` was accepted
**And**: `alpha.actions.retry_change.blocked_reason` remains `apply_iteration_limit_active` in the authoritative snapshot at the result revision

#### Scenario: Retry after run closure starts a later boundary

**Given**: The scheduler boundary that limited `alpha` has completed finish-hook ownership and closed
**When**: A current-revision retry for `alpha` is accepted
**Then**: It cannot notify the closed scheduler
**And**: It may start a new scheduler boundary with workspace-derived state and a fresh budget
