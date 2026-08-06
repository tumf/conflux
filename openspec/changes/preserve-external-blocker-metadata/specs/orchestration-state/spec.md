## MODIFIED Requirements

### Requirement: Blocked lifecycle preserves blocker kind

A blocked proposal SHALL expose whether it is waiting on a proposal dependency or an external prerequisite. External blocker metadata SHALL include origin, category, evidence, prerequisite or owner, unblock condition, next action, and resumability. Dependency blocking SHALL continue to derive from the proposal graph and SHALL NOT be represented as an external blocker.

Structured blocker classifications SHALL take precedence over lower-fidelity workspace status observations for the same non-terminal change. After `AcceptanceGated` or `ExecutionBlocked` establishes an external blocked state or an Acceptance-owned stalled hold, a later or duplicate generic blocked workspace status SHALL NOT change the wait kind, clear structured blocker metadata, clear Acceptance ownership, or change resumability. A generic blocked workspace status MAY establish conservative stalled metadata only when no structured blocker classification already owns the wait.

#### Scenario: Dependency and external waits share status without losing meaning

- **GIVEN** one proposal waits on an unarchived dependency and another waits on a validated external prerequisite
- **WHEN** operator-facing state is rendered
- **THEN** both lifecycle statuses are `blocked`
- **AND** their blocker kinds are respectively dependency and external
- **AND** the external wait does not create a synthetic proposal dependency edge

#### Scenario: Acceptance external blocker survives generic blocked status

- **GIVEN** Acceptance reports a validated resumable external prerequisite with complete structured metadata
- **AND** the reducer classifies the change as external `blocked`
- **WHEN** the corresponding workspace later reports generic `Blocked`
- **THEN** the change remains external `blocked`
- **AND** category, evidence detail, origin, prerequisite owner, unblock condition, next action, and resumability remain unchanged
- **AND** the generic observation does not replace the hold with generic stalled metadata

#### Scenario: Apply external blocker receives the same precedence

- **GIVEN** Apply reports a validated external prerequisite through `ExecutionBlocked`
- **AND** the reducer classifies the change as external `blocked`
- **WHEN** a generic blocked workspace observation follows
- **THEN** the external blocker kind and Apply origin remain intact
- **AND** queue and operator projections continue to use the structured evidence

#### Scenario: Acceptance-owned execution stall survives generic blocked status

- **GIVEN** Acceptance reaches a structured non-external execution hold that is resumable
- **AND** the reducer records an Acceptance-owned `stalled` wait
- **WHEN** the workspace later reports generic `Blocked`
- **THEN** the change remains Acceptance-owned and `stalled`
- **AND** its resumability and structured guidance remain intact
- **AND** ordinary dispatch remains suppressed until explicit retry or process restart

#### Scenario: Generic blocked status remains a conservative fallback

- **GIVEN** a non-terminal change has no structured blocker classification
- **WHEN** its workspace reports generic `Blocked`
- **THEN** the reducer records the existing conservative generic `stalled` fallback
- **AND** it does not invent an external blocker kind, owner, origin, or unblock condition

#### Scenario: Preserved resumability controls explicit retry

- **GIVEN** a structured blocker has survived a later generic blocked workspace status
- **WHEN** an operator requests retry
- **THEN** a resumable hold follows the existing blocked-phase retry route
- **AND** a non-resumable hold is refused without discarding its blocker evidence
