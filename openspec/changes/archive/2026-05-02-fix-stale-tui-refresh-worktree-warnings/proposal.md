---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - openspec/specs/observability/spec.md
  - src/tui/runner.rs
  - src/vcs/commands.rs
  - ~/.local/state/cflx/logs/log-startup-version-88179737/2026-04-29.log
---

# Fix Stale TUI Refresh Worktree Warnings

**Change Type**: implementation

## Problem / Context

After `~/.local/state/cflx/logs/.last-checked` (`2026-05-01 18:21:08 JST` marker mtime), recent cflx logs contain a high-volume warning loop from a TUI session for `log-startup-version`.

Observed examples in `/Users/tumf/.local/state/cflx/logs/log-startup-version-88179737/2026-04-29.log` after the marker mtime:

- `Failed to refresh committed change snapshot: git command failed: Failed to execute git: No such file or directory (os error 2); command: git ls-tree -d --name-only HEAD:openspec/changes; working_dir: /Users/tumf/.local/share/cflx/worktrees/conflux-bda270b8/log-startup-version`
- `Failed to refresh uncommitted files snapshot: ... command: git status --porcelain -u; working_dir: .../log-startup-version`
- `Failed to refresh worktree snapshot: ... command: git worktree list --porcelain; working_dir: .../log-startup-version`

These three warnings repeat every TUI auto-refresh tick, producing more than 44,000 instances per warning type. The failure is not a valid user-visible processing error: it is a stale TUI refresh loop trying to run read-only git snapshot commands with a deleted or otherwise missing working directory.

The current source still contains the relevant refresh path in `src/tui/runner.rs`: the auto-refresh task calls `list_changes_in_head`, `list_changes_with_uncommitted_files`, and `worktree_manager.list_worktree_change_ids()` every tick and only logs warnings when those git commands fail. `src/vcs/commands.rs` still converts `Command::new(program).current_dir(cwd).output()` failures into VCS command errors, so the same missing-working-directory condition can still recur.

Other recent warning groups were judged valid or out of scope for this proposal:

- `git ls-remote` / `git fetch` failures against GitHub are transient network/DNS failures and are already WARN-level sync-state failures.
- `human_action_required: acceptance must confirm rejection proposal` is a gated rejection review path, not a core/scope bug.
- `No progress made ... continuing...` indicates agent/application progress stalling in external project changes and is not itself evidence of a Conflux core false error.

## Proposed Solution

Make TUI auto-refresh resilient to stale or removed working directories.

The refresh task should detect when its configured repository/worktree root is no longer a usable git working directory before issuing the repeated snapshot commands. Once the directory is missing or invalid, the TUI should record a bounded, truthful warning and stop or suppress further local auto-refresh attempts for that stale root until the TUI exits or the root becomes usable again through an explicit reinitialization path.

The solution must preserve legitimate warnings for real command failures when the working directory exists and git commands fail for actionable reasons.

## Acceptance Criteria

- A TUI auto-refresh loop whose `repo_root` no longer exists does not emit the same `Failed to refresh ... snapshot` warnings on every tick.
- Missing or stale refresh roots are classified as stale refresh state rather than change-processing errors.
- The TUI logs at most one concise warning per stale root per affected TUI session, or otherwise applies an explicit rate limit/backoff that prevents log flooding.
- When the refresh root exists and git commands fail for actionable reasons, the TUI still logs enough warning context for debugging.
- Remote mode behavior remains unchanged: local auto-refresh is skipped when WebSocket updates own state.

## Explicit Completion Conditions

- `src/tui/runner.rs` or a helper it calls validates refresh root usability before the auto-refresh snapshot command group and avoids repeatedly executing VCS commands with a missing current directory.
- The stale-root path has unit or integration coverage proving repeated refresh ticks do not produce repeated warning events or repeated git command attempts against a missing path.
- Existing tests for normal local TUI refresh behavior continue to pass, and at least one test proves normal command failures with an existing root remain visible.
- The implementation does not introduce durable workflow-control state outside workspace/git/base-tree inputs, preserving `openspec/CONSTITUTION.md` workspace-local workflow state law.

## Out of Scope

- Changing server-mode remote sync-state handling for GitHub network/DNS failures.
- Changing acceptance/rejection gating semantics.
- Reworking the scheduler or manual resolve flow covered by `fix-manual-resolve-starts-scheduler`.
- Implementing automatic recovery of deleted worktrees beyond preventing noisy stale refresh loops.
