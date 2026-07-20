## MODIFIED Requirements

### Requirement: Stalled blocker metadata

When a change enters non-terminal stalled state because of infrastructure, external dependency, credential, pending verification, repeated acceptance findings without progress, or acceptance cycle exhaustion, reducer-owned state and workspace-local blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the blocker from dependency blocking and terminal rejection.

The metadata MUST include the failed phase, stable stalled reason, observed error or finding summary, normalized finding identities when available, retry count, semantic progress result, retained external blockers, resumability, recommended next action, and worktree preservation context. The stalled reason MUST distinguish at least `repeated_acceptance_findings` and `acceptance_cycle_limit_exhausted`.

#### Scenario: repeated acceptance findings record evidence and next action

- **GIVEN** the same normalized acceptance findings recur after an apply retry
- **AND** no semantic repository progress exists
- **WHEN** the change enters stalled state
- **THEN** metadata records `repeated_acceptance_findings`
- **AND** it includes current findings, identities, retry count, and the no-progress result
- **AND** it states that the hold is resumable and explains how to retry after resolving the blocker

#### Scenario: cycle exhaustion is distinct from terminal failure

- **GIVEN** acceptance reaches its apply+acceptance cycle ceiling
- **WHEN** runtime records the final bounded outcome
- **THEN** metadata records `acceptance_cycle_limit_exhausted`
- **AND** display status is execution `stalled`, not dependency `blocked` or terminal `error`
- **AND** the workspace remains available for explicit retry

#### Scenario: external blocker remains visible with repository findings

- **GIVEN** acceptance observed repository-fixable findings and an external blocker
- **WHEN** stalled metadata is created after repository retries
- **THEN** the external blocker remains present with its evidence and next action
- **AND** it is distinguishable from repository repair tasks

#### Scenario: workspace evidence survives runtime-state deletion

- **GIVEN** acceptance stalled metadata is represented in the workspace blocker marker
- **WHEN** out-of-worktree runtime state is deleted and Conflux restarts
- **THEN** the same stalled reason and next action are derived from the workspace
