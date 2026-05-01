## ADDED Requirements

### Requirement: Worktree teardown script execution

The system MUST support an optional worktree-local `.wt/teardown` script that runs before deleting a Conflux-managed Git worktree.

When `<worktree-root>/.wt/teardown` exists and is executable, the system MUST execute it before invoking Git worktree removal. The teardown process MUST run non-interactively with the worktree root as its current working directory and MUST receive `ROOT_WORKTREE_PATH` pointing to the base repository root, matching the meaning used by `.wt/setup`.

`.wt/teardown` MUST be treated as a cleanup hook for side effects only. The system MUST NOT use teardown output or `.wt/state.env` as authoritative workflow-control state for resume routing, acceptance routing, archive routing, or next-action decisions.

#### Scenario: executable teardown runs before worktree removal

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **WHEN** the system deletes that worktree
- **THEN** the system runs `.wt/teardown` before invoking `git worktree remove`
- **AND** the teardown current working directory is the worktree root
- **AND** `ROOT_WORKTREE_PATH` points to the base repository root
- **AND** teardown stdin is non-interactive

#### Scenario: teardown can read worktree-local state

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** the worktree contains `.wt/state.env`
- **AND** `.wt/teardown` reads `.wt/state.env` by relative path
- **WHEN** the system runs teardown before deletion
- **THEN** `.wt/teardown` can read the worktree-local state file from the worktree root

#### Scenario: missing teardown keeps existing deletion behavior

- **GIVEN** a Conflux-managed worktree does not contain `.wt/teardown`
- **WHEN** the system deletes that worktree
- **THEN** the system proceeds with Git worktree removal without running a teardown hook

#### Scenario: non-executable teardown is not run

- **GIVEN** a Conflux-managed worktree contains `.wt/teardown`
- **AND** `.wt/teardown` is not executable
- **WHEN** the system deletes that worktree
- **THEN** the system does not execute `.wt/teardown`
- **AND** the system records a warning or diagnostic that teardown was skipped because it was not executable
- **AND** the system proceeds with Git worktree removal

### Requirement: Teardown failure preserves worktree by default

If executable `.wt/teardown` exits unsuccessfully, the system MUST abort deletion by default before invoking Git worktree removal. The system MUST preserve the worktree and report diagnostics containing enough context to debug cleanup failure, including the worktree path, base repository path, exit status, stdout, and stderr when available.

#### Scenario: teardown failure aborts deletion

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** `.wt/teardown` exits with a non-zero status
- **WHEN** the system deletes that worktree without an explicit teardown skip option
- **THEN** the deletion fails before Git worktree removal
- **AND** the worktree remains available for operator recovery
- **AND** diagnostics include the teardown exit status, stdout, stderr, worktree path, and base repository path

### Requirement: Operators can explicitly skip teardown blocking cleanup

The system MUST provide an explicit operator escape hatch for managed worktree deletion that allows deletion to proceed without teardown blocking cleanup. The option SHOULD be named `skip_teardown` or `--skip-teardown` on exposed API/CLI/UI surfaces to avoid confusion with Git worktree removal's `--force` flag.

When this option is used, the system MUST record that teardown was skipped or that teardown failure was intentionally ignored before deletion continued.

#### Scenario: skip teardown proceeds with deletion

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** `.wt/teardown` would fail or require external resources that are unavailable
- **WHEN** an operator deletes the worktree with the explicit teardown skip option
- **THEN** the system proceeds with Git worktree removal
- **AND** the system records a warning or diagnostic that teardown did not block deletion because the skip option was used

### Requirement: Managed worktree deletion paths use teardown-aware removal consistently

All Conflux-managed worktree deletion paths MUST use teardown-aware removal unless a path is explicitly documented as out of scope for repository-defined teardown. This includes parallel execution cleanup, dependency-resolved stale worktree cleanup, inconsistent worktree cleanup, rejection cleanup, TUI manual deletion, server/WebUI worktree deletion, legacy web deletion, and proposal-session worktree deletion.

#### Scenario: orchestration cleanup runs teardown

- **GIVEN** a Conflux-managed worktree with executable `.wt/teardown` is being removed by an orchestration cleanup path
- **WHEN** cleanup is triggered after merge, rejection, dependency-resolved recreation, or inconsistent workspace detection
- **THEN** the cleanup path runs teardown-aware removal
- **AND** teardown failure prevents deletion by default unless that path explicitly uses the operator skip option

#### Scenario: manual and API deletion run teardown

- **GIVEN** a Conflux-managed worktree with executable `.wt/teardown` is deleted through TUI, WebUI, or server API
- **WHEN** the deletion request does not specify teardown skip
- **THEN** the manual or API deletion path runs teardown-aware removal
- **AND** teardown failure is surfaced to the operator instead of silently deleting the worktree
