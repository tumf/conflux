## MODIFIED Requirements

### Requirement: Event-driven execution mark reconciliation

`ExecutionMarkStore` MUST remain the process-local authoritative execution-mark set across TUI, Web, and shared run-control target resolution. When a typed execution event creates a mark-revoking transition, the system MUST compare reducer evidence immediately before and after applying the event, update only the affected shared mark, and complete that reconciliation before frontend fan-out. Reducer pre-state capture, event application, post-state classification, and mark reconciliation MUST be ordered under the same process-local mutation boundary used by operator mark actions.

Mark-revoking transitions MUST include change-level transition into Error, terminal Rejected, rejected marker rows discovered by refresh, refresh classification that makes a marked target parallel-ineligible, successful per-change dequeue/legacy stop, and the first `on_merged` hook-failure transition into merge-wait recovery. Reconciliation MUST be target-scoped and idempotent.

For system-driven events, TUI row selection MUST mirror the reconciled store and MUST NOT remain an independent mark authority. Reducer `queued` status MUST remain queue presentation and MUST NOT synthesize an execution mark. Operator mark actions MUST use target-scoped shared-service mutation and then mirror the store; they MUST NOT replace the whole store from a cached TUI row set.

The system MUST preserve marks for unrelated changes, blocked/stalled/dependency-wait changes, ordinary MergeWait/ResolveWait, successful archive/merge/push/completion, global fatal Error without a target, and process-level Stopped. A steady Error or `on_merged` recovery row that was explicitly re-marked MUST NOT be cleared by an unrelated or duplicate later event.

#### Scenario: successful archive preserves target and unrelated marks

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **WHEN** `ChangeArchived` reports successful archive for `alpha`
- **THEN** the shared execution marks for both `alpha` and `beta` remain true
- **AND** TUI and `/api/v2` project the same marked set
- **AND** no queue intent, retry intent, or scheduler action is synthesized by mark preservation

#### Scenario: genuine invalidation remains target-scoped

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **WHEN** a change-level Error, terminal Rejected, rejected or parallel-ineligible refresh, explicit dequeue, or first merge-hook recovery transition invalidates `alpha`
- **THEN** only `alpha` loses its shared execution mark
- **AND** `beta` remains marked

<!-- Expected canonical result after archive: successful archive remains explicitly mark-preserving, while existing invalidation transitions continue to revoke only their target. -->
