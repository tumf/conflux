## ADDED Requirements

### Requirement: Agent-readable execution observation

The authenticated `GET /api/v2/execution-status` resource MUST provide a coherent machine-readable observation of scheduler liveness, actual active lifecycle work, typed current and last-completed phases, iteration and timing boundaries, latest lifecycle activity, and the latest retained structured log for the process and each exactly associated change. It MUST include the process incarnation, `state_revision`, `event_sequence`, and an `observed_at` instant.

Scheduler liveness MUST read the same process-local `RunBoundaryLiveness` authority used by operator snapshot eligibility and command admission, and MUST remain distinct from `has_active_work`. `has_active_work` MUST be true while at least one change has an active reducer `ActivityState`, or while a closed process-level dependency-analysis, base-branch-merge, conflict-resolution, branch-merge, or workspace-cleanup activity has received its typed start event without its typed terminal event. Persistent-idle scheduler liveness alone MUST NOT be active work. Command acceptance, `app_mode: running`, an execution mark, queue intent, task completion percentage, or a display status alone MUST NOT certify active work or a lifecycle phase.

Per-change phase MUST project the existing reducer activity authority as `preparing`, `apply`, `acceptance`, `rejection_review`, `archive`, `resolve`, typed `push`, `none`, or `unknown`. Analysis MUST remain process-level. `merge` MAY appear as a last-completed phase only from a typed per-change merge-completion fact. `hook` MUST NOT be advertised until production emits typed hook start/completion facts. Phase facts MUST update synchronously from the same source event under the authoritative dispatch boundary and MUST NOT form a second lifecycle state machine. Unknown evidence MUST remain explicitly unknown.

Every execution-status time MUST be an absolute UTC RFC 3339 instant. The resource MUST NOT return elapsed seconds, age seconds, localized relative-time text, or advance `state_revision` merely because time passed. A log-only observation MAY change latest-log output and `event_sequence` while retaining the current `state_revision`.

Latest-log selection MUST use bounded sanitized in-memory ring insertion order. A change latest log MUST be selected only by exact structured `change_id` equality. The resource MUST return a new closed projection containing only sanitized message, level, operation, iteration, and RFC 3339 UTC `created_at`; it MUST NOT embed the existing `LogEntry` wire shape, display timestamp, or `workspace_path`. The API MUST NOT read persistent log files to fill this resource, expose a persistent log path or file URL, accept a host path or filename for log access, or expose such a locator differently over UDS and TCP. Complete retained API logs MUST remain readable through authenticated `GET /api/v2/logs` and live observation through the existing event transports.

#### Scenario: Active change exposes absolute observation boundaries

**Given**: A change has completed Apply and is actively running Acceptance
**And**: Typed lifecycle facts and an exactly associated structured log are retained
**When**: A client reads execution status
**Then**: The change reports `current_phase: acceptance` and `last_completed_phase: apply`
**And**: `observed_at`, phase boundaries, activity time, and log creation time are absolute UTC RFC 3339 instants
**And**: No elapsed, age, or relative-time field is returned

#### Scenario: Idle scheduler is not active work

**Given**: A persistent scheduler task is alive but parked without admitted lifecycle work
**When**: A client reads execution status
**Then**: `scheduler_running` is true
**And**: `has_active_work` is false
**And**: application mode, marks, queue intent, or prior command success do not change that answer

#### Scenario: Log-only activity preserves state revision

**Given**: An execution-status response identifies state revision 12 and event sequence 30
**When**: A retained structured log arrives without changing the authoritative snapshot
**Then**: A later execution-status response may expose that latest log and a later event sequence
**And**: Its state revision remains 12

#### Scenario: Change latest log requires exact association

**Given**: The retained ring contains process logs, logs for `alpha`, and text mentioning `alpha` without structured association
**When**: A client reads execution status for `alpha`
**Then**: `alpha.latest_log` is the newest entry whose structured `change_id` exactly equals `alpha`
**And**: Message substring, operation, iteration, or workspace path is not used as fallback association

