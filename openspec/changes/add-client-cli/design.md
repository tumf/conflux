# Design: stable existing-owner client CLI

## Boundaries

`cflx client` is a client. It must never acquire the repository orchestration lock, bind an API listener, initialize logging, load orchestration configuration, launch lifecycle adapters, or start AI subprocesses. It may inspect Git only to derive the canonical common directory and later confirm repository-visible completion.

The existing owner remains authoritative for process-local marks, queue intent, action eligibility, command admission, retries, scheduling, and transitions. Workspace and Git evidence remain authoritative for workflow routing and completion under the constitution.

## Command surface

```text
cflx client [--unix-socket PATH] [--auth-token-env NAME] status [--json]
cflx client [--unix-socket PATH] [--auth-token-env NAME] enqueue <change-id> [--json]
cflx client [--unix-socket PATH] [--auth-token-env NAME] wait <change-id> [--timeout DURATION] [--json]
```

Connection options belong to the `client` namespace. `--auth-token-env` names an environment variable; no literal token flag is supported. `--json` may be shared or per-action according to the existing Clap conventions, but the resulting help must be unambiguous.

## Transport and discovery

The default socket is derived with the same canonical Git common-directory helper as the owner. An override supports tests and explicitly configured owners. The client performs health, capabilities, instance, state, execution-status, and owner execution-contract reads before deciding an action. Because these are separate resources, one coherent observation requires the same `instance_id` and matching `state_revision`; event cursors must not move backwards. A mismatch triggers a bounded full reread, then `observation_conflict` rather than a mixed snapshot.

The client should reuse the source v2 DTOs. Transport code should add only the minimum HTTP-over-UDS machinery needed by this repository; do not introduce a general HTTP client dependency if the existing Tokio/Axum/HTTP stack or a small local codec suffices.

## Stable JSON envelope

Every JSON invocation emits exactly one object:

```json
{
  "schema_version": 1,
  "ok": true,
  "operation": "enqueue",
  "instance_id": "...",
  "change_id": "alpha",
  "outcome": "admitted",
  "detail": { }
}
```

Failures use `ok: false`, a stable `outcome`, sanitized `message`, and optional current process/revision facts. API wire objects may appear only inside explicitly versioned diagnostic detail; callers must not need them to determine success.

Initial stable unsuccessful outcomes include:

- `not_in_repository`
- `owner_not_running`
- `authentication_failed`
- `incompatible_owner`
- `owner_not_command_capable`
- `owner_restarted`
- `change_not_found`
- `target_ineligible`
- `operator_intent_conflict`
- `revision_conflict`
- `observation_conflict`
- `command_failed`
- `change_rejected`
- `process_failed`
- `timeout`

Exit zero means the requested operation reached its successful outcome. Status succeeds when a compatible owner snapshot was read. Enqueue succeeds only for an accepted/idempotent already-admitted result. Wait succeeds only for verified successful completion.

## Enqueue algorithm

1. Read capabilities, instance, state, and execution status.
2. Locate the change and inspect authoritative action eligibility and lifecycle fields.
3. Refuse final, blocked, unsupported, or unsafe targets before mutation.
4. Choose the minimum high-level route:
   - retry-eligible terminal evidence uses the existing retry command;
   - a live command-capable owner uses queue intent where authoritative semantics permit it;
   - an idle owner marks the target and submits start only when no unrelated ordinary marks would be consumed;
   - already admitted/active work is an idempotent success.
5. For each typed mutation, generate an internal idempotency key and correlation ID, submit at the observed revision, and poll the command record to settlement.
6. On `stale_revision`, re-read the snapshot and recompute the complete intent. Retry only a small fixed number of times. Never reuse an idempotency key with a changed typed identity.
7. Abort if `instance_id` changes. A new process cannot prove whether an old in-flight command settled.

When idle Start requires a mark followed by Start, this is intentionally a two-command sequence using server-owned transactions. Before marking and again before Start, the client refuses `operator_intent_conflict` if unrelated ordinary marks would be consumed. It never clears another operator's marks to manufacture an isolated target set. After the mark settles, the client rereads state before Start. If Start fails or another mark appears in that race window, the requested mark may remain as truthful next-run intent; JSON must report that observable partial intent rather than claim rollback.

## Wait algorithm

`wait` is an observer, not a workflow engine.

1. Capture `instance_id`, the typed owner execution contract, initial change snapshot, and execution status at a reconciled revision.
2. Observe events when available, with polling as recovery for gaps; always rehydrate all observation resources after a gap.
3. Treat command/API presentation as progress evidence, not durable completion authority.
4. For terminal mode `merged`, require terminal `merged` presentation plus an archived proposal and Git ancestry proving the owner-published terminal commit is reachable from the owner-published base branch.
5. For terminal mode `pushed`, require terminal `pushed` presentation plus an archived proposal and typed owner evidence naming the selected remote and remotely confirmed terminal commit; do not substitute local branch ancestry.
6. Return unsuccessful outcomes for rejection, process-fatal failure, incompatible owner replacement, missing/ambiguous completion evidence, or timeout.

The owner execution contract is a minimal source-owned v2 DTO projected with `instance_id` and `state_revision`. It publishes base branch identity, terminal success mode, selected remote when applicable, and the exact terminal commit evidence already owned by the orchestration boundary. It is observability only and cannot drive workflow routing.

## Security and failure handling

- Unix socket paths and change IDs remain ordinary data, never shell fragments.
- Tokens are read from an environment variable and redacted from diagnostics.
- Responses are size-bounded before deserialization.
- Unknown response fields remain forward-compatible where DTO policy permits; unknown required command/action semantics fail closed.
- Owner restart invalidates command-record polling and event cursors.
- JSON mode writes no progress lines to stdout.

## Testing strategy

Use a real local router on a temporary Unix socket with fixture projections and bound/unbound executors. Run the compiled CLI against it. Deterministic synchronization should advance state and command settlement without short correctness timeouts. Timeout flags remain generous safety valves and explicit timeout behavior tests may use paused time or immediate fixture deadlines.

Tests must prove real command calls occurred where expected and zero calls occurred for status, wait, and refused enqueue paths. Stub JSON responses without router/client execution are insufficient for the main integration proof.
