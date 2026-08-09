## MODIFIED Requirements

### Requirement: Authoritative operator snapshot

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs. A change-scoped `ProcessingError` MUST update the failed change's display status, bounded sanitized error detail, activity, attention, action eligibility, and reconciled execution mark without changing `app_mode` or setting `process_error`. Only a typed process-fatal event MAY set process-wide Error state. For each change whose command-capable run owns typed Apply iteration-limit evidence and whose scheduler task reports live, the snapshot MUST block `retry_change` with `apply_iteration_limit_active` at the same state revision. Projection and command admission MUST consult the same scheduler-liveness authority. A live-to-exited scheduler transition MUST publish the changed authoritative action snapshot without waiting for unrelated repository activity. Record presence without live ownership MUST NOT remain an action blocker. A headless `cflx run` process with no bound command executor or scheduler-liveness authority MUST omit this process-local blocked reason; command submission remains unavailable through the existing unbound-runtime lifecycle contract.

#### Scenario: Change-local processing error preserves process snapshot mode

**Given**: `/api/v2/state` reports `app_mode: running` for a multi-change run
**When**: `ProcessingError` is authoritatively dispatched for change `alpha`
**Then**: `alpha.display_status` is `error` and its sanitized `error_detail` is present
**And**: `alpha.execution_marked` reflects the same-revision mark reconciliation
**And**: `app_mode` remains `running`
**And**: `process_error` remains null
**And**: unrelated changes retain their current marks and action eligibility

#### Scenario: Fatal process error remains distinct

**Given**: a snapshot may already contain one or more change-level errors
**When**: a typed fatal `ExecutionEvent::Error` is authoritatively dispatched
**Then**: `app_mode` becomes `error`
**And**: `process_error` contains the sanitized fatal detail
**And**: change-level error details remain distinguishable from the process failure

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

<!-- Expected canonical result after archive: the authoritative snapshot requirement will explicitly separate change-local ProcessingError fields from process-wide app_mode/process_error. -->
