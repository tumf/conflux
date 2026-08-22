## ADDED Requirements

### Requirement: Change-blocking verification declarations remain cohesive and bounded

Native proposal validation MUST use structured verification declarations and parseable task verification references to reject heterogeneous or heavyweight work bundled into one change-blocking Acceptance gate. Reuse of one change-blocking verification ID across active task checkboxes MUST require the same exact ownership marker and the same normalized concrete command on every reference. The marker MUST be one case-insensitive closed-set token immediately after `verification:` and before the first ` - `; missing or ambiguous markers MUST produce a diagnostic. Command normalization MUST remove Markdown backticks, fold whitespace, trim, and compare case-insensitively. Validation MUST apply its structural denylist to every declared command form: `evidence`, `rerun`, task-line concrete commands, and structured argv when present. It MUST reject Docker/container orchestration, cross-architecture emulation, benchmark, explicit full/exhaustive/heavy suite, or repeated-stability forms as change-blocking commands and direct authors to bounded repository-local proof plus operational observation or repository automation. This native syntax rule MUST take precedence over generic evidence-hint recognition and guidance that otherwise permits a bounded repository-local integration path; an allowed bounded path MUST not match the denylist. The resulting diagnostic MUST be the heavyweight-command diagnostic, not `missing repository evidence`. Validation MUST NOT infer task meaning from free-text prose and does not detect unrelated tasks that deliberately declare the same marker and command.

#### Scenario: Heterogeneous tasks cannot share one blocker

**Given**: active task checkboxes reference one change-blocking verification ID
**And**: the task verification notes declare different ownership markers or concrete commands
**When**: strict validation runs
**Then**: validation fails with the verification ID and affected task lines
**And**: the diagnostic directs the author to split the verification declarations

#### Scenario: Heavy broad suite cannot block Acceptance

**Given**: any `evidence`, `rerun`, task concrete command, or structured argv for a change-blocking verification structurally selects container orchestration, cross-architecture emulation, benchmark, full, exhaustive, heavy, or repeated stability execution
**When**: strict validation runs
**Then**: validation fails
**And**: the diagnostic requires bounded repository-local proof and separately owned broad verification

#### Scenario: Heavy evidence cannot hide behind a focused rerun

**Given**: a change-blocking declaration has a focused `rerun` command
**And**: its `evidence` or structured argv matches the heavyweight denylist
**When**: strict validation runs
**Then**: validation fails with the heavyweight-command diagnostic

#### Scenario: Native denylist overrides bounded-path guidance

**Given**: a proposal declares a bounded repository-local path
**And**: its command form matches the structural denylist
**When**: strict validation runs
**Then**: native validation rejects the command
**And**: generic evidence-hint recognition does not make it valid

#### Scenario: Focused proof may cover coupled tasks

**Given**: implementation and regression-test tasks reference the same change-blocking ID
**And**: both name the same focused ownership marker and concrete command
**When**: strict validation runs
**Then**: the shared declaration remains valid

#### Scenario: Prose does not control cohesion

**Given**: narrative text uses words such as heavy or full outside parseable verification syntax
**When**: strict validation runs
**Then**: that prose does not create or alter a workflow-control classification
