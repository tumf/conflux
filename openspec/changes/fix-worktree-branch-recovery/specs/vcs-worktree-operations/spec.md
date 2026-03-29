## MODIFIED Requirements

### Requirement: Worktree delete removes branch

When deleting a worktree from the Worktrees view, including deletion triggered by the `D` key on the selected row, the system MUST also attempt to delete the associated local branch after worktree removal succeeds.

If the branch does not exist or branch deletion fails, the worktree deletion MUST still be treated as successful, and the branch deletion failure MUST be logged as a warning.

#### Scenario: Branch is deleted when worktree is deleted from TUI
- **GIVEN** A worktree is selected in the Worktrees view
- **AND** the target worktree has an associated local branch
- **WHEN** the user deletes the worktree with `D`
- **THEN** the worktree is removed
- **AND** the local branch is also deleted
- **AND** success logs for both worktree and branch deletion are recorded

#### Scenario: Worktree deletion succeeds even if branch deletion fails
- **GIVEN** A worktree is selected in the Worktrees view
- **AND** branch deletion returns an error or the branch is already absent
- **WHEN** the user deletes the worktree with `D`
- **THEN** the worktree deletion is treated as successful
- **AND** a warning log for the branch deletion failure is recorded

### Requirement: Worktree add failure diagnostics and safe retry

The system MUST classify representative `git worktree add` failures from stderr and include the classification in diagnostics.

The minimum classified causes MUST include:
- existing path (the worktree path already exists)
- branch duplicate (already checked out in another worktree)
- existing branch (the branch exists locally but is not necessarily checked out)
- invalid reference (the base commit or branch does not exist)
- permission error

If `git worktree add` fails because the target path already exists, the system MUST verify whether the path is stale, run `git worktree prune` only for stale paths, remove the stale directory, and retry exactly once.

If that retry fails because the branch already exists, the system MUST perform the same safe attach check used for direct existing-branch failures and attach the branch only when it is not checked out in another worktree.

If the retry still fails, the system MUST preserve the original error and retry diagnostics in the reported failure.

#### Scenario: Stale path retry falls through to existing branch attach
- **GIVEN** The target worktree path exists but is not registered in `git worktree list`
- **AND** the target local branch already exists
- **AND** the branch is not checked out in any other worktree
- **WHEN** `git worktree add <path> -b <branch> <base>` first fails due to the stale path and is retried after prune and stale-directory removal
- **THEN** the system attempts `git worktree add <path> <branch>` once
- **AND** worktree creation succeeds

#### Scenario: Stale path retry does not attach a checked-out branch
- **GIVEN** The target worktree path exists but is not registered in `git worktree list`
- **AND** the target local branch already exists
- **AND** the branch is checked out in another worktree
- **WHEN** `git worktree add <path> -b <branch> <base>` first fails due to the stale path and is retried after prune and stale-directory removal
- **THEN** the system does not attach the existing branch
- **AND** the final failure is reported with classified diagnostics for the original and retry errors
