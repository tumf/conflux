## Context

`ExecutionMarkStore` is already shared by the operator command service, TUI state, and Web state. Start target resolution reads it directly, and `/api/v2` snapshots derive `execution_marked` from it. `ChangeState::selected` is therefore a presentation cache, not a second authority.

The event path violates that boundary. TUI failure/rejection/stop handlers write `selected = false` without updating the store. Conversely, publishing every row back into the store after an event would let a frontend cache overwrite remote operator intent.

## Goals

- Apply system-driven mark revocation once at a shared event boundary.
- Make TUI and `/api/v2` observe the same mark in the event's revision.
- Preserve all existing retry, blocker, stop/resume, and process-local semantics.
- Remove local-only deselection ownership.

## Non-Goals

- Persisting marks or deriving workflow routing from them.
- Clearing marks broadly from steady-state status.
- Changing mark/queue command admission.
- Redesigning retry or force-stop state.

## Decision: Reconcile Marks Before Frontend Fan-Out

The authoritative dispatch order becomes:

1. apply the typed event to `OrchestratorState`;
2. reconcile any mark-revoking edge into the shared `ExecutionMarkStore`;
3. construct the authoritative dispatch snapshot;
4. fan the event and state out to TUI, Web, and other sinks.

The reconciler belongs to the process-local operator boundary, not to a frontend. It receives typed event and post-transition reducer evidence. It mutates only `ExecutionMarkStore`; it never changes reducer state, queue intent, workspace evidence, or durable files.

Every production dispatcher that can reach a TUI/Web orchestration boundary must bind the same store. Test/CLI dispatchers with no operator mark store may use an explicit no-mark binding rather than creating a second store.

## Mark-Revoking Edges

The policy is edge-based, not status-based. A steady Error row may be explicitly re-marked; clearing every marked Error during unrelated events would destroy that fresh intent.

| Typed edge | Mark result |
|---|---|
| Processing/apply/acceptance/archive/push/rejection-review failure transitions target to Error | clear target |
| `ChangeRejected` or rejected marker introduced by refresh | clear target |
| successful `ChangeDequeued` / legacy `ChangeStopped` | clear target idempotently |
| `HookFailed` for `on_merged` where existing TUI policy deselects the target | clear target |
| dependency block, stalled/external hold, `ChangeSkipped`, ordinary MergeWait/ResolveWait | preserve |
| archive/merge/push success or `AllCompleted` | preserve |
| process-level `Stopped` | preserve all |
| global `Error` without a change target | preserve all |

For reducer-driven failure variants, revocation occurs only when the event establishes the target Error edge. A late failure that cannot supersede an existing final outcome does not clear a mark solely because the variant name contains `Failed`.

## TUI Projection

After handling an orchestrator event, TUI row marks are refreshed from the shared store. Local handlers may update status, diagnostics, timing, and modal state but do not independently decide mark truth.

This direction is one-way for system events: store to row. Operator interactions continue through the shared service or the existing publish path, so a remote mark cannot be overwritten by a stale TUI row during an unrelated event.

## Web and Revision Semantics

Web reads marks while building the candidate snapshot for the same authoritative dispatch. Because reconciliation runs before the Web sink, a failure/rejection event envelope and its snapshot revision contain `execution_marked: false` together.

Reconciliation is idempotent. Repeating an event after the target mark is already false yields no additional mark change; normal projection dedup/no-change logic prevents revision churn.

## Retry and Stop Boundaries

A change-level Error clears stale pre-failure intent. An operator can then express fresh intent through the existing supported mode/status matrix. Running Error re-mark follows queue/retry behavior; Stopped Error re-mark remains mark-only until resume; global Error mode remains retry-command owned.

Process-level `Stopped` preserves marks exactly as the existing stopped/resume contract requires. This proposal does not change the separate reducer reconciliation performed by force stop.

## Verification Strategy

- A table-driven unit test covers every revoking event and late-event exclusion.
- Preservation tests cover dependency/stalled/wait/success/global stop/global error cases and unrelated IDs.
- TUI tests begin with an intentionally divergent row/store pair, dispatch an event, and prove the row follows the store rather than replacing it.
- Web tests use the real authoritative dispatch boundary and assert mark/event revision coherence.
- Cross-adapter tests clear on Error, explicitly re-mark, and prove existing retry/start routing consumes the fresh mark.

## Risks and Mitigations

- **Over-clearing explicitly re-marked Error rows:** reconcile transition edges only, never steady status on every event.
- **A late failure clears a successful final row:** consult post-transition reducer evidence and test terminal supersession guards.
- **TUI overwrites a Web command:** event synchronization reads from the store; it does not replace the store from rows.
- **Stop loses resume targets:** process-level Stopped is an explicit preservation case.
- **One dispatch path skips reconciliation:** bind the policy at the common dispatch owner and enumerate production construction sites in integration tests.
