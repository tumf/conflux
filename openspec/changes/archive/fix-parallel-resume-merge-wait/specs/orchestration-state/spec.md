## ADDED Requirements

### Requirement: Parallel Resume Applies Archive-Complete Wait Semantics

In Parallel execution mode, when a resumed workspace is already archive-complete, the shared lifecycle state SHALL apply the same wait semantics as a `ChangeArchived` transition.

This resume-time archive-complete transition MUST preserve the user-visible merge-wait lifecycle and MUST NOT fall back to `not queued` before merge handling has been attempted.

#### Scenario: Resume-time archived change becomes merge wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a reused workspace is detected as already archived but not yet merged
- **WHEN** the parallel resume path reports archive-complete completion for that change
- **THEN** the wait state becomes `MergeWait`
- **AND** the derived display status is merge wait
- **AND** the change does not regress to `not queued` during the restart flow
