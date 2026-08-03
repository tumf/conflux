---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/separate-tui-execution-modal-state/proposal.md
  - openspec/changes/fix-resolve-merge-continuation/proposal.md
  - src/parallel/conflict.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/parallel/types.rs
  - src/orchestration/state.rs
  - src/tui/orchestrator.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/output.rs
  - src/web/remote_control_api/projection.rs
  - src/web/operator_facts.rs
verifications:
  - id: change-local-merge-error-tests
    requirement: "Bounded post-archive resolve exhaustion remains change-scoped, preserves retryable MergeWait state, reports finite completion truthfully, and reserves global Error for outcomes that abort the run"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering exhaustive typed outcomes, producer event sequences, finite and persistent scheduler behavior, fatal abort, reducer state, TUI/Web/lifecycle projection, and absence of duplicate global Error events"
    rerun: "cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo test --lib lifecycle_integration && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Keep change-local merge failures out of global TUI Error

**Change Type**: implementation

## Premise / Context

- A real TUI run observed `unify-remote-operator-commands` exhaust three conflict-resolution attempts and correctly receive `ResolveFailed`, which returned that change to `merge wait`.
- The same post-archive merge failure was then emitted as global `ParallelEvent::Error` by both the merge layer and its queue-result wrapper.
- TUI global error handling changed the whole application to `AppMode::Error`; the retained terminal-mode rule kept that stale mode for hours and rejected bulk mark even though the persistent scheduler remained alive.
- Canonical `tui-error-handling` reserves global Error for failures that stop or invalidate the active run, while canonical scheduler termination permits a finite run to finish with manual `MergeWait` work remaining.
- `separate-tui-execution-modal-state` intentionally excludes producer reclassification. `fix-resolve-merge-continuation` overlaps merge/resolve files but does not own this severity contract. Neither is a hard dependency.

## Problem / Context

Bounded conflict exhaustion already has a typed change-scoped transition: `ResolveFailed` restores `MergeWait` for explicit retry. The background task boundary then erases failure meaning into `Result<MergeTaskOutcome, String>`, so merge and queue wrappers promote the same failure to process-scoped `ExecutionEvent::Error`. The frontend cannot recover the lost scope because the global event has no change ID and is contractually fatal.

The erased `Err(String)` also mixes already-reported publication and hook failures with failures that leave repository truth unknown. Simply suppressing every post-archive `Err` would hide genuine run-fatal failures. Simply removing Error events would also let a finite scheduler report false success after change-local failures. Classification, scheduler disposition, and terminal reporting therefore need one typed end-to-end contract.

## Proposed Solution

Replace the bare background merge `Result<MergeTaskOutcome, String>` boundary with exhaustive typed outcomes:

- `Merged`;
- `Deferred { reason, auto_resumable }`;
- `ResolveExhausted { change_id, attempts, classification, detail }` for bounded conflict exhaustion after repository/worktree evidence is preserved;
- `RecoverableAlreadyReported { change_id, kind, detail }` for failures whose existing typed owner, such as `PushFailed` or `HookFailed`, has already emitted the change transition;
- `RunFatal { detail }` for failures that leave base/run truth unsafe or provide no safe continuation.

Names MAY be adjusted to local style, but these five exhaustive semantics and fields are required. Implementations MUST NOT infer scope from diagnostic substrings or `MergeResultOrigin` alone.

The conflict layer SHALL emit `ResolveFailed { change_id, error }` exactly once as the authoritative lifecycle transition for `ResolveExhausted`. `ConflictResolutionFailed` MAY remain presentation-only telemetry but SHALL NOT mutate reducer state, `process_error`, TUI execution mode, or lifecycle state. Merge and queue wrappers SHALL NOT emit another global Error for `ResolveExhausted` or `RecoverableAlreadyReported`.

Queue result handling SHALL return a typed scheduler disposition: `Merged`, `ContinueWithErrors`, or `AbortRun`. `RunFatal` has one global event owner at the queue/orchestration boundary, emits global Error once, stops new dispatch, performs bounded drain of in-flight tasks and pending base-lane results, and terminates the scheduler future as failure.

