## MODIFIED Requirements

### Requirement: Applied resume uses workspace-local evidence only

Parallel execution MUST determine resume routing from workspace-local file state, Git state, and base-branch tree evidence only.

For implementation changes, if implementation tasks are incomplete, resume routing MUST return to apply.

Otherwise, a complete implementation that is not repository-verifiably archived or base-integrated MUST run acceptance before archive. Conflux MUST NOT create or consult a generated acceptance checkpoint to infer prior PASS after restart.

An archived or base-integrated workspace MAY continue to post-archive resolve, merge, or terminal handling without rerunning acceptance.

Out-of-worktree durable state (for example under `~/.local/state/cflx/**`) MUST NOT be used as authoritative input for this decision.

#### Scenario: applied workspace resumes acceptance regardless of external durable state

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are complete
- **AND** external durable acceptance/archive state files exist or do not exist
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Acceptance`
- **AND** the result is identical regardless of external state presence
- **AND** `.cflx/acceptance-state.json` is not created or consulted

#### Scenario: applied workspace with incomplete implementation tasks resumes apply

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are incomplete
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Apply`
- **AND** acceptance/archive are not entered in that cycle

#### Scenario: interrupted incomplete archive reruns acceptance

- **GIVEN** archive work began but repository evidence does not prove a complete valid archive
- **AND** no resumable blocker marker prevents dispatch
- **WHEN** execution resumes after process restart
- **THEN** acceptance runs before archive finalization
- **AND** a prior PASS is not inferred from missing generated state

#### Scenario: repository-verifiably archived workspace continues post-archive handling

- **GIVEN** the active change directory is absent
- **AND** a valid archive entry exists
- **WHEN** resume routing is evaluated
- **THEN** the change continues to resolve, merge, or terminal handling as appropriate
- **AND** acceptance checkpoint state is not required

### Requirement: Acceptance stalled retry evidence is workspace-local

Acceptance retry control during an active serial or parallel run MUST use in-memory previous finding identities, semantic baseline, and cycle count. Runtime MUST NOT persist this ordinary retry context in `.cflx/acceptance-state.json` or another replacement hidden checkpoint.

An acceptance-generated stalled hold MUST use the existing tracked apply-blocked marker contract. Ordinary dispatch MUST honor the marker after restart. Explicit retry MAY consume a resumable acceptance-generated marker, but MUST NOT clear unrelated, unknown-origin, or non-resumable blockers.

If restart occurs before a stalled marker exists, Conflux MAY begin a fresh acceptance retry sequence, but MUST run acceptance again and MUST NOT treat the unarchived revision as accepted.

#### Scenario: restart before stalled marker reruns acceptance with fresh in-memory context

- **GIVEN** acceptance previously failed but no stalled blocker marker was persisted
- **AND** the orchestration process restarts
- **WHEN** serial or parallel execution resumes the complete unarchived workspace
- **THEN** acceptance runs again
- **AND** prior in-memory retry count and semantic baseline are not reconstructed from a generated checkpoint
- **AND** archive does not start from an inferred prior PASS

#### Scenario: restart reconstructs acceptance stalled state

- **GIVEN** a workspace contains an acceptance-generated resumable blocker marker
- **AND** out-of-worktree Conflux state is absent
- **WHEN** Conflux detects the workspace after restart
- **THEN** it reconstructs the stalled hold and next action from the marker
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

## ADDED Requirements

### Requirement: Acceptance execution creates no JSON checkpoint

Serial and parallel acceptance execution MUST NOT create, read, update, or delete `.cflx/acceptance-state.json`. Acceptance PASS for an active run MAY be held in memory only until archive handoff. After restart, incomplete archive work MUST be accepted again unless repository evidence already proves archive or base integration.

#### Scenario: uninterrupted pass reaches archive without checkpoint

- **GIVEN** apply completed and acceptance runs in the same orchestration process
- **WHEN** acceptance returns PASS
- **THEN** archive handoff proceeds for that accepted revision
- **AND** `.cflx/acceptance-state.json` never exists

#### Scenario: checkpoint cleanup cannot dirty post-archive worktree

- **GIVEN** acceptance passes and archive artifacts are committed
- **WHEN** post-archive merge verification runs
- **THEN** no acceptance checkpoint cleanup is performed
- **AND** no manual `MergeWait` is produced solely by generated acceptance state

#### Scenario: genuine dirty evidence remains a blocker

- **GIVEN** archive artifacts are valid
- **AND** an unrelated user file remains modified
- **WHEN** post-archive merge verification runs
- **THEN** the unrelated dirty worktree remains concrete manual blocker evidence
- **AND** removing acceptance checkpoints does not suppress the deferral
