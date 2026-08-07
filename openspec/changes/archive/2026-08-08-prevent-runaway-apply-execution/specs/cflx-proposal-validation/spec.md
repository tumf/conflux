## ADDED Requirements

### Requirement: Proposal verification plans bound Apply-owned work

Bundled proposal guidance MUST keep Apply-blocking verification repository-local and bounded to one direct execution by default. Docker, database, heavy, credentialed, deployed-service, physical-device, external-approval, and long-running repository-wide gates MUST be assigned to repository automation, Acceptance, benchmark, manual, or operational-observation ownership unless the proposal declares a bounded repository-local path that can complete within one Apply invocation. Proposal guidance MUST NOT create checkbox tasks whose sole purpose is repeated stability execution of the same verification command.

#### Scenario: Heavy repository gate is not an Apply checkbox

**Given**: a change requires a Docker and database repository-wide validation suite
**And**: requirement-specific repository-local tests can prove implementation before integration
**When**: bundled proposal guidance creates the verification plan
**Then**: active implementation checkboxes reference the bounded repository-local tests
**And**: the heavy suite is assigned to repository automation, Acceptance, or operational observation
**And**: no checkbox requires repeated execution of the heavy suite

#### Scenario: Bounded repository-local integration test may block completion

**Given**: a database behavior can be verified with a local fixture in one direct bounded command
**When**: proposal guidance declares that verification
**Then**: it may be `pre-integration`, `repository-local`, and `change-blocking`
**And**: the task names one rerun command and does not prescribe a stability loop

#### Scenario: Non-local verification cannot be hidden in task prose

**Given**: an outcome requires credentials, deployment, physical hardware, or external approval
**When**: proposal guidance writes tasks and structured verifications
**Then**: the outcome is not attached to an Apply-blocking checkbox
**And**: its structured verification role is operational observation or narrative Future Work
