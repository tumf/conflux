## MODIFIED Requirements

### Requirement: CLI parallel mode

The CLI SHALL support a `--parallel` flag to enable parallel change execution using git worktrees. When parallel execution reuses an existing workspace, the CLI SHALL report that the change is resuming from detected workspace state rather than implying a fresh start.

#### Scenario: parallel run reuses an existing workspace

- **GIVEN** the user runs `cflx run --parallel`
- **AND** an existing workspace for a requested change is eligible for reuse
- **WHEN** workspace state detection determines the change should resume from that workspace
- **THEN** the CLI reports that the workspace is being reused
- **AND** the CLI includes the detected resume state in user-visible output

### Requirement: CLI parallel resume control

When `--no-resume` is specified, parallel execution MUST NOT reuse an existing workspace for automatic resume.

#### Scenario: no-resume disables automatic workspace reuse

- **GIVEN** the user runs `cflx run --parallel --no-resume`
- **AND** an existing workspace for a requested change is present
- **WHEN** parallel execution starts
- **THEN** the existing workspace is not reused for automatic resume
- **AND** execution uses a fresh workspace path or other non-resume path according to the workspace manager rules
