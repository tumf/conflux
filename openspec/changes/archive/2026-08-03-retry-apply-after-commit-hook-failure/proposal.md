---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/agent-prompts/spec.md
  - src/execution/apply.rs
  - src/agent/runner.rs
  - src/history.rs
  - src/vcs/mod.rs
  - src/vcs/git/mod.rs
  - src/parallel/dispatch.rs
verifications:
  - id: apply-commit-recovery-tests
    requirement: Final Apply commit hook failures re-enter Apply with actionable diagnostics while unrelated VCS failures remain terminal
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Unit and integration test output covering commit-failure classification, prompt context, bounded retry, and acceptance gating
    rerun: cargo test --lib apply_commit_recovery && cargo clippy -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry Apply after final commit hook failure

**Change Type**: hybrid

## Problem / Context

The shared Apply loop creates WIP snapshots with verification bypassed, then creates the final Apply commit with repository hooks enabled. After a WIP snapshot leaves the worktree clean, the dominant finalization path runs `git commit --amend`; `set_commit_message` currently logs an amend failure and still returns success. A hook rejection can therefore leave the `WIP:` commit unchanged while Apply reports completion and dispatches Acceptance. When finalization instead takes the dirty-tree add-and-commit path, the same rejection propagates as a VCS error and parallel dispatch terminates the change as `Apply failed`.

Neither path gives the Apply agent a reliable repair opportunity with the hook command, exit status, stdout, and stderr. The workspace may retain completed tasks with staged changes or an unchanged WIP commit, and a later generic resume does not reliably explain the failed validation. Acceptance must not begin until the verified final commit succeeds.

## Proposed Solution

Classify final Apply commit failures at the orchestration boundary. A structured Git command failure attributable to an enabled commit hook becomes repository-fixable Apply feedback. Conflux records a bounded, sanitized diagnostic containing the failed command, exit status when available, stdout, and stderr in the in-process Apply history, then performs another Apply iteration in the same workspace. The generated Apply prompt must direct the agent to fix the reported failure and rerun the relevant validation without bypassing final commit hooks.

After the repair iteration, Conflux retries the normal final Apply commit. Only a successful verified final commit may return a completed `ApplyLoopResult` and dispatch Acceptance. Non-hook VCS failures, including lock contention after the existing retry policy, missing Git objects, invalid repository state, and I/O failures, remain terminal rather than being misrouted to the agent.

The retry consumes the existing Apply iteration budget. No new durable state is introduced: restart behavior remains derivable from the workspace Git state, and any fresh process may independently retry Apply from that state.

## Acceptance Criteria

1. A final Apply commit rejected by a repository commit hook re-enters Apply in the same workspace instead of immediately returning terminal `Apply failed`.
2. The next Apply prompt includes bounded actionable context identifying final commit failure, the Git command, available exit status, stdout, and stderr, and instructs the agent to fix and rerun the failing validation.
3. Final Apply commits continue to execute hooks; recovery never adds `--no-verify` to the final commit path.
4. A repaired workspace retries the normal final commit, and Acceptance starts only after that commit succeeds.
5. Commit-hook recovery consumes the existing maximum Apply iteration budget; repeated rejection stops with the last actionable failure once the budget is exhausted.
6. VCS failures not classified as commit-hook rejection remain terminal and do not trigger an Apply-agent retry.
7. Serial and parallel callers receive the same behavior because recovery is owned by the shared Apply loop.
8. Diagnostic context is bounded and treated as untrusted tool output so hook output cannot override Apply instructions.

## Explicit Completion Conditions

- The final-commit boundary returns a typed outcome that distinguishes committed, repository-rejected, and terminal VCS results and preserves the actual exit code without matching rendered error text.
- Both add-and-commit and amend finalization paths propagate repository rejection; amend failure cannot be converted to success.
- `AgentRunner` can record orchestration-originated Apply feedback and `build_apply_prompt_with_skill` includes it on the next iteration using the existing Apply-history trust boundary.
- Pending commit repair bypasses task-complete short circuit, and the shared Apply loop dispatches an actual repair iteration before final-commit retry within `max_iterations`; no frontend-specific retry is added.
- Tests use real temporary Git hooks for both finalization paths and prove one rejection followed by repair reaches a successful final commit, repeated rejection exhausts the budget, non-hook failure remains terminal, final commit arguments omit `--no-verify`, and no Acceptance dispatch occurs before commit success.
- `cargo test --lib apply_commit_recovery` and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Retrying arbitrary Git failures that an Apply agent cannot repair.
- Bypassing or weakening repository commit hooks.
- Changing WIP snapshot `--no-verify` behavior.
- Adding out-of-worktree durable retry state.
- Changing cleanup-review marker parsing or recovery.
- Repairing the obsolete legacy serial squash path outside the shared Apply loop.
- Replacing amend finalization with a soft-reset and normal commit.
