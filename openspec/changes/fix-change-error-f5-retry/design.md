# Design: Mode-Independent Start Retry Routing

## Context

`ProcessingError` is intentionally change-scoped. It records terminal error evidence and revokes the failed change's stale execution mark without moving the process into global Error. This keeps unrelated work operable, but it means process mode no longer identifies whether marked retryable work exists.

The current Start boundary still branches on mode before target evidence:

- Error calls marked retry planning.
- Select and Stopped admit only `not queued` marks.
- Running rejects Start.

That mode-first split makes the configured retry control unreachable for the normal change-local failure lifecycle.

## Decision

Use target evidence after a coherent mark/reducer read to choose Start intent, while retaining mode as a lifecycle guard.

| Process mode | Ordinary marked targets | Retry-eligible marked targets | Decision |
| --- | --- | --- | --- |
| Select / Stopped | one or more | any | ordinary Start; retry-only rows excluded |
| Select / Stopped | none | one or more | explicit retry |
| Running | ignored by F5 | one or more | explicit retry through live scheduler |
| Running | any | none | refusal; ordinary live admission remains mark settlement |
| Error | ignored | one or more | existing explicit retry |
| Stopping | any | any | refusal |

This is a fallback rather than a mixed command. `explicit_retry` is currently a run-level launch property, while retry safety also depends on target-specific reducer transitions and queue edges. Mixing ordinary and retry targets would broaden retry semantics to work that did not request them. Ordinary Start therefore retains priority and reports marked retry-only rows as exclusions.

## Admission and Commit Sequence

1. Acquire the shared operator application gate.
2. Read process mode and one coherent marked/reducer eligibility view.
3. Classify ordinary targets and retry routes without mutation.
4. Apply the existing complete-request worktree fence to the full marked set before selecting ordinary or retry-class admission, then apply retry-specific active-run guards to the selected retry class.
5. Prepare scheduler start or wake before mutation.
6. Commit either ordinary queue intent or existing retry routes.
7. Dispatch the authoritative accepted outcome and capture its revision.
8. Activate the prepared scheduler start or notification.

A refusal exits before step 6. No compensating rollback is required because no effect exists to undo.

## Retry Edge Ownership

Terminal error retry remains `ReducerCommand::RetryError`. Its state-changing outcome publishes one target-ID-bearing explicit-retry edge to `DynamicQueue`. The scheduler consumes that edge to release only the matching ephemeral failed classification and to establish an immediate reanalysis reason.

An ordinary `AddToQueue`, generic scheduler notification, execution mark, or mark-settlement deadline cannot substitute for this edge. Those signals do not prove that terminal error evidence was intentionally cleared.

## Runtime-Limit Semantics

`OrchestratorError::RuntimeLimit` prevents the terminated invocation from being automatically retried in the same scheduler handling cycle. Once `ProcessingError` has settled, a later F5 is a new operator command. It is accepted only if the ordinary shared retry classifier finds retryable evidence and no active iteration-limit owner blocks the target.

## Interaction with Running Mark Settlement

This change depends on `restore-running-mark-reanalysis`. After that dependency is implemented, Running-mode ordinary marks continue to settle into the current run after the configured stability interval. Retry statuses remain excluded from settlement. F5 on a retry-eligible mark bypasses settlement because it is explicit operator retry intent; F5 does not arm, reset, or consume the ordinary mark deadline.

## Verification Strategy

- Run-control unit tests cover mode and target-class matrices plus mutation-free refusals.
- Coordinator/cross-adapter tests cover atomic ordering, exact side-effect cardinality, and TUI/API parity.
- Scheduler component coverage observes the target-specific retry edge and `AnalysisStarted`, preventing a reducer-only or notification-only stub from passing.
- Runtime-limit coverage separates absence of automatic retry from success of a later explicit request.
- Tests use event ordering, channels, and paused Tokio time rather than short elapsed-time thresholds.

## Rejected Alternatives

### Return to process-wide Error on ProcessingError

Rejected because it would reintroduce the failure corrected by `preserve-run-mode-on-change-error`: one failed change would disable unrelated controls and misrepresent scheduler health.

### Make error marks enter delayed mark settlement

Rejected because marking is next-run intent, not retry authorization. It would create implicit retries and bypass `RetryError` edge ownership.

### Always combine ordinary and retry targets

Rejected because current scheduler startup carries one run-level `explicit_retry` flag. A mixed launch would apply retry-specific startup behavior to ordinary work and obscure target-specific auditability.

### Add a dedicated retry key or API command

Rejected because explicit retry commands already exist and configured Start/F5 is the documented app-level recovery control. The defect is shared Start routing, not missing surface area.