#### Scenario: Log path remains private on every transport

**Given**: The same process serves UDS and TCP listeners
**When**: A client reads capabilities, state, execution status, logs, events, commands, or OpenAPI over either listener
**Then**: No response exposes the persistent log path, repository root, workspace path as a log locator, or file URL
**And**: The client can still read the bounded structured log ring through `/api/v2/logs`

<!-- Expected canonical result after archive: v2 includes an agent-readable execution-status resource with absolute timestamps, exact structured log association, and a transport-independent private-log-path boundary. -->

### Requirement: Phase-aware immutable stop command result

A settled successful `stop_and_dequeue` command record MUST contain a closed typed result captured after confirmed termination and final lifecycle revalidation. The result MUST identify the cancelled phase, the last completed phase, nullable proof of the final managed-worktree Apply commit and its commit OID, and `effects_rolled_back: false`. The presentation detail MUST state that dequeue does not roll back previously completed worktree effects.

Apply commit evidence MUST retain each non-empty typed `ApplyCompleted.revision` OID as a per-change, per-process-incarnation fact. At settlement the server MUST identify the managed worktree through its own change mapping and prove the retained OID equals or is an ancestor of the quiescent worktree HEAD before reporting presence true and returning that retained OID. Missing or empty completion facts, restart-empty facts, absent worktree, Git failure, or non-ancestor evidence MUST be unknown; the server MUST NOT reconstruct evidence from task count, display status, log text, or commit subject. No result may expose branch, worktree, repository, or log paths.

The command registry MUST store the typed result with the settled command record. Exact idempotent replay MUST return the original result unchanged and MUST NOT re-read Git, reclassify phase, or repeat cancellation or dequeue effects after later state changes.

#### Scenario: Apply completes before acceptance cancellation settles

**Given**: The final managed-worktree Apply commit is created and Apply completion is published
**And**: Acceptance starts before stop-and-dequeue termination is confirmed
**When**: stop-and-dequeue settles successfully after cancelling Acceptance
**Then**: The typed result reports `cancelled_phase: acceptance` and `last_completed_phase: apply`
**And**: Apply commit presence is true and its OID equals the retained final Apply commit
**And**: `effects_rolled_back` is false
**And**: The final Apply commit remains in the managed worktree

#### Scenario: Already-terminated target has no cancelled phase

**Given**: The target task already terminated and no typed phase is active at settlement
**When**: stop-and-dequeue settles successfully through its existing already-terminated path
**Then**: The typed result reports `cancelled_phase: none`
**And**: It does not invent a phase from prior display or log evidence

#### Scenario: Stop settles before final Apply commit

**Given**: Apply is active and no final managed-worktree Apply commit has been created
**When**: stop-and-dequeue confirms termination and settles
**Then**: The typed result reports Apply as the cancelled phase
**And**: It does not claim that the final Apply commit is present
**And**: It states that no prior worktree effect was rolled back

#### Scenario: Unavailable evidence remains unknown

**Given**: Cancellation and dequeue succeed but lifecycle or managed-worktree Git evidence cannot be read safely
**When**: The command record settles
**Then**: Unavailable phase or Apply commit fields are explicitly unknown or null
**And**: The result does not infer them from display status, task progress, logs, or commit subject

#### Scenario: Exact replay preserves settlement evidence

**Given**: A stop-and-dequeue command settled with typed phase and Apply commit evidence
**And**: Later lifecycle or Git state changed
**When**: The same idempotency key and typed command identity are replayed
**Then**: The original command ID, state, detail, result, and result revision are returned unchanged
**And**: The server does not re-read Git or issue another cancellation

<!-- Expected canonical result after archive: stop-and-dequeue command records explain the actual cancelled phase and retained Apply effect without implying rollback or requiring clients to inspect Git independently. -->
