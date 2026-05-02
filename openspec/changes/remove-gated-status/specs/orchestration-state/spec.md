## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish blocker-adjacent concerns without exposing a separate `gated` lifecycle/display status:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- non-terminal resumable or review-required hold (`stalled`), including apply, rejecting-review, and acceptance-gate holds

Derived display status exposed from reducer-owned runtime state SHALL preserve the distinction between dependency waits, resumable holds, and terminal rejection without using `gated` as a user-facing status.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed as stalled
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `stalled`
- **AND** blocker metadata identifies the reason as an acceptance/review hold such as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and terminal `rejected`
- **AND** no user-facing lifecycle/display status is `gated`

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

Acceptance-gate and rejecting-review holds that are not confirmed as rejected MUST remain non-terminal and display as `stalled` when execution is paused for intervention.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

#### Scenario: unconfirmed acceptance hold remains stalled

- **GIVEN** acceptance reports an implementation blocker
- **AND** rejecting review has not confirmed terminal rejection
- **WHEN** the reducer exposes the paused lifecycle state
- **THEN** the terminal result remains `None`
- **AND** the derived display status is `stalled`
