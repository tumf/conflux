## MODIFIED Requirements

### Requirement: Japanese Localization

The project SHALL provide README.ja.md as a complete Japanese translation.

#### Scenario: README.ja.md availability

- **GIVEN** a Japanese-speaking user visits the repository
- **WHEN** they look for documentation
- **THEN** README.ja.md provides complete Japanese documentation
- **AND** the content matches README.md in structure and completeness

#### Scenario: Technical consistency

- **WHEN** README.ja.md is compared with README.md
- **THEN** code examples are identical
- **AND** command-line examples are identical
- **AND** only prose text is translated to Japanese

#### Scenario: Parallel execution documentation parity

- **WHEN** README.ja.md documents parallel execution
- **THEN** it includes both jj workspaces and Git worktrees support
- **AND** VCS backend selection options (auto, jj, git) are documented
- **AND** CLI flags `--parallel`, `--max-concurrent`, `--vcs`, `--dry-run` are documented

#### Scenario: Hooks documentation parity

- **WHEN** README.ja.md documents hooks
- **THEN** it includes all current hook types (on_start, on_finish, on_error, on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end, on_queue_add, on_queue_remove, on_approve, on_unapprove)
- **AND** deprecated hooks are not documented
- **AND** placeholder variables match README.md
