## MODIFIED Requirements

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into standard skill locations without requiring a source argument.

#### Scenario: Install bundled skills includes router and operation-specific workflow skills

- **WHEN** the user runs `cflx install-skills`
- **THEN** the installed bundled skill set includes `cflx-proposal`, `cflx-run`, `cflx-workflow`, `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, and `cflx-archive`
- **AND** `cflx-workflow` is installed as a backward-compatible router alongside the new operation-specific workflow skills

#### Scenario: Install bundled skills preserves per-skill auxiliary files and self-contained router compatibility

- **WHEN** the user runs `cflx install-skills`
- **THEN** each operation-specific workflow skill installs the auxiliary files required for that operation within its own skill directory
- **AND** `cflx-workflow` remains installable as a self-contained compatibility router
- **AND** legacy prompts that load only `cflx-workflow` do not require cross-skill auxiliary file access after installation
