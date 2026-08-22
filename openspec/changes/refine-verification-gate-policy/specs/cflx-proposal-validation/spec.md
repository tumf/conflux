## MODIFIED Requirements

### Requirement: Proposal verification plans bound Apply-owned work

Bundled proposal guidance MUST keep Apply-blocking verification repository-local and bounded to one direct execution by default. Docker orchestration, database services, heavy or credentialed execution, deployed-service checks, physical-device checks, external approval, and long-running repository-wide gates MUST be assigned to repository automation, Acceptance, benchmark, manual, or operational-observation ownership unless the proposal declares a bounded repository-local path that can complete within one Apply invocation. Proposal guidance MUST NOT create checkbox tasks whose sole purpose is repeated stability execution of the same verification command. Native validation MUST inspect only structured `verifications[].evidence` and `verifications[].rerun` commands for deterministic heavyweight forms; task prose MUST NOT be a command-authority source. During migration, matches MUST produce actionable strict-validation warnings and MUST NOT fail archive-gate validation. Matching MUST use exact command tokens or explicit adjacent token pairs rather than substring inference. Bounded `docker build` evidence MUST remain valid.

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
