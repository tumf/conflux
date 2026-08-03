## Context

A post-archive merge conflict currently produces a correct change-scoped sequence and then an incorrect global sequence. Conflict exhaustion emits `ResolveFailed { change_id, error }`; the reducer and TUI convert that to `MergeWait`. The enclosing merge function also emits `ParallelEvent::Error`, returns an error, and `handle_merge_result_with_tx` emits a second `ParallelEvent::Error` for `PostArchiveMerge`. Every generic Error becomes process-scoped `ExecutionEvent::Error`, so the TUI enters retained global Error while the persistent scheduler continues waiting for work.

The background task boundary is `MergeResult.outcome: Result<MergeTaskOutcome, String>`. That string erases whether a typed change event was already emitted, whether manual retry evidence remains, and whether repository truth is unsafe. It also prevents the finite scheduler from distinguishing successful completion from completion with unresolved change-local failures.

## Goals

- Preserve change identity through exhausted post-archive resolve outcomes.
- Make every background base-lane outcome exhaustive and typed.
- Keep reducer-owned `MergeWait` and explicit retry authoritative.
- Eliminate duplicate process-scoped promotion of already-reported failures.
- Preserve persistent continuation and truthful finite completion.
- Make a genuine run-fatal outcome actually abort the run, not only change frontend presentation.
- Keep TUI, Web, and lifecycle projections aligned.

## Non-Goals

- Alter conflict resolution commands or retry budgets.
- Turn manual merge failures into automatic retries.
- Redesign publication or hook retry policy.
- Redesign the complete execution event taxonomy.
- Change TUI modal representation or command admission.
- Infer event scope from strings or origin names.

## Required Outcome Contract

Replace the bare `Result<MergeTaskOutcome, String>` background boundary with an exhaustive outcome equivalent to:

```rust
enum MergeTaskOutcome {
    Merged,
    Deferred {
        reason: String,
        auto_resumable: bool,
    },
    ResolveExhausted {
        change_id: String,
        attempts: u32,
        classification: ResolveFailureClassification,
        detail: String,
    },
    RecoverableAlreadyReported {
        change_id: String,
        kind: AlreadyReportedFailureKind,
        detail: String,
    },
    RunFatal {
        detail: String,
    },
}

enum AlreadyReportedFailureKind {
    Push,
    Hook,
}

enum MergeResultDisposition {
    Merged,
    ContinueWithErrors,
    AbortRun,
}
```

Exact Rust names may follow local conventions, but the variants, semantic distinctions, and exhaustive fields are mandatory. `ResolveFailureClassification` must be bounded and machine-readable; it may describe the final failure class such as unresolved conflict, agent/protocol failure, or retry exhaustion, but must not embed unbounded agent output.

## Error Classification Table

| Producer condition reaching the background result boundary | Required outcome | Existing lifecycle owner |
|---|---|---|
| Merge and cleanup complete, or repository-visible evidence proves already integrated | `Merged` | merge producer success events |
| Work remains deferred under existing auto/manual wait policy | `Deferred` | existing deferred event/state owner |
| Bounded conflict resolution exhausts retries after workspace/repository evidence is preserved | `ResolveExhausted` | conflict layer emits one `ResolveFailed` per change |
| Publication fails after `PushFailed` was emitted | `RecoverableAlreadyReported(kind = Push)` | `PushFailed` |
| Change hook fails after `HookFailed` was emitted | `RecoverableAlreadyReported(kind = Hook)` | `HookFailed` |
| Base branch cannot be identified, including detached HEAD where no safe base identity exists | `RunFatal` | queue/orchestration global owner |
| Conflict detection or repository query fails before a change-scoped failure transition can be established | `RunFatal` | queue/orchestration global owner |
| Post-merge verification leaves base integration truth unknown | `RunFatal` | queue/orchestration global owner |
| Internal invariant failure or unknown background failure not already represented by a typed change event | `RunFatal` | queue/orchestration global owner |

Unknown outcomes fail closed as `RunFatal`; they are never inferred to be change-local from `MergeResultOrigin` or message content.

## Event Ownership

1. The conflict layer classifies bounded exhaustion and emits exactly one `ResolveFailed { change_id, error }` for each affected change.
2. The merge task returns `ResolveExhausted`; it does not emit generic Error for the same failure.
3. Existing `PushFailed` and `HookFailed` producers remain lifecycle owners, then return `RecoverableAlreadyReported` through the shared boundary.
4. Queue result handling releases counters and base-lane ownership, maps the outcome to a scheduler disposition, and does not duplicate an already-owned change event.
5. Queue/orchestration owns the sole global Error for `RunFatal` and returns `AbortRun`.
6. TUI, Web, and external lifecycle adapters project typed events without message-based reclassification.

`ConflictResolutionFailed` may remain ordered presentation telemetry. It is not an authoritative lifecycle event, does not carry workflow state, and must not change reducer state, TUI execution mode, Web `process_error`, or external lifecycle state.

## Diagnostic Contract

`ResolveFailed.error` must contain bounded operator detail with:

- the number of attempts exhausted;
- the final `ResolveFailureClassification` token or stable label;
- a sanitized bounded summary of the last failure.

Raw or unbounded agent output remains in tracing/output events and is not copied into lifecycle state. Lower layers may log context, but frontend event streams must not contain duplicate global errors for an already-classified failure.

