## ADDED Requirements

### Requirement: Change-blocking verification declarations remain cohesive and bounded

Native proposal validation MUST use structured verification declarations and parseable task verification references to prevent unrelated or heavyweight work from being bundled into one change-blocking Acceptance gate. Reuse of one change-blocking verification ID across active task checkboxes MUST require the same verification ownership marker and the same concrete rerun command on every reference. Validation MUST reject structurally identified Docker/container orchestration, cross-architecture emulation, benchmark, explicit full/exhaustive/heavy suite, or repeated-stability commands as change-blocking evidence and direct authors to bounded repository-local proof plus operational observation or repository automation. Validation MUST NOT infer task meaning from free-text prose, and MUST permit a focused command to prove multiple tightly coupled tasks.

#### Scenario: Heterogeneous tasks cannot share one blocker

**Given**: active task checkboxes reference one change-blocking verification ID
**And**: the task verification notes declare different ownership markers or concrete commands
**When**: strict validation runs
**Then**: validation fails with the verification ID and affected task lines
**And**: the diagnostic directs the author to split the verification declarations

#### Scenario: Heavy broad suite cannot block Acceptance

**Given**: a change-blocking verification rerun command structurally selects container orchestration, cross-architecture emulation, benchmark, full, exhaustive, heavy, or repeated stability execution
**When**: strict validation runs
**Then**: validation fails
**And**: the diagnostic requires bounded repository-local proof and separately owned broad verification

#### Scenario: Focused proof may cover coupled tasks

**Given**: implementation and regression-test tasks reference the same change-blocking ID
**And**: both name the same focused ownership marker and concrete command
**When**: strict validation runs
**Then**: the shared declaration remains valid

#### Scenario: Prose does not control cohesion

**Given**: narrative text uses words such as heavy or full outside parseable verification syntax
**When**: strict validation runs
**Then**: that prose does not create or alter a workflow-control classification