Persistent schedulers SHALL continue after `ContinueWithErrors` and may dispatch unrelated eligible changes. Finite schedulers SHALL remember that change failures occurred and, once eligible work drains, report `CompletedWithErrors`: emit no global Error and no success message, emit the existing `AllCompleted` terminal event, and preserve manual `MergeWait` for explicit retry.

## Acceptance Criteria

1. Bounded conflict exhaustion emits one authoritative `ResolveFailed` carrying the change ID, attempt count, and bounded final failure classification, with no global Error for the same failure.
2. The failed change remains `MergeWait`, preserves its worktree and repository evidence, and remains available through the existing explicit resolve path.
3. Background merge outcomes exhaustively distinguish `Merged`, `Deferred`, `ResolveExhausted`, `RecoverableAlreadyReported`, and `RunFatal`; no scope decision matches rendered text.
4. Queue handling exhaustively maps those outcomes to `Merged`, `ContinueWithErrors`, or `AbortRun` and releases lane/counter ownership independently of severity.
5. A persistent scheduler continues after `ResolveExhausted`; an unrelated and non-dependent eligible change remains dispatchable, while dependents of the failed change remain blocked.
6. A finite scheduler with one or more change-local failures terminates after eligible work drains as `CompletedWithErrors`, emits `AllCompleted`, emits no success message and no global Error, and preserves the affected `MergeWait` state.
7. `RunFatal` emits one global Error, stops new dispatch, bounded-drains owned work, and returns scheduler failure; TUI enters global Error only for this path.
8. Existing `PushFailed` and `HookFailed` owners do not fall through to `RunFatal` or duplicate global Error merely because the background task reports failure.
9. TUI, Web, and external lifecycle projections preserve scope: `resolve_failed(change_id=alpha)` is emitted once, Web `process_error` remains unset, TUI does not enter Error, and no process-scoped lifecycle Error is projected for `ResolveExhausted`.
10. `ConflictResolutionFailed`, if retained, remains presentation-only and non-authoritative.

## Explicit Completion Conditions

- `src/parallel/types.rs` no longer exposes a bare string error at the background base-lane result boundary; exhaustive typed outcome and disposition matches compile.
- Every current error producer reaching the background merge queue is mapped according to the classification table in `design.md`, including publication, hooks, repository invariant failures, and detached HEAD/base identity failures.
- `src/parallel/conflict.rs` and `src/parallel/merge.rs` have one typed owner for exhausted change-local failures; `src/parallel/queue_state.rs` has one owner for global fatal emission.
- `src/parallel/orchestration.rs` stops dispatch and bounded-drains on `AbortRun`, and tracks change-local failures for finite `CompletedWithErrors` reporting.
- `src/tui/orchestrator.rs` distinguishes `Completed`, `CompletedWithErrors`, `Stopped`, and `Failed` without truthful-completion regressions.
- Tests cover persistent continuation, finite completed-with-errors, fatal abort, lane release, dependency blocking, Web/operator facts, external lifecycle projection, TUI mode, and duplicate suppression.
- `cargo test --lib parallel::tests::`, `cargo test --lib tui::`, `cargo test --lib lifecycle_integration`, `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Scope Rationale

Producer classification, queue disposition, scheduler termination, reducer transition, and frontend projection must ship together. Changing only the frontend would conceal a misclassified event; changing only the producer would create false-success finite completion or leave fatal scheduler behavior undefined. Publication and hook logic are not redesigned, but their already-reported outcomes must cross the shared result boundary without accidental fatal promotion.

## Out of Scope

- Separating TUI execution and modal state; that remains owned by `separate-tui-execution-modal-state`.
- Changing conflict-resolution retry count, merge algorithm, key bindings, bulk-mark policy, or explicit retry ownership.
- Redesigning publication or hook retry policy beyond preserving their existing typed event ownership at the shared outcome boundary.
- Automatically retrying manual `MergeWait` failures.
- Persisting workflow decisions outside repository/workspace evidence.
- Reclassifying unrelated startup, dependency-analysis, or repository-wide failures without a typed background base-lane outcome.
