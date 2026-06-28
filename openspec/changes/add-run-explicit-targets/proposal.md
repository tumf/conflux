---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/main.rs
  - src/orchestrator.rs
  - skills/cflx-run/SKILL.md
  - skills/cflx-run/references/cflx-run.md
  - skills/README.md
---

# Add explicit targets for `cflx run`

**Change Type**: implementation

## Premise / Context

- `cflx run` currently defaults to processing all active OpenSpec changes when no target is supplied.
- The user wants run-mode targets to mirror TUI execution marks: explicit change IDs behave like selected rows, and `--all` behaves like the TUI `x` bulk mark.
- Existing compatibility for `--change a,b` should remain, but new positional IDs should be the preferred direct syntax.
- Parallel, dry-run, and web-monitoring run modes must use the same normalized target set.
- Bundled `skills/cflx-run` operator guidance is embedded/distributed by Conflux and must be updated together with CLI behavior.
- `openspec/CONSTITUTION.md` requires repository-verifiable evidence and forbids hidden external workflow-control state.

## Problem / Context

Run mode currently has an implicit and potentially surprising target: `cflx run` without arguments processes every discovered change. That differs from the TUI model, where execution requires a visible selection mark and `x` is the explicit all-target action. The command also lacks a compact positional form such as `cflx run id1 id2`, forcing operators to use `--change` for common targeted runs.

The CLI, orchestration filtering, dry-run planning, and bundled `cflx-run` skill documentation need one coherent target contract so agents and humans do not accidentally run all changes.

## Proposed Solution

- Add positional change IDs to `cflx run`: `cflx run id1 id2 ...`.
- Add `cflx run --all` as the explicit all-current-changes target mode.
- Require exactly one target mode for `cflx run`: `--all`, positional IDs, or existing `--change`.
- Keep `--change a,b` for backward compatibility, but treat it as the same normalized target list as positional IDs.
- Reject duplicate and unknown requested change IDs before starting orchestration or partial execution.
- Apply normalized targets consistently to serial execution, parallel execution, parallel dry-run planning, and web-enabled run mode.
- Update CLI help/examples and bundled `skills/cflx-run` documentation to avoid recommending bare `cflx run`.

## Acceptance Criteria

- `cflx run id1 id2` runs only `id1` and `id2`, in the same sense as starting the TUI with those changes selected.
- `cflx run --all` runs all current eligible changes, in the same sense as pressing `x` in the TUI before starting.
- `cflx run` without `--all`, positional IDs, or `--change` fails before orchestration starts and tells the operator to choose an explicit target mode.
- `--all`, positional IDs, and `--change` are mutually exclusive.
- Duplicate requested IDs fail before orchestration starts.
- Unknown requested IDs fail before orchestration starts; no valid subset is executed.
- Parallel mode, dry-run mode, and web-enabled mode honor the same normalized target set.
- Bundled `cflx-run` skill docs and skill README describe the explicit target contract and use explicit target examples.

## Explicit Completion Conditions

- `src/cli.rs` parses `cflx run id1 id2`, `cflx run --all`, and existing `cflx run --change a,b`, and has parser tests for valid and invalid target-mode combinations.
- The run command path in `src/main.rs` normalizes targets once and passes the normalized target mode into `Orchestrator::new`.
- `src/orchestrator.rs` validates requested IDs against the start snapshot and fails atomically for unknown IDs instead of warning and continuing with a subset.
- Parallel execution and parallel dry-run use the same filtered snapshot as serial execution.
- CLI help text and examples no longer present bare `cflx run` as a valid non-interactive execution command.
- `skills/cflx-run/SKILL.md`, `skills/cflx-run/references/cflx-run.md`, and `skills/README.md` reflect the new explicit target requirement.
- Tests or scripted checks demonstrate success, error, and dry-run target behavior without relying on hidden runtime state.

## Out of Scope

- Changing TUI selection key behavior itself.
- Removing the legacy `--change` option.
- Changing dependency analysis, archive routing, acceptance verdict parsing, or worktree cleanup semantics.
- Adding new durable workflow-control state outside the repository/worktree.
