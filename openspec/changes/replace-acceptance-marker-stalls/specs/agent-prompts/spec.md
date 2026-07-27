## MODIFIED Requirements

### Requirement: Acceptance prompt MUST evaluate implementation blockers

Acceptance prompts and distributed Acceptance skills MUST distinguish repository-fixable findings, validated stalled blockers, and protocol-invalid bare blocker verdicts.

A stalled handoff MUST include an explicit supported category, concrete non-empty evidence, next action, and resumability, and MUST state why repository-only Apply work cannot resolve the prerequisite. During compatibility migration, the machine-readable verdict MAY use the parser-supported `gated` token only when the structured blocker payload accompanies it. Bare `{"acceptance":"gated"}`, `ACCEPTANCE: GATED`, or legacy `blocked` input is a protocol error subject to bounded Acceptance retry and MUST NOT be presented as sufficient stalled evidence.

Guidance MUST use `stalled` for user-facing lifecycle taxonomy, MUST NOT instruct Acceptance to create `APPLY_BLOCKED` or another runtime marker under the change directory, and MUST NOT infer credential, infrastructure, or other categories from narrative prose. Repository-fixable code, tests, specs, tasks, documentation, or mockable dependencies remain FAIL findings returned to Apply.

#### Scenario: Acceptance emits structured blocker handoff

- **GIVEN** Acceptance verifies a prerequisite that repository-only work cannot resolve
- **WHEN** the reviewer emits a stalled compatibility handoff
- **THEN** it supplies an explicit supported category, concrete evidence, next action, and resumability with the machine-readable verdict
- **AND** runtime/user-facing status is `stalled`
- **AND** the reviewer does not create a marker in the change directory

#### Scenario: bare GATED is corrected by retry

- **GIVEN** an Acceptance response emits bare `gated` or legacy `blocked` without the required blocker fields
- **WHEN** runtime invokes the bounded corrective retry
- **THEN** prompt guidance identifies the prior output as protocol-incomplete
- **AND** it asks for one canonical repository-fixable verdict or one fully structured stalled blocker
- **AND** it does not suggest a category or fabricate evidence for the reviewer

#### Scenario: Acceptance uses FAIL for repository-fixable issues

- **GIVEN** Acceptance finds code, tests, tasks, specs, documentation, or a mockable dependency that repository work can resolve
- **WHEN** the reviewer emits a machine-readable verdict
- **THEN** it uses FAIL with concrete current-worktree findings
- **AND** no stalled state or change-directory blocker marker is requested

#### Scenario: non-mockable credential blocker retains explicit evidence

- **GIVEN** a required credential is non-mockable for the declared verification phase
- **AND** Acceptance identifies the credential name or owning prerequisite and the exact rerun action
- **WHEN** it emits a structured blocker
- **THEN** it explicitly selects category `credential`
- **AND** it includes the concrete evidence and next action
- **AND** runtime does not derive the category merely because narrative text contains credential, token, or auth
