## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `acceptance-blocked`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`
