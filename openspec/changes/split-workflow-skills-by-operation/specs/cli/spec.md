## MODIFIED Requirements

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into standard skill locations without requiring a source argument.

#### Scenario: Install bundled skills includes router and operation-specific skills

- **WHEN** the user runs `cflx install-skills`
- **THEN** the installed bundled skill set includes `cflx-proposal`, `cflx-run`, `cflx-workflow`, `cflx-analyze`, `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, `cflx-archive`, and `cflx-resolve`
- **AND** `cflx-workflow` is installed as a backward-compatible router alongside the new operation-specific skills

#### Scenario: Install bundled skills preserves per-skill auxiliary files and self-contained router compatibility

- **WHEN** the user runs `cflx install-skills`
- **THEN** each operation-specific skill installs the auxiliary files required for that operation within its own skill directory
- **AND** bundled skill installation does not reintroduce `scripts/cflx.py`
- **AND** `cflx-workflow` remains installable as a self-contained compatibility router
- **AND** legacy prompts that load only `cflx-workflow` do not require cross-skill auxiliary file access after installation