## Scheduler Disposition

Queue result handling returns:

- `Merged` for merge success;
- `ContinueWithErrors` for `ResolveExhausted` and `RecoverableAlreadyReported`;
- existing non-terminal continuation for `Deferred`, without marking an error when no failure occurred;
- `AbortRun` for `RunFatal`.

Lane and counter release is independent of disposition. `ContinueWithErrors` records invocation-scoped `had_change_failures`; it does not globally stop the scheduler. `AbortRun` stops admission of new work, cancels or prevents new dispatch, bounded-drains in-flight tasks and pending merge/base-lane results through the existing managed cleanup path, and returns scheduler failure. The global Error is emitted once before failure termination by the queue/orchestration owner.

## Persistent and Finite Lifetimes

### Persistent

After `ContinueWithErrors`, the scheduler remains available for dynamic queue notifications. An ordinary non-dependent change may dispatch. A change depending on the failed change remains blocked by existing dependency state. The failed change remains manual `MergeWait` until explicit retry.

### Finite

Manual `MergeWait` does not prevent finite scheduler termination under the canonical scheduler-loop requirement. If eligible work drains after one or more `ContinueWithErrors` outcomes, the scheduler returns a completed-with-errors report rather than success or failure:

- emit no global Error;
- emit no success log;
- emit a warning such as `Processing completed with errors`;
- emit the existing `AllCompleted` terminal event;
- preserve `MergeWait` and worktree evidence for later explicit retry.

The TUI boundary must classify terminal results as `Completed`, `CompletedWithErrors`, `Stopped`, or `Failed`. `Failed` is reserved for `AbortRun` or another scheduler future failure.

## Frontend Projection

For `ResolveExhausted` affecting `alpha`:

- reducer and TUI receive exactly one authoritative `ResolveFailed(change_id = alpha)`;
- TUI row becomes `merge wait` and execution mode remains Running while other active work exists, otherwise existing active-work logic may choose Select;
- Web ordered event projection emits one `resolve_failed` event with change ID `alpha`;
- Web operator facts may retain change-local diagnostic detail but `process_error` remains unset;
- external lifecycle projection does not emit a process-scoped Error or Blocked state solely for this failure;
- optional `conflict_resolution_failed` remains presentation-only.

For `RunFatal`, global Error projection remains unchanged and the scheduler failure confirms that the active run is actually invalidated.

## Compatibility and Integration Order

- Existing `ResolveFailed`, `PushFailed`, and `HookFailed` consumers remain valid.
- Existing explicit `ResolveMerge` retry remains the recovery path.
- `separate-tui-execution-modal-state` may land before or after this change; it preserves classification while this change corrects the producer.
- `fix-resolve-merge-continuation` overlaps `conflict.rs` and `merge.rs`; either integration order must preserve the exhaustive outcome table and include a merge/resolve continuation regression after rebase.
- No durable runtime state is added. `had_change_failures` and terminal disposition are invocation-scoped; restart behavior remains derived from workspace and Git evidence under `openspec/CONSTITUTION.md`.

## Verification Strategy

Use deterministic in-memory channels, reducer state, and service doubles:

- collect bounded conflict-exhaustion events and assert one `ResolveFailed`, optional presentation telemetry, and zero global Error;
- exhaustively construct every outcome variant and assert queue disposition, event owner, counter release, and lane release;
- assert `PushFailed` and `HookFailed` paths return already-reported outcomes without duplicate global Error;
- assert detached HEAD/base identity failure, pre-transition repository failure, unknown invariant failure, and post-merge uncertain verification map to `RunFatal`;
- prove `AbortRun` prevents new dispatch, bounded-drains owned work, emits one global Error, and returns scheduler failure;
- prove persistent `ContinueWithErrors` accepts unrelated `beta` while a dependent remains blocked;
- prove finite `ContinueWithErrors` ends as `CompletedWithErrors`, emits warning plus `AllCompleted`, emits no success/global Error, and preserves `MergeWait`;
- apply the real event sequence to reducer/TUI state and assert non-Error mode;
- assert Web `resolve_failed(change_id=alpha)` once and `process_error == None`;
- assert external lifecycle projection remains non-process-fatal for exhaustion and process-fatal for `RunFatal`;
- retain the existing merge/resolve continuation behavior regardless of integration order with `fix-resolve-merge-continuation`.

Tests must remain under one second or use the repository heavy-test policy. Detached-HEAD integration setup may use the existing heavy classification if it cannot meet the default threshold; outcome and control-flow tests remain fast and default.

## Risks and Mitigations

- **Accidentally suppressing a true global error:** fail closed to `RunFatal`, require exhaustive matches, and retain fatal abort tests.
- **Frontend Error without scheduler abort:** bind global Error ownership to `AbortRun` and test new dispatch stops.
- **False successful finite completion:** add invocation-scoped `had_change_failures` and `CompletedWithErrors` terminal reporting.
- **Leaking base-lane ownership:** release lane/counters independently before applying disposition and test every outcome.
- **Losing diagnostics:** require bounded attempt count, classification, and summary on `ResolveFailed` while keeping raw output in observability events.
- **Publication/hook regression:** preserve existing typed event ownership and represent shared-boundary fallout as already reported.
