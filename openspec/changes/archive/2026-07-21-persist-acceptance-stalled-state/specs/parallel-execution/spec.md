## ADDED Requirements

### Requirement: Acceptance stalled retry evidence is workspace-local

Acceptance retry control MUST be derivable from workspace-local evidence in serial and parallel modes. Before a stalled hold, runtime MUST persist previous finding identities, semantic baseline, and cycle count in a non-blocking workspace checkpoint. An acceptance-generated stalled hold MUST use the existing apply-blocked marker contract. Ordinary dispatch MUST honor the marker after restart. Explicit retry MAY consume a resumable acceptance-generated marker, but MUST NOT clear unrelated, unknown-origin, or non-resumable blockers.

#### Scenario: restart preserves pre-stall retry context

- **GIVEN** acceptance recorded an initial FAIL and its semantic baseline
- **AND** out-of-worktree Conflux state is absent
- **WHEN** serial or parallel execution resumes after restart
- **THEN** previous finding identities, semantic baseline, and cycle count are reconstructed from the workspace
- **AND** the next FAIL is not incorrectly treated as the first attempt

#### Scenario: restart reconstructs acceptance stalled state

- **GIVEN** a workspace contains an acceptance-generated resumable blocker marker
- **AND** out-of-worktree Conflux state is absent
- **WHEN** Conflux detects the workspace after restart
- **THEN** it reconstructs the stalled hold and next action from workspace evidence
- **AND** serial and parallel ordinary dispatch do not start apply, acceptance, or archive

#### Scenario: explicit retry clears only acceptance-generated marker

- **GIVEN** an operator explicitly retries a stalled acceptance change
- **WHEN** runtime prepares the workspace for retry
- **THEN** it consumes the resumable acceptance-generated marker before dispatch
- **AND** an apply-generated, unknown-origin, or non-resumable marker is not silently cleared

#### Scenario: marker consume failure blocks dispatch

- **GIVEN** a resumable acceptance marker cannot be safely consumed
- **WHEN** explicit retry preparation runs
- **THEN** runtime reports the failure
- **AND** it does not dispatch apply or acceptance with ambiguous workspace evidence
