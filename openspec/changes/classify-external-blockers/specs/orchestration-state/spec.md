## ADDED Requirements

### Requirement: Orchestrator classifies external blockers separately from stalls

Conflux SHALL make the final lifecycle classification from validated structured evidence. A change SHALL be `blocked` when a concrete non-repository prerequisite prevents useful execution and the evidence includes a verifiable unblock condition. A change SHALL be `stalled` when automatic execution stops because of no semantic progress, repeated findings, or exhausted retry or repair policy.

Agents SHALL report blocker facts but SHALL NOT directly assign canonical lifecycle state. Compatibility verdict token text alone SHALL NOT determine the classification.

#### Scenario: Structured external prerequisite becomes blocked

- **GIVEN** Apply or Acceptance reports a supported external prerequisite with non-empty evidence, prerequisite or owner, verifiable unblock condition, next action, and resumability
- **AND** repository-only work or a test double cannot satisfy the prerequisite
- **WHEN** the orchestrator validates the report
- **THEN** the proposal lifecycle status is `blocked`
- **AND** blocker metadata identifies the blocker kind as external and preserves its origin and evidence

#### Scenario: Exhausted execution remains stalled

- **GIVEN** a change repeats the same finding, makes no semantic progress, or exhausts its retry or repair budget
- **WHEN** the orchestrator finalizes the automatic execution outcome
- **THEN** the proposal lifecycle status is `stalled`
- **AND** it is not classified as external `blocked` merely because progress stopped

#### Scenario: Bare compatibility token does not choose lifecycle

- **GIVEN** an agent emits `gated` or legacy `blocked` without complete structured blocker evidence
- **WHEN** the orchestrator parses the compatibility input
- **THEN** bounded protocol correction handles the incomplete output
- **AND** the token text alone creates neither external `blocked` nor `stalled`

### Requirement: Blocked lifecycle preserves blocker kind

A blocked proposal SHALL expose whether it is waiting on a proposal dependency or an external prerequisite. External blocker metadata SHALL include origin, category, evidence, prerequisite or owner, unblock condition, next action, and resumability. Dependency blocking SHALL continue to derive from the proposal graph and SHALL NOT be represented as an external blocker.

#### Scenario: Dependency and external waits share status without losing meaning

- **GIVEN** one proposal waits on an unarchived dependency and another waits on a validated external prerequisite
- **WHEN** operator-facing state is rendered
- **THEN** both lifecycle statuses are `blocked`
- **AND** their blocker kinds are respectively dependency and external
- **AND** the external wait does not create a synthetic proposal dependency edge

### Requirement: External blocked state remains workspace-local and process-lifetime

External blocked runtime state SHALL remain in memory for the current process and SHALL NOT introduce out-of-worktree durable workflow control. Restart routing SHALL be recomputed from workspace file state, workspace git state, and base-branch comparison. Runtime blocker metadata SHALL NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or integration.

#### Scenario: Restart does not trust blocked runtime state

- **GIVEN** a process held a validated external blocked state
- **WHEN** Conflux restarts with the same workspace and git evidence
- **THEN** previous in-memory status has no routing authority
- **AND** Conflux re-evaluates the current repository evidence
- **AND** a complete unarchived Apply revision returns to Acceptance rather than inferring PASS or archive readiness

#### Scenario: Explicit retry reconciles current evidence

- **GIVEN** a proposal is externally blocked
- **WHEN** an operator requests retry
- **THEN** Conflux validates current workspace identity and unblock evidence before dispatch
- **AND** an unchanged or identity-mismatched blocker is not silently discarded
- **AND** evidence supporting a changed unblock condition permits the safe next action
