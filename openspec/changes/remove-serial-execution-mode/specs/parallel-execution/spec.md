## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Managed-worktree execution SHALL run `acceptance_command` after successful apply and before archive. Every configured frontend SHALL use the same verdict parsing, missing-verdict retry, history, restart, and stalled-hold behavior.

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the managed workspace remains applied but unarchived
- **WHEN** Conflux restarts
- **THEN** it runs acceptance again from workspace file and Git state
- **AND** it does not require a generated retry checkpoint

### Requirement: Parallel apply runs in worktree

Every change-level apply command MUST run in the selected change's managed worktree. A base-repository or other non-managed execution directory MUST fail before the apply command starts.

#### Scenario: apply outside managed worktree fails

- **GIVEN** a change is selected for execution
- **AND** its apply directory is not its managed worktree
- **WHEN** apply dispatch is attempted
- **THEN** execution fails with the change ID and invalid directory
- **AND** the base repository is not mutated by apply

### Requirement: VCS Backend Auto-Detection

The sole execution path SHALL auto-detect Git when `--vcs` is absent or `auto`. Executable orchestration without a usable Git repository SHALL fail before orchestration side effects.

#### Scenario: No VCS available

- **WHEN** executable orchestration starts outside a usable Git repository
- **THEN** an actionable Git-repository error is displayed
- **AND** the exit code is non-zero
- **AND** no serial fallback starts

### Requirement: AI エージェントクラッシュリカバリー

ApplyまたはArchiveコマンドの異常終了時、managed-worktree executionは既存transport retry、history、fresh repository/handoff evaluation、permission、progress、stall、およびper-change active-run `max_iterations` contractを維持しなければならない（MUST）。

#### Scenario: Apply command failures exhaust one per-change active-run budget

- **GIVEN** `max_iterations` is `3`
- **AND** one change has Apply dispatches before and after an Acceptance FAIL-to-Apply cycle
- **WHEN** the third cumulative configured Apply dispatch completes
- **THEN** no fourth Apply command starts from CLI, TUI, or remote-controlled execution
- **AND** the typed `iteration_limit` diagnostic includes the exact cumulative count
