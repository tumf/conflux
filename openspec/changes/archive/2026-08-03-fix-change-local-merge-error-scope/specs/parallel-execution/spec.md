## ADDED Requirements

### Requirement: Typed Base-Lane Outcomes and Scheduler Dispositions

Parallel orchestration MUST represent every background base-lane result through an exhaustive typed outcome equivalent to `Merged`, `Deferred`, `ResolveExhausted`, `RecoverableAlreadyReported`, or `RunFatal`. The boundary MUST NOT use a bare string error, diagnostic substring matching, or merge-result origin alone to infer failure scope.

`ResolveExhausted` MUST be limited to bounded conflict-resolution exhaustion after repository and worktree evidence has been preserved for explicit retry. It MUST carry the affected change ID, attempts exhausted, a bounded machine-readable final failure classification, and sanitized bounded detail. Publication or hook failures that already emitted `PushFailed` or `HookFailed` MUST cross the shared boundary as `RecoverableAlreadyReported` and MUST NOT fall through to `RunFatal`.

Base identity loss including detached HEAD without a safe captured base, repository/conflict-query failure before a change-scoped transition, uncertain post-merge verification, and unknown internal failures MUST fail closed as `RunFatal`.

Queue handling MUST map typed outcomes to scheduler dispositions equivalent to `Merged`, `ContinueWithErrors`, or `AbortRun`. Pending counters and base-lane ownership MUST be released independently of the disposition.

#### Scenario: bounded conflict exhaustion is typed change-local failure

- **GIVEN** archived change `alpha` enters post-archive base merge
- **AND** conflict resolution exhausts its configured attempts
- **AND** the workspace and repository evidence remain available for explicit retry
- **WHEN** the background merge task reports its outcome
- **THEN** the outcome SHALL be `ResolveExhausted` with change ID `alpha`, attempt count, final failure classification, and bounded detail
- **AND** conflict handling SHALL emit one authoritative `ResolveFailed` for `alpha`
- **AND** merge and queue wrappers SHALL NOT emit a global Error for the same failure
- **AND** reducer state for `alpha` SHALL become `MergeWait`

#### Scenario: already-reported publication failure is not promoted

- **GIVEN** publication for `alpha` emits `PushFailed`
- **WHEN** the background result crosses the shared base-lane boundary
- **THEN** it SHALL be represented as `RecoverableAlreadyReported` with Push kind
- **AND** queue handling SHALL preserve the existing PushFailed reducer and retry semantics
- **AND** it SHALL NOT emit a duplicate global Error

#### Scenario: already-reported hook failure is not promoted

- **GIVEN** a change hook for `alpha` emits `HookFailed`
- **WHEN** the background result crosses the shared base-lane boundary
- **THEN** it SHALL be represented as `RecoverableAlreadyReported` with Hook kind
- **AND** queue handling SHALL preserve the existing HookFailed state semantics
- **AND** it SHALL NOT emit a duplicate global Error

#### Scenario: unsafe repository truth fails closed

- **GIVEN** a background base-lane operation cannot identify a safe base, cannot establish repository truth before a change-scoped transition, or cannot verify post-merge integration
- **WHEN** the failure is classified
- **THEN** the outcome SHALL be `RunFatal`
- **AND** it SHALL NOT be inferred as change-local from its diagnostic text or origin

### Requirement: Change-Scoped Base-Lane Failure Events

Parallel orchestration MUST preserve change scope when bounded post-archive conflict resolution exhausts retries but leaves repository and worktree evidence available for explicit retry. The conflict layer MUST produce one authoritative `ResolveFailed` lifecycle transition carrying the affected change ID. `ConflictResolutionFailed` MAY remain presentation-only telemetry but MUST NOT mutate reducer state, TUI execution mode, Web `process_error`, or external lifecycle state.

Merge, conflict-resolution, and queue-result layers MUST NOT each promote the same change-local failure into separate lifecycle errors. Operator detail MUST include attempts exhausted, the final bounded failure classification, and a sanitized bounded summary; unbounded agent output MUST remain in observability output rather than lifecycle state.

#### Scenario: queue wrapper does not duplicate a classified failure

- **GIVEN** a background merge result already contains `ResolveExhausted` for `alpha`
- **WHEN** queue result handling releases the base lane and updates scheduler bookkeeping
- **THEN** it SHALL return `ContinueWithErrors`
- **AND** it SHALL NOT wrap the result in another global Error event
- **AND** operator-facing lifecycle diagnostics SHALL retain structured change ID `alpha`

