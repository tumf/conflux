## MODIFIED Requirements

### Requirement: Inactive blocked-only work is a safe graceful-stop boundary

The scheduler MUST settle an accepted graceful stop when a coherent reducer view and scheduler runtime ownership prove that only inactive blocked work remains and no active task, lane waiter, resolver, merge, push or cleanup boundary is owed. It MUST reuse the existing stopped outcome and cleanup protocol, preserve marks and real blocker/completion evidence under existing stop semantics, and MUST NOT execute blocked work or report completion success.

After an awaited blocked-only classification qualifies this boundary, the scheduler MUST re-read the existing graceful-stop request immediately before beginning terminal event emission. If cancel-stop completed during classification and the request is still withdrawn at that final read, the scheduler MUST NOT emit Stopped or choose the graceful-stop loop exit from that evaluation. It MUST retain the existing no-request/fully-drained behavior and MUST allow a later accepted Stop to settle normally. This requirement does not redefine cancellation ordering after the final read or while an event send is already pending.

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

#### Scenario: Cancel-stop completes while blocked-only classification is pending

- **GIVEN** the safe-boundary helper observed a pending graceful stop and a complete reducer snapshot
- **AND** the real classifier is awaiting workspace/base evidence for an archived but unmerged dependency
- **WHEN** shared run control accepts cancel-stop, clears the shared request, and completes before that classification is released
- **AND** the classifier returns blocked-only with the request still withdrawn at the final read
- **THEN** this evaluation returns without Stopped, AllCompleted, or a graceful-stop loop exit
- **AND** it executes, retries, dequeues, merges, and cleans up no work

#### Scenario: A pending or later request retains exactly-once settlement

- **GIVEN** the same complete inactive blocked-only evidence
- **WHEN** classification returns and graceful stop remains pending at the final read, either unwithdrawn or from a later accepted Stop after cancellation
- **THEN** the existing settlement path emits exactly one Stopped and the scheduler exits without success finalization
- **AND** no cancellation latch or new workflow state is introduced
