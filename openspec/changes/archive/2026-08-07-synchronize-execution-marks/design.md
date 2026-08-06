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

The authoritative dispatch order becomes one operation under the shared process-local operator mutation guard:

1. capture the target's reducer pre-state needed for edge classification;
2. apply the typed event to `OrchestratorState`;
3. compare post-state and reconcile any mark-revoking edge into the shared `ExecutionMarkStore`;
4. construct the authoritative dispatch snapshot;
5. release the mutation guard and fan the event and state out to TUI, Web, and other sinks.

The reconciler belongs to the process-local operator boundary, not to a frontend. It receives typed event plus explicit pre/post reducer evidence. It mutates only target-scoped `ExecutionMarkStore` entries; it never derives marks from steady status and never changes reducer state, queue intent, workspace evidence, or durable files.

Every production dispatcher that can reach a TUI/Web orchestration boundary must bind the same store. Test/CLI dispatchers with no operator mark store may use an explicit no-mark binding rather than creating a second store.

## Mark-Revoking Edges

The policy is edge-based, not status-based. A steady Error row may be explicitly re-marked; clearing every marked Error during unrelated events would destroy that fresh intent.

| Typed edge | Mark result |
|---|---|
| Processing/apply/acceptance/archive/push/rejection-review failure transitions target to Error | clear target |
| `ChangeRejected` or rejected marker introduced by refresh | clear target |
| refresh changes target from parallel-eligible to parallel-ineligible under the canonical eligibility cleanup rule | clear target and preserve target-scoped queue cleanup ownership |
| successful `ChangeDequeued` / legacy `ChangeStopped` | clear target idempotently |
| first `HookFailed` for `on_merged` that enters merge-wait recovery | clear target |
| dependency block, stalled/external hold, `ChangeSkipped`, ordinary MergeWait/ResolveWait | preserve |
| archive/merge/push success or `AllCompleted` | preserve |
| process-level `Stopped` | preserve all |
| global `Error` without a change target | preserve all |

For reducer-driven failure variants, revocation occurs only when the event establishes the target Error edge. A late failure that cannot supersede an existing final outcome does not clear a mark solely because the variant name contains `Failed`. The same rule applies to `on_merged`: its first failure records a reducer-visible merge-wait recovery edge; replay while that recovery state is already present is a no-op and cannot clear a fresh re-mark.

`ChangesRefreshed` carries the active/rejected catalog and committed/uncommitted evidence needed for rejected and parallel-ineligible target classification. Reconciliation reuses the existing eligibility classifier and target-scoped cleanup rule within that dispatch. It does not rely on a later TUI-only `clear_parallel_ineligible_intent` pass or replace the whole mark set.

## TUI Projection

After handling an orchestrator event, TUI row marks are refreshed from the shared store. Local handlers may update status, diagnostics, timing, and modal state but do not independently decide mark truth. Reducer `queued` status updates queue presentation only; the existing queued-to-`selected = true` amplification is removed.

This direction is one-way for system events: store to row. Operator interactions route target-scoped mark mutations through `OperatorCommandService` under the same mutation guard, then mirror the resulting store. The existing whole-row `publish_execution_marks()`/`replace()` path is not used for operator mark mutation or event cleanup, so an interaction starting from stale rows cannot overwrite a remote mark or resurrect an event-revoked mark.

## Web and Revision Semantics

Web reads marks while building the candidate snapshot for the same authoritative dispatch. Because reconciliation runs before the Web sink, a failure/rejection event envelope and its snapshot revision contain `execution_marked: false` together.

Reconciliation is idempotent. Repeating an event after the target mark is already false yields no additional mark change; normal projection dedup/no-change logic prevents revision churn.

## Retry and Stop Boundaries

A change-level Error clears stale pre-failure intent. An operator can then express fresh intent through the existing supported mode/status matrix. Running Error re-mark follows queue/retry behavior; Stopped Error re-mark remains mark-only until resume; global Error mode remains retry-command owned. Because the mark mutation and event reconciliation use the same guard and revocation requires a new pre/post edge, duplicate delivery after re-mark preserves that fresh intent.

Process-level `Stopped` preserves marks exactly as the existing stopped/resume contract requires. This proposal does not change the separate reducer reconciliation performed by force stop.

## Verification Strategy

- A table-driven unit test covers every revoking edge, including parallel-ineligible refresh, first `on_merged` recovery, duplicate-after-re-mark delivery, and late-event exclusion.
- Preservation tests cover dependency/stalled/wait/success/global stop/global error cases and unrelated IDs.
- TUI tests begin with an intentionally divergent row/store pair, dispatch an event, and prove the row follows the store; queued reducer status never creates a mark.
- A deterministic ordering test pauses an operator mark mutation against event reconciliation and proves stale TUI rows cannot resurrect the revoked target.
- Web tests use the real authoritative dispatch boundary and assert mark/event revision coherence.
- Cross-adapter tests clear on Error, explicitly re-mark, replay the same failure, and prove existing retry/start routing still consumes the fresh mark.

## Risks and Mitigations

- **Over-clearing explicitly re-marked Error or hook-recovery rows:** compare pre/post reducer evidence under the mutation guard; never clear from steady status or event name alone.
- **A late failure clears a successful final row:** consult pre/post reducer evidence and test terminal supersession guards.
- **Refresh leaves invalid marks or clears unrelated remote marks:** reuse the canonical target classifier and mutate only ineligible target IDs.
- **Queued presentation invents a mark:** remove queued-to-selected amplification and test queue/mark separation.
- **TUI overwrites a Web command or resurrects an event-revoked mark:** operator writes and event reconciliation serialize through one guard and never replace the store from whole-row state.
- **Stop loses resume targets:** process-level Stopped is an explicit preservation case.
- **One dispatch path skips reconciliation:** bind the policy at the common dispatch owner and enumerate production construction sites in integration tests.
