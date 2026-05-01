---
change_type: implementation
priority: high
dependencies: []
references:
  - https://github.com/tumf/conflux/issues/6
  - openspec/CONSTITUTION.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - .wt/AGENTS.md
  - src/vcs/git/commands/worktree.rs
  - src/vcs/git/mod.rs
  - src/server/api/worktrees.rs
  - src/tui/command_handlers.rs
  - src/orchestration/rejection.rs
---

# Change: Add worktree teardown hook before managed worktree deletion

**Change Type**: implementation

## Problem / Context

Conflux can create managed Git worktrees and run repository-local `.wt/setup` inside the new worktree with `ROOT_WORKTREE_PATH` pointing at the base repository. Some repositories use that setup hook to allocate per-worktree resources such as Docker Compose projects, volumes, databases, Redis instances, generated env files, or ports.

There is currently no symmetric cleanup point before Conflux removes a managed worktree. Existing deletion paths call Git worktree removal from multiple places, so resources created by `.wt/setup` can leak after rejection, dependency-resolved recreation, merge cleanup, proposal session close, or manual TUI/WebUI deletion.

The project constitution requires workflow state to be derivable from workspace-local file/git state and truthful completion to be backed by repository-verifiable evidence. This change treats `.wt/teardown` as an optional worktree-local side-effect cleanup hook, not as an authoritative workflow-control input.

## Proposed Solution

Add repository-defined worktree teardown support to Conflux's managed worktree deletion lifecycle.

- Detect `.wt/teardown` inside the worktree being deleted.
- Run it before `git worktree remove` when it exists and is executable.
- Execute it non-interactively from the worktree root.
- Pass `ROOT_WORKTREE_PATH` with the same meaning used by `.wt/setup`.
- Abort deletion by default when teardown fails, preserving the worktree for operator recovery.
- Provide an explicit `skip_teardown` / `--skip-teardown` escape hatch that warns and proceeds with deletion after teardown failure or without running teardown.
- Log teardown exit status, stdout, stderr, worktree path, and root path with enough context for debugging.
- Route Conflux-managed deletion paths through shared teardown-aware removal behavior rather than leaving direct `worktree_remove` call sites inconsistent.
- Document `.wt/teardown` alongside `.wt/setup`, including `.wt/state.env` as a worktree-local convention for per-worktree cleanup metadata.

## Acceptance Criteria

- Repositories can define executable worktree-local `.wt/teardown`, and Conflux executes it before deleting managed worktrees.
- The hook runs with the worktree root as cwd and receives `ROOT_WORKTREE_PATH` pointing at the base repository.
- `.wt/teardown` can read `.wt/state.env` using worktree-relative paths.
- Teardown failure prevents deletion by default and reports diagnostic stdout/stderr/exit status.
- Operators have an explicit skip/escape option that allows deletion to proceed after warning.
- Major Conflux-managed worktree deletion paths use the teardown-aware removal behavior consistently.
- Documentation describes `.wt/teardown`, `.wt/state.env`, failure behavior, and the skip option.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/vcs/git/commands/worktree.rs` or an equivalent shared VCS module exposes teardown execution and teardown-aware worktree removal behavior with options for default abort and explicit skip.
- Parallel cleanup, stale/inconsistent worktree cleanup, rejection cleanup, TUI deletion, server/WebUI deletion, legacy web deletion, and proposal-session deletion paths either call the shared teardown-aware behavior or explicitly justify why they are outside managed worktree deletion scope.
- Unit or integration tests prove teardown success runs before removal, teardown failure preserves the worktree by default, skip teardown proceeds with a warning, non-executable teardown is not run, cwd/env are correct, and `.wt/state.env` can be consumed by a teardown script.
- API/TUI/WebUI or CLI-facing escape hatch behavior is covered by tests or explicit manual verification.
- `cflx openspec validate add-worktree-teardown-hook --strict --evidence warn` passes without unresolved evidence warnings.
- Rust and dashboard lint/typecheck/test commands relevant to touched code pass, or any intentionally skipped heavy tests are documented.

## Out of Scope

- Standardizing the contents or schema of `.wt/state.env` beyond documenting it as a convention.
- Changing `.wt/setup` execution semantics or its current permission behavior.
- Adding long-running external Docker/Postgres/Redis integration tests that require real services or credentials.
- Using teardown output or `.wt/state.env` as authoritative workflow-control state for resume, acceptance, archive, or next-action routing.
