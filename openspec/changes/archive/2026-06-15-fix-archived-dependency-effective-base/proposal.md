---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-06-04-fix-dependency-dispatch-after-merge
  - src/parallel/queue_state.rs
  - src/execution/state.rs
  - src/vcs/git/mod.rs
  - src/parallel/tests/executor.rs
---

# Fix archived dependency effective base

**Change Type**: implementation

## Problem / Context

A previous fix correctly prevented dependent changes from dispatching when a dependency had only been archived but not merged. That fix made archive evidence a classification signal, not a dispatch-satisfaction signal, and required base-branch merge evidence before unblocking dependents.

A newer stacked orchestration case shows the opposite failure mode: a dependency can be merged into the branch/context where Conflux is accumulating completed changes, while the scheduler still checks the original branch captured at startup. In that case `cflx openspec show` and analysis can report the dependency as archived/done, but dispatch remains blocked because `is_dependency_resolved()` verifies `original_branch` instead of the effective integration base.

Observed symptoms from `review-gauntlet`:

- `clarify-run-finalize-timeout` depends on `remove-timeout-from-run-tui-header`.
- The analyzer logs `Accepted archived dependency target as already satisfied`.
- Dispatch logs `Archived dependency evidence found; verifying base-branch merge before dispatch`.
- `is_merged_to_base` checks `base_branch=main` and reports `archive_entry_exists=false`, `change_dir_exists=true`.
- The current orchestration branch contains merge/archive commits for the dependency, but the dependent remains blocked.

This appears to be a side effect of the earlier `fix-dependency-dispatch-after-merge` behavior: the no-pre-merge-dispatch guard is still required, but the merge evidence must be evaluated against the scheduler's effective integration base rather than always against the original startup branch.

## Proposed Solution

Define and use an effective dependency base for archived dependency dispatch checks.

The scheduler SHALL continue to block dependents when an archived dependency has not been merged into the effective dependency base. The effective dependency base SHALL be the repository-visible branch or tree context that Conflux is actually using as the accumulated integration result for dispatch decisions. It MUST NOT silently fall back to archive evidence alone.

For ordinary runs where the original branch is the integration base, behavior remains unchanged. For stacked orchestration runs where Conflux has advanced an integration branch with dependency merge/archive commits, archived dependency merge checks SHALL use that effective integration base so dependents unblock after the dependency is actually merged into the current orchestration context.

The dependency status shown by analysis/diagnostics and the dispatch gate SHALL use consistent terminology so an archived dependency is not simultaneously reported as `done` and retained as an opaque `Archived` blocker without explaining the missing merge evidence.

## Acceptance Criteria

- A queued change whose dependency is archived but not merged into the effective dependency base remains dependency-blocked.
- A queued change whose dependency is archived and merged into the effective dependency base becomes dispatchable when no other blockers remain, even if the original startup branch has not yet advanced.
- The previous regression guard is preserved: archive evidence alone MUST NOT satisfy dependent dispatch.
- Dependency-blocked diagnostics distinguish archived-but-not-merged from missing/rejected/queued/in-flight blockers and identify the effective base used for the merge check when useful for operators.
- Analyzer, queue dispatch, and user-visible dependency status do not present contradictory states such as `done` while dispatch remains blocked solely because a different base was checked.
- Workflow-control decisions remain derivable from workspace file state, workspace git state, and base-branch/tree comparison, without introducing out-of-worktree durable state.

## Explicit Completion Conditions

This change is complete when:

- `src/parallel/queue_state.rs` resolves archived dependency merge evidence through an explicit effective dependency base instead of always using the original startup branch.
- `src/execution/state.rs` or the caller path preserves the archive-only guard while allowing merge checks against the selected effective dependency base.
- `src/vcs/git/mod.rs` / workspace manager integration exposes enough repository-visible branch/context information for the scheduler to choose the effective dependency base deterministically.
- `src/parallel/tests/executor.rs` covers both: archived-but-not-merged dependency remains blocked, and archived dependency merged into the effective integration base unblocks the dependent even when the original branch lacks that merge.
- Existing dependency dispatch tests from `fix-dependency-dispatch-after-merge` continue to pass.
- Focused Rust tests for dependency dispatch pass.
- `cargo test` or the repository's documented focused check set passes, or any unrelated failure is documented with evidence.

## Out of Scope

- Treating archive evidence alone as dispatch satisfaction.
- Removing the earlier no-pre-merge-dispatch safety rule.
- Introducing durable workflow-control state outside the workspace or git/base tree evidence.
- Changing proposal metadata dependency syntax.
- Reworking unrelated dependency classes such as rejected, missing, queued, in-flight, active-but-not-queued, or terminal-error handling beyond consistent diagnostics.
