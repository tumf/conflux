## ADDED Requirements

### Requirement: Event-driven execution mark reconciliation

`ExecutionMarkStore` MUST remain the process-local authoritative execution-mark set across TUI, Web, and shared run-control target resolution. When a typed execution event creates a mark-revoking transition, the system MUST update the target's shared mark after applying reducer state and before frontend fan-out. TUI row selection MUST mirror the reconciled store and MUST NOT remain an independent mark authority.

Mark-revoking transitions MUST include change-level transition into Error, terminal Rejected, rejected rows discovered by refresh, successful per-change dequeue/legacy stop, and the existing `on_merged` hook-failure path that revokes TUI selection. Reconciliation MUST be target-scoped and idempotent.

The system MUST preserve marks for unrelated changes, blocked/stalled/dependency-wait changes, ordinary MergeWait/ResolveWait, successful archive/merge/push/completion, global fatal Error without a target, and process-level Stopped. A steady Error row that was explicitly re-marked MUST NOT be cleared by an unrelated later event.

#### Scenario: change-level Error clears stale intent before projection

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **WHEN** a processing, apply, acceptance, archive, push, or rejection-review failure transitions `alpha` into reducer Error
- **THEN** the shared execution mark for `alpha` is false before frontend fan-out
- **AND** `beta` remains marked
- **AND** TUI, Web, and Start target resolution observe the same result

#### Scenario: rejection and rejected refresh clear the target mark

- **GIVEN** a change is execution-marked
- **WHEN** `ChangeRejected` makes it terminal Rejected or `ChangesRefreshed` introduces it as a rejected marker row
- **THEN** only that change's shared mark is cleared
- **AND** the TUI rejected row and API snapshot both report it unmarked

#### Scenario: explicit dequeue clears the shared target

- **GIVEN** a marked change completes stop-and-dequeue successfully
- **WHEN** `ChangeDequeued` or the legacy target-scoped stop event is projected
- **THEN** the shared mark and TUI row mark are false
- **AND** duplicate event delivery is a no-op

#### Scenario: wait and stop boundaries preserve marks

- **GIVEN** one or more changes are execution-marked
- **WHEN** they become dependency blocked, stalled, externally blocked, MergeWait, ResolveWait, archived, merged, pushed, or the process enters Stopped
- **THEN** their shared marks are preserved
- **AND** process-level Stopped can resume the same marked target set

#### Scenario: explicit Error re-mark creates fresh intent

- **GIVEN** a change-level Error transition cleared the change's prior mark
- **WHEN** an operator re-marks it through a supported Running or Stopped route
- **THEN** `ExecutionMarkStore` records the new explicit intent
- **AND** the existing retry/start flow can consume it
- **AND** an unrelated event does not clear it merely because the row remains Error

#### Scenario: restart still clears all marks

- **GIVEN** event reconciliation produced any process-local mark set
- **WHEN** the process restarts
- **THEN** every execution mark starts false
- **AND** workflow routing remains derived from workspace and Git evidence
