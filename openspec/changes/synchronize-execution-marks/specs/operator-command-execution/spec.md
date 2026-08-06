## ADDED Requirements

### Requirement: Event-driven execution mark reconciliation

`ExecutionMarkStore` MUST remain the process-local authoritative execution-mark set across TUI, Web, and shared run-control target resolution. When a typed execution event creates a mark-revoking transition, the system MUST compare reducer evidence immediately before and after applying the event, update only the affected shared mark, and complete that reconciliation before frontend fan-out. Reducer pre-state capture, event application, post-state classification, and mark reconciliation MUST be ordered under the same process-local mutation boundary used by operator mark actions.

Mark-revoking transitions MUST include change-level transition into Error, terminal Rejected, rejected marker rows discovered by refresh, refresh classification that makes a marked target parallel-ineligible, successful per-change dequeue/legacy stop, and the first `on_merged` hook-failure transition into merge-wait recovery. Reconciliation MUST be target-scoped and idempotent.

For system-driven events, TUI row selection MUST mirror the reconciled store and MUST NOT remain an independent mark authority. Reducer `queued` status MUST remain queue presentation and MUST NOT synthesize an execution mark. Operator mark actions MUST use target-scoped shared-service mutation and then mirror the store; they MUST NOT replace the whole store from a cached TUI row set.

The system MUST preserve marks for unrelated changes, blocked/stalled/dependency-wait changes, ordinary MergeWait/ResolveWait, successful archive/merge/push/completion, global fatal Error without a target, and process-level Stopped. A steady Error or `on_merged` recovery row that was explicitly re-marked MUST NOT be cleared by an unrelated or duplicate later event.

#### Scenario: change-level Error clears stale intent before projection

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **AND** `alpha` is not yet in reducer Error
- **WHEN** a processing, apply, acceptance, archive, push, or rejection-review failure transitions `alpha` into reducer Error
- **THEN** the shared execution mark for `alpha` is false before frontend fan-out
- **AND** `beta` remains marked
- **AND** TUI, Web, and Start target resolution observe the same result

#### Scenario: rejection and rejected refresh clear the target mark

- **GIVEN** a change is execution-marked
- **WHEN** `ChangeRejected` makes it terminal Rejected or `ChangesRefreshed` introduces it as a rejected marker row
- **THEN** only that change's shared mark is cleared
- **AND** the TUI rejected row and API snapshot both report it unmarked

#### Scenario: eligibility refresh clears invalid target intent

- **GIVEN** `alpha` and `beta` are execution-marked
- **AND** one refresh classifies `alpha` as parallel-ineligible while `beta` remains eligible
- **WHEN** that authoritative refresh dispatch is reconciled
- **THEN** only `alpha` loses its shared mark
- **AND** existing target-scoped queue cleanup remains coherent
- **AND** no whole-store row publication can remove `beta`

#### Scenario: explicit dequeue clears the shared target

- **GIVEN** a marked change completes stop-and-dequeue successfully
- **WHEN** `ChangeDequeued` or the legacy target-scoped stop event is projected
- **THEN** the shared mark and TUI row mark are false
- **AND** duplicate event delivery is a no-op

#### Scenario: queued presentation does not create a mark

- **GIVEN** a change has reducer queue intent but no execution mark
- **WHEN** a system event synchronizes its TUI row as queued
- **THEN** the row remains unmarked
- **AND** TUI and `/api/v2` continue to distinguish queue intent from execution marks

#### Scenario: duplicate failure after re-mark preserves fresh intent

- **GIVEN** a change-level Error or first `on_merged` recovery edge cleared the old mark
- **AND** an operator explicitly re-marked the steady recovery row through a supported route
- **WHEN** the same failure event is delivered again without creating a new reducer transition
- **THEN** the fresh shared mark remains true
- **AND** existing retry/start flow can consume it

#### Scenario: stale TUI rows cannot resurrect a revoked mark

- **GIVEN** TUI cached rows still show a target marked
- **AND** a system event revokes that target in `ExecutionMarkStore`
- **WHEN** a concurrent TUI operator action settles after the event
- **THEN** the action mutates only its requested target through the shared service
- **AND** it cannot replace the whole store or restore the revoked mark from stale row state

#### Scenario: wait and stop boundaries preserve marks

- **GIVEN** one or more changes are execution-marked
- **WHEN** they become dependency blocked, stalled, externally blocked, MergeWait, ResolveWait, archived, merged, pushed, or the process enters Stopped
- **THEN** their shared marks are preserved
- **AND** process-level Stopped can resume the same marked target set
