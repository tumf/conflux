## Context

A post-archive merge conflict currently produces a correct change-scoped sequence and then an incorrect global sequence. Conflict exhaustion emits `ResolveFailed { change_id, error }`; the reducer and TUI convert that to `MergeWait`. The enclosing merge function also emits `ParallelEvent::Error`, returns an error, and `handle_merge_result_with_tx` emits a second `ParallelEvent::Error` for `PostArchiveMerge`. Every generic Error becomes process-scoped `ExecutionEvent::Error`, so the TUI enters retained global Error while the persistent scheduler continues waiting for work.

The event contract already states that `ProcessingError` and phase failures carry change identity, while `ExecutionEvent::Error` carries no change identity and represents execution-wide failure. The correction belongs at the producer boundary, not in frontend message filtering.

## Goals

- Preserve change identity through exhausted post-archive merge and resolve outcomes.
- Keep reducer-owned `MergeWait` and explicit retry behavior authoritative.
- Eliminate duplicate process-scoped promotion of the same failure.
- Keep the persistent scheduler and unrelated change execution available.
- Preserve genuine global fatal behavior.

## Non-Goals

- Alter conflict resolution commands or retry budgets.
- Turn manual merge failures into automatic retries.
- Redesign the complete execution event taxonomy.
- Change TUI modal representation or command admission.
- Infer event scope from error strings.

## Event Ownership

The lowest layer that can classify the failed operation owns the lifecycle event:

1. conflict resolution exhaustion emits one `ResolveFailed { change_id, error }` per affected change;
2. the merge task returns an outcome that remains identifiable as change-local failure;
3. queue result handling performs lane release and scheduler bookkeeping but does not emit a second lifecycle failure;
4. TUI/Web/lifecycle adapters project the typed event without reclassifying its message.

`ExecutionEvent::Error` remains reserved for failures with no safe run continuation, such as orchestration startup or finalization failure that invalidates the whole run.

## Outcome Representation

Prefer the smallest typed change that prevents string classification. If existing control flow can prove that every resolve-exhaustion error has already emitted `ResolveFailed`, remove the redundant generic emissions and retain a typed internal result. If `MergeResult.outcome: Result<MergeTaskOutcome, String>` conflates run-fatal and change-local failures at the queue boundary, extend `MergeTaskOutcome` or introduce a minimal internal failure enum carrying:

- the scope (`change-local` or `run-fatal`);
- the associated change ID for change-local outcomes;
- the diagnostic detail.

The queue layer may emit global Error only for the run-fatal variant. It must not inspect message text or maintain a list of substrings.

## State and Scheduler Semantics

For exhausted change-local merge resolution:

- reducer state becomes idle plus `WaitState::MergeWait`;
- queue intent remains not queued until explicit retry;
- the worktree and branch remain preserved;
- lane reservations and counters are released through existing cleanup paths;
- the persistent scheduler continues and may dispatch unrelated eligible changes;
- no `AllCompleted` or success event is synthesized for the failed change.

The TUI receives `ResolveFailed`, shows the existing warning/error diagnostic, and invokes its existing active-work check. It remains Running when other active work exists and may transition to Select when none remains. It never enters global Error for this sequence.

## Diagnostic Semantics

One lifecycle failure event does not require one log line total: lower layers may retain tracing diagnostics. Operator-facing event logs, however, must not include duplicate process-scoped errors for the same change-local failure. The authoritative frontend diagnostic must carry the change ID and complete failure reason.

## Compatibility

- Existing `ResolveFailed` consumers remain valid.
- Existing explicit `ResolveMerge` retry remains the recovery path.
- Canonical global `Error` behavior is unchanged for truly run-fatal events.
- `separate-tui-execution-modal-state` may land before or after this change. It preserves event classification, while this change corrects that classification.
- No durable state is introduced; restart behavior remains derived from workspace and Git evidence under `openspec/CONSTITUTION.md`.

## Verification Strategy

Use deterministic unit/integration tests with event channels and reducer/TUI state, not a real agent command:

- drive conflict-exhaustion or its merge-result fixture and assert `ResolveFailed` with the expected change ID;
- collect the full emitted event sequence and assert no `ExecutionEvent::Error` or `ParallelEvent::Error` represents that failure;
- apply the sequence to reducer and TUI state and assert `MergeWait` plus non-Error execution mode;
- queue an unrelated eligible change and prove scheduler dispatch remains available after the failed merge result;
- assert worktree-preservation and lane-release bookkeeping remain intact;
- inject a separately typed run-fatal result and assert global Error still emits and TUI enters Error;
- assert repeated wrapper handling does not duplicate the operator-facing failure.

Tests must remain under one second or use the repository heavy-test policy. The expected cases use service doubles and in-memory channels and should stay in the default suite.

## Risks and Mitigations

- **Accidentally suppressing a true global error:** represent scope in a typed result and retain a dedicated run-fatal regression test.
- **Leaking base-lane ownership:** keep lane release independent from event severity and assert counters/reservations clear.
- **Losing diagnostics:** assert the change-scoped event retains full detail and structured change ID.
- **Frontend drift:** apply the exact producer sequence to TUI state instead of testing only an isolated handler.
