## MODIFIED Requirements

### Requirement: Proposal verification plans bound Apply-owned work

Bundled proposal guidance MUST keep Apply-blocking verification repository-local and bounded to one direct execution by default. Docker orchestration, database services, heavy or credentialed execution, deployed-service checks, physical-device checks, external approval, and long-running repository-wide gates MUST be assigned to repository automation, Acceptance, benchmark, manual, or operational-observation ownership unless the proposal declares a bounded repository-local path that can complete within one Apply invocation. Proposal guidance MUST NOT create checkbox tasks whose sole purpose is repeated stability execution of the same verification command. Native validation MUST inspect only structured `verifications[].evidence` and `verifications[].rerun` commands for deterministic heavyweight forms; task prose MUST NOT be a command-authority source. During migration, matches MUST produce actionable strict-validation warnings and MUST NOT fail archive-gate validation. Matching MUST use exact command tokens or explicit adjacent token pairs rather than substring inference. Bounded `docker build` evidence MUST remain valid.

#### Scenario: Heavy repository gate is not an Apply checkbox

**Given**: a change requires a Docker and database repository-wide validation suite
**And**: requirement-specific repository-local tests can prove implementation before integration
**When**: bundled proposal guidance creates the verification plan
**Then**: active implementation checkboxes reference the bounded repository-local tests
**And**: the heavy suite is assigned to repository automation, Acceptance, or operational observation
**And**: no checkbox requires repeated execution of the heavy suite

#### Scenario: Heavy repository gate produces a migration warning

**Given**: a change-blocking verification declares `docker compose up --wait`, `docker run`, `cargo bench`, `--workspace`, `--all-features`, `--ignored`, `--include-ignored`, `--features heavy`, `--exhaustive`, `qemu-system-*`, `cross`, `seq`, or `xargs` in `evidence` or `rerun`
**When**: strict validation runs during migration
**Then**: validation emits an actionable warning naming the verification ID and matched form
**And**: archive-gate validation does not fail from that warning

#### Scenario: Bounded Docker build may block completion

**Given**: a Dockerfile change declares a bounded repository-local `docker build` command
**When**: strict validation runs
**Then**: no heavyweight-command finding is emitted for `docker build`

#### Scenario: Command substrings do not trigger warnings

**Given**: a bounded test selector contains `full`, `heavy`, `benchmark`, or `exhaustive` inside a longer token
**When**: strict validation runs
**Then**: no heavyweight-command finding is emitted from that substring

#### Scenario: Task prose is not command authority

**Given**: task prose contains heavyweight words or a task-local command that differs from frontmatter
**When**: strict validation runs
**Then**: validation emits no cohesion or heavyweight finding from that prose

#### Scenario: Legacy verification declarations remain valid

**Given**: an active proposal lacks structured execution or completion roles required for current verification linkage
**When**: strict validation runs
**Then**: the new warning policy remains inert for that declaration
**And**: existing migration compatibility is preserved

#### Scenario: Bounded repository-local integration test may block completion

**Given**: a behavior can be verified with a local fixture in one direct bounded command
**When**: proposal guidance declares that verification
**Then**: it may be `pre-integration`, `repository-local`, and `change-blocking`
**And**: the task references the verification ID without duplicating command authority

#### Scenario: Non-local verification cannot be hidden in task prose

**Given**: an outcome requires credentials, deployment, physical hardware, or external approval
**When**: proposal guidance writes tasks and structured verifications
**Then**: the outcome is not attached to an Apply-blocking checkbox
**And**: its structured verification role is operational observation or narrative Future Work

## REMOVED Requirements

### Requirement: Change-blocking verification declarations remain cohesive and bounded

**Reason**: Exact task-note cohesion rewarded repeated labels instead of detecting unrelated work, duplicated frontmatter command authority in task prose, and rejected legitimate narrow task filters. Structured `verifications[].evidence` and `verifications[].rerun` are now the single command authority, and heavyweight detection is a warning-only exact-token policy carried by the requirement above.

**Migration**: No proposal changes are required. Task notes keep linking checkboxes to verification IDs via `verification-id:`; they are simply no longer parsed for ownership-marker or command cohesion. Declarations that previously failed on a heavyweight `evidence` or `rerun` now emit a strict-validation warning that does not fail archive-gate validation.
