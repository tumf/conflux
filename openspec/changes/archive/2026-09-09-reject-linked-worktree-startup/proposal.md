---
change_type: implementation
priority: high
dependencies: []
references:
  - src/main.rs
  - src/repo_lock.rs
  - tests/run_exit_tests.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-editor/spec.md
  - openspec/specs/web-monitoring/spec.md
verifications:
  - id: linked-worktree-startup-tests
    requirement: Local orchestration owners refuse startup from a linked Git worktree before acquiring the repository lock or producing orchestration side effects
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests
    rerun: cargo test --test run_exit_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Reject local orchestration startup from linked worktrees

**Change Type**: implementation

## Problem / Context

Conflux currently derives repository locking and the owner socket from the Git common directory, so a local TUI or `cflx run` started inside any linked worktree can become the repository owner. The owner then treats that linked worktree as its base workspace and may create managed worktrees for changes found there.

This permits nested orchestration from a feature/proposal worktree. If its managed worktree later disappears while the shared Git registration remains, Conflux repeatedly probes a non-existent path and reports `Failed to verify base branch: No such file or directory`.

The permanent boundary is simpler: local orchestration owners must start only from the repository's main worktree. Read-only and client commands may continue to resolve linked worktrees.

## Proposed Solution

Add a shared startup preflight for orchestration-owning invocations: bare `cflx`, `cflx tui`, and `cflx run`.

The preflight shall derive the current Git directory and Git common directory using Git repository evidence. When their canonical identities differ, the current workspace is a linked worktree and startup shall fail closed with an actionable diagnostic that:

- identifies the current linked-worktree path;
- identifies the main worktree path derived from `git worktree list --porcelain`;
- tells the operator to start Conflux from the main worktree;
- does not remove, prune, or modify either worktree.

Run repository/Git availability checks and then this classification before repository-lock acquisition, logging initialization, listener binding, lifecycle adapters, AI subprocesses, hooks, or workspace mutation. Moving the existing Git refusal ahead of logging and listener setup is an intentional strengthening of the startup boundary. Keep `cflx client`, OpenSpec inspection/validation, logs, completion, and other non-owner commands usable from linked worktrees.

A repository using a separate Git directory is not a linked worktree when its resolved Git directory equals its Git common directory; it remains eligible.

## Acceptance Criteria

- Bare `cflx`, `cflx tui`, and `cflx run` exit non-zero when launched from a linked worktree.
- The refusal names the linked worktree and main worktree and instructs the user to start from the main worktree.
- Refusal occurs before repository-lock files, owner metadata, API sockets, logs, hooks, lifecycle adapters, AI subprocesses, or managed worktrees are created or changed.
- The same owner commands continue to start from the main worktree.
- Non-owner commands, especially `cflx client`, remain callable from a linked worktree and retain existing routing semantics.
- Separate-Git-directory repositories are not falsely rejected solely because `.git` is a pointer file.

## Explicit Completion Conditions

- A single shared classifier/preflight owns linked-worktree detection for every local orchestration-owning entrypoint.
- Integration fixtures create a real repository plus linked worktree and prove refusal for bare TUI, explicit TUI, and run entrypoints.
- Regression coverage proves main-worktree startup eligibility, non-owner bypass, separate-Git-directory eligibility, actionable diagnostics, and absence of startup side effects.
- Existing lock/socket regressions are rewritten to preserve common-directory identity coverage without starting an owner from a linked worktree; the full `run_exit_tests` target passes.
- `cargo test --test run_exit_tests linked_worktree_startup` passes.
- `cflx openspec validate reject-linked-worktree-startup --archive-gate` passes before archive.

## Out of Scope

- Automatically pruning or repairing stale worktree registrations.
- Redirecting an invocation to the main worktree.
- Changing client/MCP project routing from linked worktrees.
- Changing repository lock identity or owner socket placement.
- Repairing the already-stale `file-drag-folder-move` registration in another repository.

## Retired Scenarios

- cli: Repository-Scoped Orchestration Lock / Linked worktrees share one lock

That scenario asserted that starting local orchestration from a linked worktree is rejected *as a repository lock conflict*. This change rejects it earlier, by the main-worktree preflight, so the outcome it described can no longer occur. `Linked worktree is rejected before lock contention` replaces it and keeps the shared-common-directory identity under test.

- cli: Web Monitoring Flags / Non-Git invocation requires a decision
- web-monitoring: Configuration Options / Non-Git default path is unavailable

Both scenarios required an owner entrypoint outside Git to fail with a *socket path-selection* error naming `--web-unix-socket` and `--no-web-unix-socket`. Moving the Git refusal into the shared owner startup preflight puts it ahead of listener setup, so socket resolution is never reached from an owner entrypoint and neither outcome can occur.

Their outcomes are retired in place: a MODIFIED requirement merges into the canonical block rather than replacing it, so dropping a scenario header is not expressible in a delta. Each header is therefore kept and its body rewritten to what is now decidable — the missing-repository refusal precedes socket selection whatever the socket options say, and no path-selection guidance is offered. The resolver's own path-selection message survives in `src/web/unix_socket.rs` for callers that resolve a path without that preflight, and stays covered by that module's unit tests. `tests/run_exit_tests.rs` proves the retirement with a negative assertion rather than a comment.
