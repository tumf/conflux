## MODIFIED Requirements

### Requirement: Apply prompt MUST escalate implementation blockers

Apply guidance MUST distinguish repository-fixable work, mockable dependencies, non-repository external prerequisites, and terminal rejection proposals.

When Apply cannot proceed because of a recoverable prerequisite, it MUST append `## Implementation Blocker #<n>` to `openspec/changes/{change_id}/tasks.md`. The section MUST contain category, concrete file or log evidence, affected scope, prerequisite or owner, verifiable unblock condition, next action, and resumability, and its bullets MUST NOT use checkboxes. Apply MUST emit an `IMPLEMENTATION_BLOCKER:` stdout block with the same facts and return the compatible machine-readable `BLOCKED` outcome without creating `REJECTED.md`.

Apply guidance MUST state that the agent reports facts and that Conflux validates those facts and owns the final `blocked` versus `stalled` lifecycle classification. The agent MUST NOT claim canonical lifecycle status from prose or outcome token spelling. `REJECTED.md` is permitted only when Apply explicitly establishes why closing the whole change is more appropriate than recovery.

#### Scenario: Apply records a recoverable prerequisite

- **GIVEN** Apply verifies that repository-only work and test doubles cannot satisfy a current prerequisite
- **WHEN** it escalates the blocker
- **THEN** tasks.md gains `## Implementation Blocker #<n>` with category, evidence, affected scope, prerequisite or owner, unblock condition, next action, and resumability
- **AND** the section contains no checkboxes
- **AND** stdout contains the matching `IMPLEMENTATION_BLOCKER:` block
- **AND** Apply emits the compatible machine-readable `BLOCKED` outcome
- **AND** it does not create `REJECTED.md`
- **AND** it leaves final lifecycle classification to Conflux

#### Scenario: Apply does not externalize repository work

- **GIVEN** code, tests, specs, tasks, documentation, fixtures, mocks, or stubs can resolve the finding
- **WHEN** Apply evaluates whether to escalate
- **THEN** it continues repository work or reports a repository-fixable failure
- **AND** it does not label the finding as an external prerequisite

#### Scenario: Apply distinguishes terminal rejection proposal

- **GIVEN** Apply establishes that a proposal premise is invalid or superseded and the whole change should close
- **WHEN** it proposes rejection
- **THEN** stdout distinguishes the rejection proposal from a recoverable blocker outcome
- **AND** worktree-local `REJECTED.md` is limited to this outcome

#### Scenario: Infrastructure verification blocker is not terminal rejection

- **GIVEN** Apply or verification observes Docker unavailability, image-pull DNS timeout, package-registry timeout, port conflict, third-party outage, rate limiting, or another infrastructure condition
- **AND** no independent evidence shows that the proposal premise is invalid or obsolete
- **WHEN** the agent records the blocker
- **THEN** guidance directs it to record recoverable structured blocker facts
- **AND** guidance does not direct it to create `REJECTED.md`

### Requirement: Acceptance prompt MUST evaluate implementation blockers

Acceptance prompts and distributed Acceptance skills MUST distinguish repository-fixable findings, validated external prerequisite evidence, stalled execution conditions, and protocol-invalid bare blocker verdicts.

For an external prerequisite, Acceptance MUST report an explicit supported category, concrete non-empty evidence, prerequisite or owner, verifiable unblock condition, next action, and resumability, and MUST state why repository-only Apply work or a test double cannot resolve it. Acceptance MUST state that Conflux owns final lifecycle classification and MUST NOT create a runtime marker under the change directory.

Acceptance MUST use FAIL for repository-fixable findings. Repeated findings, no semantic progress, and exhausted repair policy are stalled execution inputs rather than external prerequisites. Bare `gated` or legacy `blocked` compatibility input remains protocol-incomplete and MUST NOT directly set lifecycle state or cause runtime to infer a category from prose.

#### Scenario: Acceptance reports complete external blocker facts

- **GIVEN** Acceptance verifies a non-repository prerequisite that Apply and test doubles cannot resolve
- **WHEN** it emits a compatibility blocker verdict
- **THEN** it includes category, evidence, prerequisite or owner, unblock condition, next action, and resumability
- **AND** it leaves `blocked` versus `stalled` lifecycle classification to Conflux
- **AND** it creates no change-directory runtime marker

#### Scenario: Acceptance distinguishes stall from external wait

- **GIVEN** Acceptance repeats the same finding, observes no semantic progress, or reaches exhausted repair policy
- **WHEN** it reports the execution condition
- **THEN** it does not fabricate an external prerequisite
- **AND** Conflux may classify the validated execution outcome as `stalled`

#### Scenario: Bare compatibility verdict remains protocol-invalid

- **GIVEN** Acceptance emits bare `gated` or legacy `blocked` without required structured fields
- **WHEN** runtime performs bounded corrective retry
- **THEN** guidance requests either a repository-fixable FAIL or complete blocker facts
- **AND** guidance does not suggest a category or fabricate evidence
- **AND** the token alone sets neither canonical `blocked` nor canonical `stalled`

#### Scenario: Acceptance uses FAIL for repository-fixable issues

- **GIVEN** Acceptance finds code, tests, tasks, specs, documentation, or a mockable dependency that repository work can resolve
- **WHEN** it emits a machine-readable verdict
- **THEN** it uses FAIL with concrete current-worktree findings
- **AND** no blocked or stalled runtime marker is requested

#### Scenario: Non-mockable credential blocker retains explicit evidence

- **GIVEN** a required credential is non-mockable for the declared verification phase
- **AND** Acceptance identifies the credential name or owning prerequisite and exact rerun action
- **WHEN** it emits structured blocker facts
- **THEN** it explicitly selects category `credential`
- **AND** it includes concrete evidence, unblock condition, and next action
- **AND** runtime does not derive the category from narrative credential, token, or auth words
