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

Process facts distinguish scheduler liveness from active work. `scheduler_running` means the run boundary is alive. `has_active_work` is true only when typed lifecycle facts identify at least one currently active change phase or active process-level resolve/merge work.

Change `execution_state` uses a closed enum:

- `queued`;
- `active`;
- `waiting`;
- `stopping`;
- `stopped`;
- `failed`;
- `completed`;
- `unknown`.

Lifecycle phase uses a closed enum:

- `analysis`;
- `apply`;
- `acceptance`;
- `archive`;
- `resolve`;
- `merge`;
- `push`;
- `hook`;
- `none`;
- `unknown`.

These values come from typed lifecycle transitions and process-local phase facts. `display_status`, task completion percentage, log text, and commit subject text are not phase classifiers.

## Latest Log Selection

The process latest log is the newest entry in the retained ring. A change latest log is the newest entry whose structured `change_id` exactly equals the target ID. Selection does not fall back to substring matching, operation guessing, workspace paths, or reading the persistent file.

The existing `LogEntry` sanitization and 8,192-byte message bound remain authoritative. The resource returns the retained structured message, level, operation, iteration, and absolute creation time. No filesystem locator is added.

## Phase-aware Stop Settlement

`stop_and_dequeue` remains a two-phase transaction:

1. validate target and issue cancellation under the application boundary;
2. await confirmed termination outside the boundary;
3. reacquire the boundary and revalidate current lifecycle state;
4. inspect settlement evidence;
5. commit dequeue, dispatch its exact revision, and store one immutable command result.

Evidence is captured after termination confirmation because the worker may cross a phase boundary between admission and cancellation settlement. The result reports the work that was actually stopped, not merely the phase seen at the original expected revision.

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

Apply commit detection must use the repository's managed-worktree and lifecycle evidence. It must not accept a commit solely because its subject starts with `Apply:`; identity and expected lifecycle relation must be verified using existing repository-local mechanisms or a narrowly added evidence port.

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
