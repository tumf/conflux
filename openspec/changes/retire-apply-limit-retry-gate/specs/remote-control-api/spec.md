## MODIFIED Requirements

### Requirement: Authoritative operator snapshot

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs. A change-scoped `ProcessingError` MUST update the failed change's display status, bounded sanitized error detail, activity, attention, action eligibility, and reconciled execution mark without changing `app_mode` or setting `process_error`. Only a typed process-fatal event MAY set process-wide Error state. A settled terminal-error change that retains typed Apply iteration-limit evidence MUST expose ordinary retry eligibility when its evidence otherwise supports retry, even while a persistent scheduler task remains live. Projection and command admission MUST use the same retry classifier. The retained iteration-limit record MAY remain visible as diagnostic evidence but MUST NOT be an operator-action block. A headless `cflx run` process with no bound command executor continues to expose read-only state while command submission remains unavailable through the existing unbound-runtime lifecycle contract.

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

#### Scenario: Settled Apply limit remains diagnostic rather than blocking

**Given**: A command-capable owner retains `ApplyIterationLimit` for terminal-error change `alpha`
**And**: The persistent scheduler task remains live
**When**: A client reads `/api/v2/state`
**Then**: `alpha.actions.retry_change.allowed` is true when terminal-error evidence otherwise supports retry
**And**: the retained attempts/max and error detail remain observable
**And**: no `apply_iteration_limit_active` blocked reason is projected
**And**: no client must parse the diagnostic to determine retry eligibility

#### Scenario: Snapshot and command admission agree

**Given**: `/api/v2/state` exposes Retry for a settled Apply-limit error
**When**: A client submits `retry_change` using that state revision
**Then**: command admission accepts the same retry classification
**And**: one explicit retry edge is published
**And**: a generic refresh or scheduler notification without that command does not retry the target

#### Scenario: Headless read-only projection remains non-actionable

**Given**: `cflx run` serves `/api/v2` without a bound command executor
**And**: Its shared state retains typed iteration-limit evidence after the run
**When**: A client reads the subsequent snapshot
**Then**: The diagnostic may remain observable
**And**: A submitted command is refused by the existing unbound-runtime lifecycle contract

<!-- Expected canonical result after archive: the authoritative snapshot treats retained Apply-limit evidence as diagnostic and exposes retry eligibility consistent with explicit command admission. -->
