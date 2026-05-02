---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/merge.rs
  - src/parallel/conflict.rs
  - src/vcs/git/mod.rs
  - ~/.local/state/cflx/logs/conflux-bda270b8/2026-05-02.log
---

# Fix Conflictless Merge Resolve Retry

**Change Type**: implementation

## Problem / Context

Premise / Context:

- The user asked to create a proposal from the current session context, with no extra `UserRequest` text.
- In this session we investigated the stall around `fix-manual-resolve-starts-scheduler` and confirmed the current running resolver was triggered by manual restart, not by an autonomous recovery path.
- Repository rules require workspace-local workflow state and truthful completion, so the fix must rely on workspace/git/base-tree evidence rather than external runtime memory.
- The previously archived change `fix-manual-resolve-starts-scheduler` addressed scheduler startup and reducer-owned `ResolveWait`, but did not cover the later archive-to-merge handoff bug now observed.
- Current code in `src/parallel/conflict.rs:344-455` constructs and emits a resolve command even when `detect_conflicts()` returns no conflict files, and `src/parallel/conflict.rs:743-829` can still reject a successful merge as missing pre-sync state.

Inferred request:

- Create a new proposal for the bug where a conflictless archived merge is incorrectly routed through `cflx-resolve` and then retried after the merge already succeeded.
- Keep the proposal separate from the already-archived manual-resolve scheduler startup fixes.
- Make the proposal implementation-oriented, strictly valid, and ready to commit.

Observed on `~/.local/state/cflx/logs/conflux-bda270b8/2026-05-02.log` during `fix-manual-resolve-starts-scheduler` merge/resolve handling:

- `src/parallel/merge.rs` entered sequential merge handling after archive completion.
- `git merge --no-ff --no-commit fix-manual-resolve-starts-scheduler` completed without conflicts and staged a normal merge commit candidate.
- Despite `Conflicting files (repo root, if any): (none)`, the runtime still launched `cflx-resolve` via `src/parallel/conflict.rs` with a conflict-oriented prompt.
- The prompt also showed `Worktree directories ... => (unknown)`, weakening the evidence available to the resolve agent.
- The resolve agent completed the ordinary merge and produced commit `dd258d3c Merge change: fix-manual-resolve-starts-scheduler`, but post-command verification in `src/parallel/conflict.rs:743-829` still treated the attempt as failed and retried because `is_ancestor(repo_root, pre_merge_base, revision)` was false for the archived branch tip.

This creates a false-positive conflict resolution workflow: the system asks an AI agent to resolve conflicts that do not exist, then can continue retrying after the merge already succeeded.

## Proposed Solution

Tighten the archive-to-merge handoff so conflictless sequential merges do not enter the AI resolve path, and so post-merge verification accepts a valid normal merge result.

The runtime should distinguish two cases clearly:

1. A true merge-conflict case, where Git reports unresolved conflicts and conflict files exist. Only this case should launch `cflx-resolve` and emit `ResolveStarted`.
2. A conflictless merge case, where `git merge --no-ff --no-commit` or equivalent merge preparation succeeds without unresolved conflicts. This case should complete the merge commit directly, verify the merge result, and skip AI resolve entirely.

The verification logic must also avoid treating the archived branch tip as proof that pre-sync was skipped after a successful merge commit already integrated the change. Post-merge validation should verify the merged outcome from repository-visible merge evidence, not by re-requiring the source branch tip to contain the pre-merge base after the branch has been merged into `main`.

The solution should also ensure resolve prompts and logs stay truthful: no conflict-oriented prompt, no `(none)` conflict list presented as if conflict markers exist, and no `(unknown)` worktree path in the normal conflictless merge path.

## Acceptance Criteria

- When an archived change reaches sequential merge with no actual Git conflicts, Conflux does not start `cflx-resolve` and does not emit a conflict-oriented resolve prompt.
- A conflictless archived merge can complete through the normal merge commit path and be accepted as successful without manual restart.
- Post-merge verification does not retry solely because the archived worktree branch tip does not contain the pre-merge base after a valid merge commit already integrated the change.
- When a true merge conflict exists, Conflux still emits `ResolveStarted`, includes real conflict evidence, and routes through the existing AI resolve path.
- Resolve/merge logs are truthful: no `Conflicting files: (none)` conflict prompt, and no `(unknown)` worktree path in the conflictless merge path.

## Explicit Completion Conditions

- `src/parallel/merge.rs` and/or `src/parallel/conflict.rs` contain a repository-verifiable guard that skips AI resolve startup when conflict detection is empty and the merge is already in a normal merge-ready state.
- `src/parallel/conflict.rs:743-829` or equivalent verification logic is updated so a successful merge commit is not retried because of a false pre-sync negative on the archived source branch.
- Regression tests prove the old behavior fails and the new behavior passes for both conflictless merge and true-conflict merge scenarios.
- At least one targeted test verifies that `ResolveStarted` is not emitted for the conflictless merge path, while another verifies it still appears for a real conflict path.
- The implementation preserves `openspec/CONSTITUTION.md` by deriving decisions from workspace file state, workspace git state, and base-branch tree comparison only.

## Out of Scope

- Reworking the previously archived `fix-manual-resolve-starts-scheduler` proposal itself.
- Changing general scheduler lifetime or reducer ownership rules beyond what is needed for conflictless merge handoff correctness.
- Adding durable recovery state outside repository/worktree evidence.
