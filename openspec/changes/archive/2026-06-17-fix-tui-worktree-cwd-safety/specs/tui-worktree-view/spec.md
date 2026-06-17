## ADDED Requirements

### Requirement: Worktree command launch uses validated cwd

When the user creates a worktree from the TUI Worktrees view and Conflux is about to launch `worktree_command`, the system MUST verify that the target cwd is a materialized and registered Git worktree. The validation MUST use repository evidence: filesystem existence, directory type, usable Git worktree metadata or toplevel resolution, and registration in `git worktree list` for the base repository.

#### Scenario: valid worktree command launch

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **AND** `git worktree add` creates a registered worktree with materialized files and Git metadata
- **AND** `.wt/setup` is absent or succeeds
- **WHEN** the user presses `+`
- **THEN** Conflux validates the created worktree path before launching the configured command
- **AND** the configured command is launched with the validated worktree path as its cwd

#### Scenario: invalid created worktree does not launch command

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **WHEN** the user presses `+`
- **AND** the created worktree path is missing, not a directory, not a Git worktree, or not registered in `git worktree list`
- **THEN** Conflux does not launch `worktree_command`
- **AND** the TUI logs a diagnostic that identifies the invalid worktree path and validation failure

#### Scenario: deleted cwd before launch is suppressed

- **GIVEN** `git worktree add` initially creates a valid worktree for the TUI `+` action
- **AND** `.wt/setup` succeeds or is absent
- **WHEN** the worktree path is deleted or invalidated before the command-launch boundary
- **THEN** Conflux revalidates the path before command launch
- **AND** Conflux does not launch `worktree_command` in the deleted or invalid cwd
- **AND** the TUI logs that command launch was suppressed because the cwd is invalid

### Requirement: Setup failure cleanup is visible

When `.wt/setup` fails during TUI Worktrees `+` creation and Conflux removes the newly created worktree, the TUI MUST log the setup failure, the cleanup action, and the cleanup result with the affected path before returning control to the operator.

#### Scenario: setup failure cleanup logs path and result

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **AND** `git worktree add` creates a valid worktree
- **WHEN** `.wt/setup` fails
- **THEN** Conflux logs the setup failure
- **AND** Conflux logs that it is cleaning up the created worktree path
- **AND** Conflux logs whether cleanup succeeded or failed
- **AND** Conflux does not launch `worktree_command`

### Requirement: Worktree command validation remains observability-only

TUI Worktrees `+` validation, diagnostics, and cleanup logging MUST NOT introduce durable workflow-control state outside workspace file state, workspace git state, or base-branch tree comparison.

#### Scenario: validation state is not workflow control input

- **GIVEN** TUI Worktrees `+` validation records diagnostics for an invalid or deleted command cwd
- **WHEN** scheduler dispatch, resume routing, acceptance gating, archive routing, or next-action selection runs
- **THEN** those workflow decisions do not depend on the TUI validation diagnostics or transient UI state
- **AND** deleting external logs or UI caches does not change the next action chosen for the same workspace contents
