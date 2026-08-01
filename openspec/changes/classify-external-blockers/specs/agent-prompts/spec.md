## MODIFIED Requirements

### Requirement: Apply prompt MUST escalate implementation blockers

Apply guidance MUST distinguish repository-fixable work, mockable dependencies, non-repository external prerequisites, and terminal rejection proposals.

When Apply cannot proceed because of a recoverable external prerequisite, it MUST record structured blocker facts including category, concrete evidence, affected scope, prerequisite or owner, verifiable unblock condition, next action, and resumability. It MUST return the compatible machine-readable blocked outcome without creating `REJECTED.md`.

Apply guidance MUST state that the agent reports facts and that Conflux validates those facts and owns the final `blocked` versus `stalled` lifecycle classification. The agent MUST NOT claim canonical lifecycle status from prose or outcome token spelling.

#### Scenario: Apply reports an external prerequisite for orchestrator classification

- **GIVEN** Apply verifies that repository-only work and test doubles cannot satisfy a current external prerequisite
- **WHEN** it escalates the blocker
- **THEN** it records category, evidence, affected scope, prerequisite or owner, unblock condition, next action, and resumability
- **AND** it emits the compatible machine-readable blocked outcome
- **AND** it does not create `REJECTED.md`
- **AND** it leaves final lifecycle classification to Conflux

#### Scenario: Apply does not externalize repository work

- **GIVEN** code, tests, specs, tasks, documentation, fixtures, mocks, or stubs can resolve the finding
- **WHEN** Apply evaluates whether to escalate
- **THEN** it continues repository work or reports a repository-fixable failure
- **AND** it does not label the finding as an external prerequisite

### Requirement: Acceptance prompt MUST evaluate implementation blockers

Acceptance guidance MUST distinguish repository-fixable findings, validated external prerequisite evidence, stalled execution conditions, and protocol-invalid bare blocker verdicts.

For an external prerequisite, Acceptance MUST report an explicit supported category, concrete evidence, prerequisite or owner, verifiable unblock condition, next action, and resumability, and MUST state why repository-only Apply work or a test double cannot resolve it. Acceptance MUST state that Conflux owns final lifecycle classification.

Acceptance MUST use FAIL for repository-fixable findings. Repeated findings, no semantic progress, and exhausted repair policy are stalled execution inputs rather than external prerequisites. Bare `gated` or legacy `blocked` compatibility input remains protocol-incomplete and MUST NOT directly set lifecycle state.

#### Scenario: Acceptance reports complete external blocker facts

- **GIVEN** Acceptance verifies a non-repository prerequisite that Apply and test doubles cannot resolve
- **WHEN** it emits a compatibility blocker verdict
- **THEN** it includes category, evidence, prerequisite or owner, unblock condition, next action, and resumability
- **AND** it leaves `blocked` versus `stalled` lifecycle classification to Conflux

#### Scenario: Acceptance distinguishes stall from external wait

- **GIVEN** Acceptance repeats the same finding, observes no semantic progress, or reaches exhausted repair policy
- **WHEN** it reports the execution condition
- **THEN** it does not fabricate an external prerequisite
- **AND** Conflux may classify the validated execution outcome as `stalled`

#### Scenario: Bare compatibility verdict remains protocol-invalid

- **GIVEN** Acceptance emits bare `gated` or legacy `blocked` without required structured fields
- **WHEN** runtime performs bounded corrective retry
- **THEN** guidance requests either a repository-fixable FAIL or complete blocker facts
- **AND** the token alone sets neither canonical `blocked` nor canonical `stalled`
