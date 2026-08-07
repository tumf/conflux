---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/tui-worktree-merge/spec.md
  - openspec/specs/observability/spec.md
  - src/tui/worktrees.rs
  - src/tui/runner.rs
  - src/worktree_ops.rs
  - src/worktree_ops/git_backend.rs
  - src/web/mod.rs
  - src/web/state.rs
  - src/vcs/git/commands/merge.rs
verifications:
  - id: stale-worktree-refresh-regression
    requirement: Worktree refresh avoids repeated merge simulation for non-active or unchanged worktrees while preserving correct active-worktree status
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: Rust tests prove filtering across TUI and Web/UDS periodic paths, shared revision-keyed reuse, active revision invalidation, operator-targeted reinspection, truthful not-inspected diagnostics, bounded diagnostics, and unchanged worktree state
    rerun: cargo test --locked --test e2e_git_worktree_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Harden Stale Worktree Refresh

**Change Type**: implementation

## Problem / Context

Conflux currently discovers every Git-registered worktree and runs ahead/conflict inspection during periodic Worktrees refresh. A worktree whose change is no longer active can remain registered with a branch far behind the base branch. When that branch has a large conflict surface, each refresh repeats `git merge-tree`, and the unparsed conflict fallback writes the complete command output to persistent debug logs.

This caused one stale Corvus worktree to monopolize refresh work, repeatedly generate the same large conflict report, grow one daily log beyond 2.6 million lines, and obscure the active change. The stale worktree was valid Git state and contained uncommitted work, so automatic destructive cleanup is not acceptable.

## Proposed Solution

Keep all registered worktrees visible and manually manageable, but classify conflict-inspection eligibility from current repository evidence. Both periodic TUI refresh and periodic Web/UDS refresh apply the same policy: only the main worktree and worktrees whose branch maps to a currently active or rejected OpenSpec change participate in automatic conflict simulation. Unrelated, completed, archived, or otherwise non-active worktrees remain listed with an explicit not-inspected state and never influence scheduler or workflow decisions. Operator-initiated merge and deletion perform a fresh targeted observation before eligibility is decided, including for branches that do not map to an OpenSpec change.

Cache conflict/ahead observations in one process-local observation layer shared by TUI and Web/UDS refresh, keyed by the repository-verifiable tuple of base HEAD, worktree HEAD, merge base, and branch identity. A refresh with the same tuple reuses the observation; any tuple change invalidates it and reruns inspection. The cache is non-authoritative and disposable, consistent with the workspace-local workflow-state constitution.

Bound `merge-tree` diagnostics. Conflict results retain the conflict count and a small deterministic file sample; parser fallback and command failure logs report byte counts plus bounded prefixes rather than complete stdout/stderr.

## Acceptance Criteria

- A registered worktree not associated with a current active or rejected change remains visible in Worktrees view but periodic refresh does not execute `merge-tree` for it.
- A current change worktree is still checked for commits ahead and merge conflict eligibility.
- Repeated refresh with unchanged base HEAD, worktree HEAD, merge base, and branch identity performs no duplicate merge simulation.
- Changing base HEAD, worktree HEAD, merge base, or branch identity invalidates the cached observation and performs a fresh check.
- Skipped or cached observations are transient presentation metadata only and do not affect dispatch, resume, acceptance, archive, merge execution, deletion execution, or next-action selection; merge affordances may use a current-keyed cached observation, but operator execution revalidates repository state.
- Conflict and command-failure diagnostics are bounded independently of raw `merge-tree` output size while retaining exit status, output byte counts, worktree identity, conflict count, and a deterministic file sample.
- Refresh inspection never modifies a worktree, its index, or uncommitted files.

## Explicit Completion Conditions

- Both TUI and Web/UDS periodic refresh receive the current active/rejected change identities and decide inspection eligibility before spawning ahead/conflict Git commands.
- Worktree rows distinguish checked, cached, and not-inspected observations without treating not-inspected as conflict-free and mergeable or reporting that an uninspected branch has no commits ahead.
- Operator-initiated merge and deletion perform a fresh targeted observation before eligibility is decided, including for worktrees that periodic refresh skips.
- One shared in-memory observation cache serves both periodic refresh paths, uses only current Git-derived identity/revision inputs, is discarded at process exit, and cannot become workflow-control state.
- `check_merge_conflicts` returns structured bounded conflict evidence and never interpolates complete unbounded stdout/stderr into tracing records or operator errors.
- `tests/e2e_git_worktree_tests.rs` uses an injected command recorder or PATH-scoped Git shim to count actual ahead/conflict invocations across both periodic paths and proves stale worktrees are skipped, unchanged active worktrees are reused once process-wide, revision changes rerun checks, operator actions revalidate skipped worktrees, large outputs remain bounded, and tracked/dirty files remain unchanged.
- `cargo test --locked --test e2e_git_worktree_tests` passes without network access or external credentials.

## Out of Scope

- Automatically deleting, pruning, stashing, committing, or rebasing stale worktrees.
- Hiding stale worktrees from Worktrees view.
- Persisting the refresh cache outside the cflx process.
- Changing scheduler dependency analysis or lifecycle state classification.
- Adding a separate background cleanup daemon.

Repository-wide Rust formatting and clippy remain owned by the existing path-scoped `.pre-commit-config.yaml` hooks when Rust files are staged; the requirement-specific regression command above remains explicit.
