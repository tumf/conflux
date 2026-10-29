## ADDED Requirements

### Requirement: Inactive blocked-only work is a safe graceful-stop boundary

The scheduler MUST settle an accepted graceful stop when a coherent reducer view and scheduler runtime ownership prove that only inactive blocked work remains and no active task, lane waiter, resolver, merge, push or cleanup boundary is owed. It MUST reuse the existing stopped outcome and cleanup protocol, preserve marks and real blocker/completion evidence under existing stop semantics, and MUST NOT execute blocked work or report completion success.

#### Scenario: Dependency-only park stops without waiting for a nonexistent phase

- **GIVEN** a persistent scheduler is parked with a queued valid proposal blocked by an unmet dependency and no active runtime or cleanup work
- **WHEN** graceful stop is accepted and wakes that scheduler
- **THEN** the scheduler settles exactly one Stopped outcome without another external wake
- **AND** Core, API and TUI leave Stopping and observe an inactive scheduler
- **AND** no analyzer, Apply, gate or reviewer starts and the target is not completed

#### Scenario: Runtime or incomplete evidence forbids early settlement

- **GIVEN** a stop is pending but a coherent snapshot is unavailable or real task, lane waiter, merge, resolver or cleanup work remains
- **WHEN** the scheduler evaluates the blocked-only stop boundary
- **THEN** it does not claim Stopped from absent evidence or an idle display flag
- **AND** the existing graceful completion and bounded cleanup protocol remains authoritative

#### Scenario: Stop preserves repair and resume semantics

- **GIVEN** inactive held or errored work was stopped with real evidence and marks preserved under the normal stop contract
- **WHEN** files or marks change without an explicit accepted Start or Retry
- **THEN** stopped work is not automatically restarted
- **AND** later explicit Start or Retry follows existing blocker, completion and phase-aware admission rules
