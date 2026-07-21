## MODIFIED Requirements

### Requirement: Stalled blocker metadata

When a change enters non-terminal stalled state, reducer-owned state and workspace-local blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the blocker from dependency blocking and terminal rejection. Acceptance-generated evidence MUST include failed phase, stable reason, observed finding summary and identities when available, retry count, semantic progress result, retained external blockers, resumability, recommended next action, origin, and worktree preservation context.

#### Scenario: acceptance stalled evidence survives runtime-state deletion

- **GIVEN** an acceptance-generated stalled marker exists in the workspace
- **WHEN** out-of-worktree runtime state is deleted and Conflux restarts
- **THEN** the same stalled reason, evidence, resumability, and next action are derived from the workspace
- **AND** display status remains execution `stalled`

#### Scenario: acceptance and apply blockers remain distinguishable

- **GIVEN** workspace blocker evidence is loaded
- **WHEN** runtime determines whether explicit acceptance retry may consume it
- **THEN** marker origin and resumability are available to the decision
- **AND** legacy or unknown-origin evidence is not assumed to be acceptance-generated
