---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel/executor.rs
  - src/parallel/tests/executor.rs
  - src/parallel/dispatch.rs
  - src/parallel/acceptance_state.rs
  - openspec/CONSTITUTION.md
---

# Proposal: Remove acceptance report artifact

**Change Type**: implementation

## Premise / Context

- The current acceptance executor writes `ACCEPTANCE_REPORT.json` directly into the workspace root from `src/parallel/executor.rs`.
- The file is an untracked side-effect of acceptance execution and can dirty the worktree even though acceptance itself checks for a clean working tree.
- The current resume router in `src/parallel/dispatch.rs` routes `WorkspaceState::Applied` back to acceptance and does not use this file as an authoritative archive handoff input.
- `openspec/CONSTITUTION.md` limits authoritative workflow-control inputs to workspace file state, workspace git state, and base-branch tree comparison, and forbids hidden runtime state as completion evidence.
- The desired artifact is implementation: remove the runtime side-effect and lock the behavior with tests.

## Problem / Context

Acceptance now creates a workspace-root `ACCEPTANCE_REPORT.json` artifact. This artifact is not a user-authored OpenSpec artifact, is not tracked by git, and is not required by the current resume routing path. Because acceptance requires a clean working tree, creating a new untracked file in the workspace introduces noise and can make subsequent checks or operator inspection misleading.

The current helper also always serializes `"result": "pass"`, including at least one non-pass command-failure path. Keeping this artifact risks confusing operators and future agents into treating a local report file as authoritative acceptance evidence.

## Proposed Solution

Remove workspace-root acceptance report generation from parallel acceptance execution. Acceptance attempts should continue to be recorded through the existing runtime history structures and emitted events, but no `ACCEPTANCE_REPORT.json` file should be written into the target workspace.

The implementation should update tests that currently expect the artifact to exist so they instead assert that acceptance finalization records pass/failure state without creating the file.

## Acceptance Criteria

- Parallel acceptance execution does not create `ACCEPTANCE_REPORT.json` in the workspace root for PASS, command failure, FAIL, CONTINUE, or stalled-hold verdicts.
- Acceptance PASS still records an acceptance attempt with the final revision and output tails in existing acceptance history.
- Command-failure acceptance no longer writes a misleading report with `"result": "pass"`.
- Resume and archive routing do not depend on `ACCEPTANCE_REPORT.json` for workflow-control decisions.
- Regression tests fail if any acceptance branch recreates the workspace-root report artifact.

## Explicit Completion Conditions

- `src/parallel/executor.rs` no longer contains a helper or call path that writes `workspace_path.join("ACCEPTANCE_REPORT.json")`.
- `src/parallel/tests/executor.rs` has focused regression coverage asserting that successful verdict-finalized acceptance and command-failure acceptance do not create `ACCEPTANCE_REPORT.json`.
- Existing acceptance history assertions continue to prove acceptance results are captured without relying on a worktree-root JSON file.
- Repository validation includes focused Rust tests for the touched executor behavior and the standard OpenSpec proposal validation commands.

## Out of Scope

- Changing the JSON-primary acceptance verdict contract emitted by agents.
- Redesigning resume routing or archive handoff semantics.
- Introducing a replacement durable acceptance-state file in another location.
- Adding `.gitignore` entries to hide the artifact instead of removing its creation.
