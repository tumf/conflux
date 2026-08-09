# Design: Agent-readable execution observability

## Decision

Expose one derived read model for remote agents and one typed settlement result for lifecycle intervention. Keep all evidence process-local or repository-derived, non-authoritative, bounded, sanitized, and path-free.

## Execution Status Resource

`GET /api/v2/execution-status` is a read-only authenticated resource served by the existing v2 router. It does not replace `/state` or `/logs`:

- `/state` remains the coherent authoritative operator decision snapshot at `state_revision`;
- `/logs` remains the complete retained structured log ring;
- `/execution-status` provides a compact, machine-readable join for determining whether work is active and what was most recently observed.

The projection owner reads the snapshot and log ring under one lock. The response includes both `state_revision` and `event_sequence`. A log-only event may therefore change the response and advance `event_sequence` while leaving `state_revision` unchanged.

## Time Contract

All wire times are UTC RFC 3339 absolute instants:

- `observed_at`: response observation instant;
- phase `started_at` and `completed_at` boundaries when known;
- latest activity timestamp;
- latest log `created_at`.

The API does not return elapsed seconds, age seconds, or “N minutes ago”. A client that needs relative display computes it against `observed_at`, not its own wall clock. Missing timestamps are `null`; the server does not synthesize them from revision order.

## Closed Execution Vocabulary

Process facts distinguish scheduler liveness from active work. `scheduler_running` reads the same `RunBoundaryLiveness` authority used by snapshot eligibility and command admission. `has_active_work` is true while either a per-change phase is active or a closed process-level activity has started and has not reached its typed terminal event. The process-level set is dependency analysis, base-branch merge, conflict resolution, branch merge, and workspace cleanup. Persistent-idle scheduler liveness alone is not active work.

Change `execution_state` uses a closed enum:

- `queued`;
- `active`;
- `waiting`;
- `stopping`;
- `stopped`;
- `failed`;
- `completed`;
- `unknown`.

Per-change `current_phase` uses the existing reducer `ActivityState` as its sole authority and projects a closed enum:

- `preparing` from `ActivityState::Preparing`;
- `apply` from `ActivityState::Applying`;
- `acceptance` from `ActivityState::Accepting`;
- `rejection_review` from `ActivityState::Rejecting`;
- `archive` from `ActivityState::Archiving`;
- `resolve` from `ActivityState::Resolving`;
- `push` when typed push activity is open;
- `none` when no phase is active;
- `unknown` only when typed evidence cannot be classified.

Analysis is process-level only. `merge` may appear in `last_completed_phase` only from a typed per-change merge-completion fact. `hook` is not advertised until production emits typed hook start/completion facts. Phase facts are updated synchronously from the same typed event under the authoritative dispatch boundary and never form a second lifecycle authority. `display_status`, task completion percentage, log text, and commit subject text are not phase classifiers.

## Latest Log Selection

The process latest log is the last entry by retained-ring insertion order. A change latest log is the last inserted entry whose structured `change_id` exactly equals the target ID. Selection does not compare second-precision timestamps and does not fall back to substring matching, operation guessing, workspace paths, or reading the persistent file.

The resource does not embed the existing `LogEntry` wire shape, whose `created_at` is epoch seconds and whose schema includes `workspace_path`. It returns a closed projection containing only the already-sanitized and 8,192-byte-bounded message, level, operation, iteration, and an RFC 3339 UTC `created_at`. Display timestamp and workspace path are omitted. No filesystem locator is added.

## Phase-aware Stop Settlement

`stop_and_dequeue` remains a two-phase transaction:

1. validate target and issue cancellation under the application boundary;
2. await confirmed termination outside the boundary;
3. while the terminated worktree is quiescent and before reacquiring the boundary, read explanatory managed-worktree Git evidence so Git subprocess latency cannot monopolize operator admission;
4. reacquire the boundary, revalidate current lifecycle state, and read typed phase facts before applying `ReducerCommand::DequeueChange` clears them;
5. commit dequeue, dispatch its exact revision, and store one immutable command result.

Phase facts are updated under the authoritative dispatch boundary before termination confirmation can settle, so every typed fact dispatched by the worker before exit is visible in step 4. `cancelled_phase` means the active typed per-change phase observed at settlement immediately before dequeue; an already-terminated target or target with no active phase reports `none`. The result does not report merely the phase seen at the original expected revision.

The result shape is a tagged closed variant, conceptually:

```json
{
  "kind": "stop_and_dequeue",
  "cancelled_phase": "acceptance",
  "last_completed_phase": "apply",
  "apply_commit": {
    "present": true,
    "oid": "<full commit oid>"
  },
  "effects_rolled_back": false
}
```

`present` is nullable so unreadable or ambiguous evidence is not collapsed into `false`. `oid` is present only when the final Apply commit is proven. OID disclosure is allowed repository identity evidence; no branch, worktree path, repository root, or log path accompanies it.

Apply commit detection retains the non-empty OID from typed `ApplyCompleted.revision` as a per-change, per-process-incarnation fact. The evidence port identifies the managed worktree through the server-owned change-to-worktree mapping, reads its HEAD while quiescent, and proves the retained OID is equal to or an ancestor of HEAD. Only then is `present: true`, and `oid` is the retained completion OID. Missing/empty completion OID, restart-empty facts, missing worktree, Git failure, or a non-ancestor result is unknown rather than a subject-based guess. Commit subject is never an evidence input.

## Result Stability and Replay

`CommandRecord.result` is optional for backward-compatible commands and uses closed tagged variants. The registry writes it once when the command settles. Exact idempotent replay returns the stored result unchanged. Later commits, phase changes, or projection refreshes cannot rewrite it.

The detail string is presentation only. Machine consumers use `result`. The detail explicitly says that dequeue does not roll back previously completed worktree effects.

## Privacy and Security

The same contract applies to UDS and TCP:

- no persistent log path;
- no workspace or repository path as a log locator;
- no `file://` URL;
- no path parameter, arbitrary offset, glob, or filename accepted by log APIs;
- existing bearer and origin policy protects the new route;
- `/health` remains the only unauthenticated v2 route.

Agents access logs through `/logs`, `/events`, `/ws`, and the compact latest-log fields. The API never turns host filesystem access into an observability feature.

## Failure Semantics

If phase evidence is unavailable, phase fields are `unknown`. If Git evidence cannot be observed safely, Apply commit presence and OID are null. The stop command may still truthfully succeed at cancellation and dequeue; uncertainty in explanatory evidence is not silently converted into certainty and does not create a second workflow gate.

A stop failure retains the existing no-dequeue guarantees. A failed command may return typed failure detail, but it must not claim successful phase settlement or commit evidence as a completed dequeue result.

## Deterministic Verification

The race regression uses barriers or channels, not short wall-clock assertions:

1. the fake Apply worker pauses immediately before final commit;
2. the test admits stop-and-dequeue;
3. the worker creates and publishes the final Apply commit and enters acceptance;
4. cancellation is observed and termination is confirmed;
5. settlement captures evidence and dequeues;
6. assertions verify acceptance cancellation, completed Apply, exact OID, no rollback, and retained Git commit.

Additional table cases stop before Apply commit, during archive, during resolve, and with unavailable evidence. Projection tests inject fixed timestamps and structured logs to prove absolute-time serialization, exact change association, and log-only revision behavior.
