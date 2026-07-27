## MODIFIED Requirements

### Requirement: Stalled blocker metadata

When a change enters non-terminal `stalled`, reducer-owned state and authoritative blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the blocker from dependency blocking, protocol error, and terminal rejection.

Acceptance-generated stalled evidence MUST live outside the managed worktree in versioned Conflux runtime state and MUST include repository identity, change ID, worktree identity/path, Apply revision, failed phase, explicit supported category, concrete evidence, retry count, resumability, recommended next action, and timestamps. Runtime MUST validate the record against current repository, worktree, change, and revision facts before reconstructing stalled state. Runtime MUST NOT derive category from narrative keywords.

Runtime stalled metadata MAY control dispatch suppression, stalled display, and Acceptance retry preparation only. It MUST NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration, and its mutation or deletion MUST NOT dirty the managed worktree.

#### Scenario: validated Acceptance stalled evidence survives restart

- **GIVEN** a validated Acceptance stall record exists outside the worktree
- **AND** its repository, worktree, active change, and Apply revision binding still match
- **WHEN** Conflux restarts
- **THEN** reducer state reconstructs the same stalled category, evidence, resumability, and next action
- **AND** display status remains execution `stalled`
- **AND** the managed worktree remains clean

#### Scenario: runtime-state deletion fails safe to Acceptance

- **GIVEN** a previously stalled runtime record is deleted
- **AND** repository evidence still shows a complete unarchived Apply revision
- **WHEN** Conflux restarts
- **THEN** it does not reconstruct PASS or archive readiness
- **AND** it routes the change to Acceptance rather than Apply or archive

#### Scenario: stale blocker metadata loses routing authority

- **GIVEN** stored Acceptance blocker metadata no longer matches repository identity, worktree identity, active change state, or Apply revision ancestry
- **WHEN** Conflux reconciles runtime and workspace state
- **THEN** the metadata is invalidated or quarantined
- **AND** reducer state does not remain stalled solely from stale metadata
- **AND** current repository evidence determines the safe next route

#### Scenario: bare GATED produces no stalled metadata

- **GIVEN** Acceptance emits bare GATED or legacy blocked compatibility input without valid structured blocker evidence
- **WHEN** runtime handles the result
- **THEN** it records no stalled blocker metadata
- **AND** it emits no stalled lifecycle transition
- **AND** bounded Acceptance protocol retry handles the result

#### Scenario: Acceptance and Apply blockers remain distinguishable

- **GIVEN** runtime loads Acceptance stall state or workspace-local Apply blocker evidence
- **WHEN** it determines explicit retry behavior
- **THEN** Acceptance state is identified by its versioned external schema and revision binding
- **AND** Apply-origin, legacy unknown-origin, or non-resumable workspace evidence is not assumed to be Acceptance-generated
- **AND** only a valid resumable Acceptance record authorizes Acceptance-only retry
