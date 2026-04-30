## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL determine the next action for a workspace using only workspace-local evidence: the workspace file state, the workspace git state, and base-branch tree comparison.

Out-of-worktree durable workflow state MUST NOT be required for acceptance routing, archive routing, or resume routing.

When an `Applied` workspace resumes and workspace-local evidence does not prove archive handoff readiness, the runtime MUST re-run acceptance rather than trusting external durable state.

#### Scenario: applied workspace re-runs acceptance when workspace-local proof is absent

- **GIVEN** change `alpha` is detected as `Applied` from workspace-local git/file evidence
- **AND** the workspace does not contain sufficient local evidence proving archive handoff readiness
- **WHEN** the runtime resumes processing
- **THEN** the next phase is acceptance
- **AND** the runtime does not consult any state under `~/.local/state/cflx/` to skip directly to archive

### Requirement: Resume routing is independent of out-of-worktree durable state

For the same workspace contents, resume routing MUST produce the same result regardless of whether out-of-worktree durable state exists, is missing, or is stale.

#### Scenario: external durable state deletion does not change routing

- **GIVEN** change `beta` has a workspace whose file/git state resolves to a specific next phase
- **AND** one runtime run has pre-existing files under `~/.local/state/cflx/acceptance-state` and `~/.local/state/cflx/archive-resume-state`
- **AND** another runtime run deletes those directories before resume
- **WHEN** both runs evaluate resume routing for the same workspace contents
- **THEN** both runs choose the same next phase
- **AND** any difference in observability output does not alter workflow control

### Requirement: Archiving and archived states remain workspace-local

`Archiving` and `Archived` resume decisions SHALL be derived from the current workspace file/git state only.

External durable state MAY exist as non-authoritative observability output, but it MUST NOT cause a workspace to re-enter apply, acceptance, or archive when workspace-local evidence indicates otherwise.

#### Scenario: stale external state cannot revive an archived workspace

- **GIVEN** change `gamma` is detected as `Archived` from workspace-local archive completion evidence
- **AND** stale files remain under `~/.local/state/cflx/archive-resume-state`
- **WHEN** resume routing is performed
- **THEN** the runtime treats the workspace as archived and hands it to merge handling
- **AND** the stale external state does not route the workspace back into apply, acceptance, or archive
