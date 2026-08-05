---
change_type: hybrid
priority: high
dependencies: []
references:
  - skills/cflx-proposal/SKILL.md
  - skills/cflx-apply/SKILL.md
  - src/execution/apply.rs
  - src/vcs/git/commands/commit.rs
  - src/execution/final_commit_lock_retry.rs
  - src/parallel/output_bridge.rs
verifications:
  - id: apply-finalization-tests
    requirement: Apply completion, retry feedback, commit phase projection, and streamed commit diagnostics remain repository-verifiable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: change-implementation
    automation: scripts/test-time-top10.sh
    evidence: Rust unit and integration test output for apply finalization, VCS streaming, reducer projection, and TUI rendering
    rerun: cargo test --lib execution::apply vcs::git orchestration::state parallel::output_bridge tui
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Harden Apply Finalization

**Change Type**: hybrid

## Premise / Context

- Apply agents can start a long repository-wide test in the background, return before it completes, and leave Conflux to terminate the child during process-group cleanup.
- A later Apply iteration then sees unchanged task progress and repeats the same verification without knowing that the prior iteration ended with an unfinished background command.
- Conflux cannot safely infer whether an arbitrary child process is a verification command, development server, or abandoned process.
- Final Apply commits already run repository hooks, but WIP snapshots stage the whole workspace before the hook-enabled amend path.
- Operators need the finalization phase and repository-hook output to be visible without turning commit progress into durable workflow-control state.

## Problem / Context

Proposal tasks can duplicate repository-wide checks already owned by unconditional tracked pre-commit hooks. This encourages Apply agents to run long validation commands themselves and creates a retry loop when they return before those commands finish.

The current finalization contract also does not require the agent to make its intended file selection explicit before Conflux creates a WIP snapshot with `git add -A`. That can conceal unstaged or untracked files before final commit review. Removing `git add -A` from WIP snapshots would weaken crash recovery and stall detection, so finalization needs a clean pre-snapshot gate rather than a replacement snapshot model.

Finally, hook-enabled commit output is captured only after the process exits. During a long pre-commit run the TUI continues to show `[apply]`, and successful hook progress is not streamed into normal operator logs.

## Proposed Solution

1. Update bundled proposal guidance so repository-wide format, lint, test, and generated-artifact checks are not independent checkbox tasks when a tracked, unconditional pre-commit hook already executes them. Requirement-specific tests remain attached to implementation tasks; heavy and E2E checks remain explicitly owned outside pre-commit.
2. Update Apply guidance so the agent stages only change-owned files, does not commit, leaves no unstaged or untracked entries before completion, and never returns while a background verification command remains active.
3. Add an Apply completion gate after process-group quiescence and before the final WIP snapshot. If a task-complete workspace has unstaged or untracked entries, record bounded `incomplete_stage` feedback, preserve work with the existing WIP snapshot, and return to Apply instead of final commit.
4. Preserve existing WIP `git add -A`, final verified-commit, and index-lock recovery semantics. A successful completion gate makes the normal WIP staging operation an expected no-op; it is retained as the crash-recovery boundary.
5. Record structured `empty_apply_iteration` feedback when an eligible successful Apply iteration produces neither task progress nor workspace progress. Reuse the existing Apply history output tail rather than duplicating it.
6. Expose an ephemeral commit subphase while the Apply finalization gate, verified commit, repository hooks, and lock retries run. TUI renders `[commit]`; the public lifecycle remains `applying`, and restart routing never depends on the subphase.
7. Stream final `git commit` stdout and stderr through a tee that both emits line-level operator events/logs and preserves complete raw streams plus the exit code for existing rejection and index-lock classification.
8. After a successful commit, verify that hooks did not leave a dirty workspace. Route any hook-created unstaged or untracked content back through bounded Apply repair feedback before Acceptance.

## Acceptance Criteria

- A task-complete Apply iteration cannot reach final commit while `git status --porcelain` reports unstaged or untracked entries.
- Stage-gate diagnostics identify a bounded set of affected paths and return to the same workspace for repair without bypassing existing iteration or stall limits.
- WIP snapshots continue to preserve all workspace work and continue to support existing empty-WIP stall semantics and crash recovery.
- An eligible empty Apply iteration adds structured guidance telling the next agent to inspect unfinished tasks and prior output and not to return with background verification still active.
- Proposal guidance delegates a repository-wide gate only when tracked hook configuration proves that the gate runs unconditionally, including on amend; staged-file-only hooks do not qualify.
- Heavy and E2E verification is not moved into pre-commit solely by this change.
- During finalization, the TUI changes from `[apply]` to `[commit]`, returns to `[apply]` for a repair iteration, and clears commit presentation after completion or failure.
- Commit subphase state is process-local presentation state and does not change the canonical `applying` lifecycle or resume routing.
- Final commit stdout and stderr are emitted line by line to TUI and persistent logs with change ID, stream, and attempt context.
- Streamed output preserves the complete raw stdout, stderr, exit status, and command needed by existing hook-rejection and index-lock classification.
- Hook rejection retains full output in persistent logs while only bounded diagnostics enter the next Apply prompt.
- A hook that exits successfully but leaves workspace changes cannot dispatch Acceptance until those changes are repaired and finalized.

## Explicit Completion Conditions

- `skills/cflx-proposal/SKILL.md` and `skills/cflx-apply/SKILL.md` express the hook-ownership, explicit-stage, no-agent-commit, clean-gate, and foreground-verification rules.
- `src/execution/apply.rs` enforces task-complete stage cleanliness before WIP finalization, records bounded stage and empty-iteration feedback, and checks post-commit cleanliness.
- Existing WIP snapshot and final commit lock-retry behavior remains intact and its current tests continue to pass.
- Unified execution events and reducer state represent commit presentation without introducing durable workflow-control state.
- TUI rendering and output bridge tests prove `[apply]` / `[commit]` transitions and repair reset behavior.
- VCS command tests prove streamed tee output and captured classification data are identical in meaning to the existing captured command result.
- Repository-local verification declared as `apply-finalization-tests` passes.

## Scope Coupling

The guidance, completion gate, retry feedback, finalization presentation, and commit streaming ship together because they define one Apply-to-commit ownership boundary. Splitting them would either enforce agent staging without actionable observability or expose commit progress while retaining the verification loop that motivated the change.

## Out of Scope

- Detecting or classifying arbitrary agent child processes as tests, servers, or other workloads.
- Removing `git add -A` from WIP snapshots or redesigning WIP crash recovery around the Git index.
- Adding a durable `Committing` lifecycle state or using commit presentation for scheduler/resume decisions.
- Automatically adding heavy, networked, credentialed, or E2E checks to repository hooks.
- Changing repository hook bypass behavior for WIP snapshots; final commits remain hook-enabled.
