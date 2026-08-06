## Context

The stop path crosses three state layers:

1. the scheduler reaches its cancellation-safe barrier and emits process-level `ExecutionEvent::Stopped`;
2. the shared `OrchestratorState` reducer owns canonical per-change activity, waits, queue intent, and display status;
3. TUI and Web/API project that reducer state while retaining frontend-only process mode, controls, logs, timing, and execution marks.

The current event updates only the third layer. `AppState::handle_stopped` rewrites selected display-cache strings, while the reducer remains active. Because later display synchronization treats the reducer as authoritative, the local repair is temporary.

## Goals

- Make process-level stop a reducer-visible run boundary.
- Reconcile every interrupted transient state to `not queued` without inventing a per-change terminal outcome.
- Preserve execution marks, repository evidence, terminal outcomes, and explicit resume behavior.
- Keep TUI and `/api/v2` projections coherent with one reducer transition.
- Ignore stale lifecycle events from the stopped run until an explicit requeue.

## Non-Goals

- Changing scheduler cancellation or child-process cleanup.
- Persisting runtime state.
- Changing header presentation or internal TUI mode vocabulary.
- Changing the per-change stop-and-dequeue API.

## Decision: Reconcile Run-Owned Non-Terminal Rows

A global `Stopped` event targets only reducer entries that are non-terminal and still carry evidence of ownership by the ending run:

- `activity != Idle`, or
- `queue_intent == Queued`, or
- `wait_state != None`.

Fresh idle `NotQueued` rows are outside the stopped run and remain untouched. Existing terminal rows also remain untouched, including recoverable `Error`: a process stop does not erase a change outcome.

Each targeted row is returned to the same non-terminal idle queue-off shape used by successful dequeue:

- `activity = Idle`;
- `queue_intent = NotQueued`;
- `wait_state = None`;
- `terminal = None`;
- blocker and commit-phase presentation cleared;
- resolve/reject/stall scheduler membership cleared;
- process-local dequeue guard set.

The guard is required because the stop event is a terminal process boundary. A late `AcceptanceStarted`, `ArchiveStarted`, or equivalent event from the cancelled run must not reactivate the row. Existing explicit queue commands clear the guard, so F5 can convert retained execution marks into queue intent and let workspace evidence determine the next action.

## State That Remains Unchanged

- `ExecutionMarkStore` and TUI execution marks;
- workspace files, worktrees, Git state, and task progress;
- terminal `Error`, `Merged`, `Pushed`, and `Rejected` outcomes;
- process-level TUI stopped/resume mode;
- cancellation classification and exactly-once terminal log behavior.

The reducer transition MUST NOT assign `TerminalState::Stopped`. That status describes a change outcome and would make marked rows ineligible for the existing start-from-stopped path, which accepts `not queued` rows.

## Event and Projection Order

The production order remains reducer-first:

1. dispatch owner applies `Stopped` to `OrchestratorState`;
2. Web/API receives the authoritative post-event state;
3. TUI reads the reducer display/blocker/error snapshot for `Stopped`;
4. TUI local handler updates only process mode, timing, controls, and log presentation;
5. later refreshes reconcile workspace observations against the dequeue guard and cannot restore stale activity.

`should_apply_event_to_tui_reducer` must therefore classify `Stopped` as display-affecting. Keeping a second string-matching reset in `handle_stopped` would preserve the original architectural defect: newly added status variants could again be omitted from one copy.

## Verification Strategy

Use deterministic repository-local tests below the one-second default-test limit:

- reducer table test for all activity and wait variants, queued intent, terminal/fresh exclusions, duplicate stop, stale events, and explicit requeue;
- runner integration test for the exact observed `AcceptanceStarted` → `Stopped` → `ChangesRefreshed` sequence with a retained mark;
- Web/API dispatch test for coherent display/intent/mark projection and duplicate-stop revision idempotency.

At least the accepting runner test and API status assertion must fail against the current implementation, where `Stopped` is a reducer no-op.

## Risks and Mitigations

- **Unrelated idle rows become artificially dequeued:** target only rows carrying transient run ownership.
- **Manual merge or blocker evidence is lost:** only process-local wait metadata is cleared; workspace/Git evidence remains and explicit resume re-derives the next action under the constitution.
- **Late events resurrect work:** retain the existing process-local dequeue guard until explicit requeue.
- **Terminal errors disappear:** exclude every existing terminal outcome from global-stop reconciliation.
- **Frontend divergence:** project one authoritative dispatch state and remove TUI-local row lifecycle ownership.