#### Scenario: presentation telemetry remains non-authoritative

- **GIVEN** conflict exhaustion emits `ConflictResolutionFailed` presentation telemetry and `ResolveFailed` state transition
- **WHEN** frontends project both ordered events
- **THEN** `ResolveFailed` SHALL be the only workflow-state owner
- **AND** presentation telemetry SHALL NOT set process-level error or blocked state

### Requirement: Scheduler Continuation, Fatal Abort, and Truthful Completion

A `ContinueWithErrors` disposition MUST record invocation-scoped change failure without globally invalidating the run. In persistent lifetime, the scheduler MUST remain available for dynamic queue notifications and MUST allow unrelated non-dependent eligible changes to dispatch. Changes depending on the failed change MUST remain blocked by existing dependency state.

In finite lifetime, manual `MergeWait` MUST continue to permit scheduler termination under the canonical scheduler-loop requirement. When eligible work drains after one or more `ContinueWithErrors` outcomes, the run MUST terminate as completed with errors: it MUST emit no global Error and no success message, MUST emit a warning diagnostic and the existing `AllCompleted` terminal event, and MUST preserve manual `MergeWait` and workspace evidence.

An `AbortRun` disposition MUST emit one global Error from a single queue/orchestration owner, stop admission of new work, bounded-drain in-flight tasks and pending base-lane results through managed cleanup, and return scheduler failure. A frontend Error presentation without corresponding run invalidation is forbidden.

#### Scenario: persistent run continues unrelated work

- **GIVEN** persistent execution records `ResolveExhausted` for `alpha`
- **AND** ordinary change `beta` has no dependency on `alpha` and is eligible
- **AND** change `gamma` depends on `alpha`
- **WHEN** the scheduler reevaluates work
- **THEN** `beta` SHALL remain dispatchable
- **AND** `gamma` SHALL remain blocked
- **AND** the scheduler SHALL remain alive
- **AND** `alpha` SHALL remain manual `MergeWait`

#### Scenario: finite run completes with errors truthfully

- **GIVEN** finite execution records `ResolveExhausted` for `alpha`
- **AND** all other eligible work is drained
- **WHEN** the scheduler reaches its termination condition
- **THEN** it SHALL report completed with errors
- **AND** it SHALL emit a warning and `AllCompleted`
- **AND** it SHALL NOT emit a success message or global Error
- **AND** `alpha` SHALL remain manual `MergeWait` for later explicit retry

#### Scenario: run-fatal outcome aborts execution

- **GIVEN** queue handling receives `RunFatal`
- **WHEN** it returns `AbortRun`
- **THEN** orchestration SHALL emit one global Error
- **AND** it SHALL stop new dispatch
- **AND** it SHALL bounded-drain owned in-flight and base-lane work
- **AND** the scheduler future SHALL terminate as failure

### Requirement: Change-Scoped Failure Projection

TUI, Web, and external lifecycle adapters MUST preserve the typed scope of `ResolveExhausted`. The ordered frontend stream MUST contain one `resolve_failed` projection with the affected change ID, Web `process_error` MUST remain unset, TUI execution mode MUST remain non-Error, and external lifecycle projection MUST NOT report process-scoped Error or Blocked solely because of the exhausted change. Optional `conflict_resolution_failed` projection MUST remain presentation-only.

A `RunFatal` global Error MUST continue to project as process-fatal across adapters after the scheduler begins aborting the run.

#### Scenario: exhausted resolve remains change-scoped across adapters

- **GIVEN** `ResolveExhausted` for change `alpha`
- **WHEN** reducer, TUI, Web, and external lifecycle adapters consume its events
- **THEN** `resolve_failed` SHALL appear once with change ID `alpha`
- **AND** Web `process_error` SHALL remain unset
- **AND** TUI execution mode SHALL not become Error
- **AND** external lifecycle state SHALL not become process Error or Blocked solely for `alpha`

#### Scenario: run-fatal remains process-fatal across adapters

- **GIVEN** `RunFatal` begins `AbortRun`
- **WHEN** the global Error is projected
- **THEN** TUI and Web SHALL expose process-fatal Error according to their existing contracts
- **AND** external lifecycle projection SHALL expose the fatal run state
- **AND** change-local suppression SHALL NOT downgrade it
