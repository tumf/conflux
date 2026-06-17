---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/key_handlers.rs
  - src/tui/terminal.rs
  - src/tui/worktrees.rs
  - src/vcs/git/commands/worktree.rs
  - src/config/expand.rs
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/CONSTITUTION.md
---

# Fix TUI worktree command cwd safety

**Change Type**: implementation

## Premise / Context

- A user reported that pressing `+` in the TUI Worktrees view created a `proposal-*` directory that appeared empty, and running `oc` from that tmux shell failed with `The current working directory was deleted`.
- The current `handle_plus_key()` path creates a Git worktree, runs `.wt/setup`, and then launches the configured `worktree_command` with the new worktree as cwd.
- The current setup-failure path removes the newly created worktree, while command execution relies on the worktree path remaining valid.
- The observed directory was later absent from `git worktree list --porcelain`, while manual `git worktree add` to the same external `XDG_DATA_HOME` path materialized files correctly.
- The likely fault boundary is not raw Git worktree creation, but missing materialization/cwd validation and insufficient visible diagnostics around cleanup before command launch.
- The Conflux constitution requires workflow-control decisions to remain workspace/git/base-derived and completion to be repository-verifiable.

## Problem / Context

TUI Worktrees `+` is intended to create a usable worktree and immediately open the configured user command in that worktree. Today the path can be invalid, deleted, or not a materialized Git worktree by the time the command starts or by the time the user interacts with the spawned shell. When that happens, the user sees an apparently empty directory or a shell whose cwd has been deleted, but the TUI does not provide enough validation or diagnostics to distinguish setup cleanup, worktree materialization failure, command launch failure, or external deletion.

This is high priority because it makes the `+` shortcut look successful while leaving the operator in an unusable shell and risks obscuring whether Conflux deleted the worktree intentionally after setup failure.

## Proposed Solution

Tighten the Worktrees `+` lifecycle so Conflux only launches `worktree_command` after the target path is verified as a materialized, registered Git worktree, and so any cleanup or invalid cwd condition is surfaced before launching external commands.

1. Add a reusable validation path for worktree command cwd readiness after `git worktree add`, after `.wt/setup`, and immediately before command launch.
2. Validate repository evidence instead of UI-only state: the path exists, is a directory, contains usable Git metadata, resolves as a Git toplevel, and appears in `git worktree list --porcelain` for the base repo.
3. If `.wt/setup` fails and Conflux removes the worktree, log both the setup failure and the cleanup action with the target path before returning.
4. If validation fails, do not launch `worktree_command`; log a clear diagnostic explaining which validation failed and whether cleanup was attempted.
5. Add regression coverage for successful `+` worktree command launch, setup-failure cleanup logging, invalid/deleted cwd suppression, and configured commands that rely on `{workspace_dir}` or cwd propagation.

## Acceptance Criteria

- Pressing `+` in Worktrees view with a valid repo and configured `worktree_command` creates a registered Git worktree whose files and `.git` metadata are materialized before any external command is launched.
- The TUI does not launch `worktree_command` when the created path is missing, not a directory, not a Git worktree, not registered in `git worktree list`, or fails to resolve as the expected worktree toplevel.
- If `.wt/setup` fails and the created worktree is removed, the TUI logs that setup failed, that cleanup is being performed, and the affected path.
- If the worktree path disappears or becomes invalid between setup and command execution, the TUI logs a warning/error and leaves the operator out of a deleted cwd instead of launching the configured command there.
- Existing successful Worktrees `+` behavior remains intact for valid worktrees and existing `worktree_command` templates.
- The validation and diagnostics are observability/safety behavior only and do not introduce durable workflow-control state outside workspace/git/base evidence.

## Explicit Completion Conditions

- `src/tui/key_handlers.rs`, `src/tui/terminal.rs`, `src/tui/worktrees.rs`, and/or `src/vcs/git/commands/worktree.rs` include repository-evidence validation before `worktree_command` launch.
- Failure paths include visible TUI log entries for setup failure, cleanup start/result, and invalid command cwd suppression.
- Tests cover real or fixture-backed Git worktree materialization and deletion/invalid-cwd suppression, not only string formatting or placeholder validation.
- Tests prove a valid worktree still launches the configured command through `execute_worktree_command` or its equivalent boundary.
- `cflx openspec validate fix-tui-worktree-cwd-safety --strict --evidence warn` passes.
- Targeted Rust tests for TUI worktree command handling and Git worktree validation pass locally.

## Out of Scope

- Changing Git worktree creation semantics for parallel apply/archive workspaces.
- Redesigning the Worktrees view UI layout or key bindings.
- Changing scheduler resume, acceptance, archive, or merge workflow-control rules.
- Requiring users to change existing `worktree_command` templates, though documentation/tests may show safer `{workspace_dir}` or tmux `-c` usage patterns.
