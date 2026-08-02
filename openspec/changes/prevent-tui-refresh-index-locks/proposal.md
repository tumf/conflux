---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/hooks/spec.md
  - src/tui/runner.rs
  - src/tui/orchestrator.rs
  - src/parallel_run_service.rs
  - src/vcs/git/commands/commit.rs
  - src/vcs/commands.rs
verifications:
  - id: refresh-status-lock-tests
    requirement: "Repository monitoring detects committed, staged, unstaged, and untracked changes without taking an optional Git index lock that can collide with repo-mutating hooks"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git integration output proving refresh status classification remains correct and the root index is not refreshed or locked by the monitoring query"
    rerun: "cargo test vcs::git::commands::commit::tests"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent TUI refresh status from contending on the Git index

**Change Type**: implementation

## Problem / Context

The TUI periodically calls `list_changes_with_uncommitted_files` while an `on_merged` hook may mutate and commit the root repository. The monitoring query currently executes `git status --porcelain -u` with Git's default optional-lock behavior.

A production run showed the hook's root `.git/index.lock` preflight reporting no lock, followed roughly three seconds later by `git add -A` failing because the TUI refresh had started `git status --porcelain -u`. Git status may refresh stat information and briefly take `index.lock`, so a read-oriented monitoring poll can invalidate the earlier hook preflight and block a release hook after the merge and push already succeeded.

The repository paths in the preflight and failure message resolve to the same repository; path aliases were not separate lock domains. Process-local merge guards cannot prevent this race because the refresh runs independently and other `cflx` processes may monitor the same repository.

## Proposed Solution

Run the repository-monitoring status query with Git optional locks disabled while preserving its porcelain and untracked-file semantics. Keep this narrow: only the polling/query path that classifies uncommitted OpenSpec changes should opt out of index refresh writes. Repo-mutating commands and commands whose correctness requires an index update remain unchanged.

Add temporary-Git regression coverage that proves staged, unstaged, and untracked OpenSpec changes are still classified correctly. Add lock-safety coverage that makes the index stale, runs the monitoring query, and verifies the query does not replace or update the index while still returning current working-tree state.

This proposal is independent of release commit scoping. It prevents the observed read/write lock collision, while `scope-release-bump-commit` separately prevents unrelated files from entering a release commit.

## Acceptance Criteria

1. Periodic TUI and parallel-start refreshes detect uncommitted OpenSpec changes without allowing the status query to acquire an optional root index lock.
2. Staged, unstaged, renamed, and untracked files under `openspec/changes/<change-id>/` retain their existing classification behavior.
3. Files under `openspec/changes/archive/`, hidden change directories, and unrelated repository paths remain excluded.
4. The monitoring query remains read-only with respect to the Git index even when worktree metadata would otherwise permit Git to refresh index stat data.
5. Repo-mutating Git commands, merge serialization, hook failure semantics, and `on_merged` lock diagnostics are unchanged.
6. No cross-process durable lock or out-of-worktree workflow-control state is introduced.

## Explicit Completion Conditions

- `src/vcs/git/commands/commit.rs` invokes the monitoring status query with Git optional locking disabled while retaining porcelain output with individual untracked files.
- Existing callers in `src/tui/runner.rs`, `src/tui/orchestrator.rs`, and `src/parallel_run_service.rs` continue through the same helper without separate locking logic.
- Temporary-Git tests fail if the query stops detecting staged, unstaged, or untracked changes, or if it refreshes/replaces the repository index.
- `cargo test vcs::git::commands::commit::tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Retrying arbitrary failed Git commands.
- Removing an existing `.git/index.lock`.
- Changing `on_merged` retry counts, timeout policy, or failure-to-state-transition behavior.
- Adding a repository-wide cross-process mutex.
- Changing release bump staging or commit contents; that belongs to `scope-release-bump-commit`.
