## ADDED Requirements

### Requirement: Change-Scoped Base-Lane Failure Events

Parallel orchestration MUST preserve change scope when a post-archive merge or conflict-resolution attempt fails after exhausting its bounded retries but leaves repository and worktree evidence available for explicit retry. The failure MUST produce one authoritative change-scoped lifecycle transition carrying the affected change ID, MUST return the change to reducer-owned `MergeWait`, and MUST NOT emit a generic global execution Error for the same underlying failure.

Merge, conflict-resolution, and queue-result layers MUST NOT each promote the same change-local failure into separate lifecycle errors. Internal background-task outcomes MUST represent change-local and run-fatal scope through typed variants or fields; implementations MUST NOT classify scope by matching diagnostic text.

After a change-scoped base-lane failure, orchestration MUST release transient lane ownership, preserve the failed change's workspace, and remain able to dispatch unrelated eligible changes. Global execution Error MUST remain available for failures that stop or invalidate the run and have no safe continuation.

#### Scenario: conflict exhaustion emits one change-scoped transition

- **GIVEN** archived change `alpha` enters post-archive base merge
- **AND** conflict resolution exhausts its configured attempts
- **WHEN** the background merge task reports its outcome
- **THEN** orchestration SHALL emit one `ResolveFailed` transition carrying change ID `alpha` and the failure detail
- **AND** it SHALL NOT emit `ExecutionEvent::Error` or a generic `ParallelEvent::Error` for that same failure
- **AND** reducer state for `alpha` SHALL become `MergeWait`
- **AND** the workspace for `alpha` SHALL remain available for explicit retry

#### Scenario: queue wrapper does not duplicate a classified failure

- **GIVEN** a post-archive background merge result already contains a typed change-local failure for `alpha`
- **WHEN** queue result handling releases the base lane and updates scheduler bookkeeping
- **THEN** it SHALL NOT wrap that result in another global Error event
- **AND** operator-facing lifecycle diagnostics SHALL retain the structured change ID
- **AND** duplicate global diagnostics SHALL NOT be appended for the same result

#### Scenario: unrelated work continues after change-local merge failure

- **GIVEN** change `alpha` has returned to manual `MergeWait` after conflict exhaustion
- **AND** unrelated change `beta` is eligible for ordinary dispatch
- **WHEN** the scheduler evaluates available work
- **THEN** `beta` SHALL remain dispatchable
- **AND** the scheduler SHALL remain alive
- **AND** orchestration SHALL NOT synthesize merge success or run completion for `alpha`

#### Scenario: run-fatal background outcome remains global

- **GIVEN** a background orchestration outcome invalidates the active run and has no safe continuation
- **WHEN** queue result handling receives the typed run-fatal outcome
- **THEN** orchestration SHALL emit a global execution Error
- **AND** change-local suppression rules SHALL NOT downgrade that event
