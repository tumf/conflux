---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-merge/spec.md
  - src/parallel/conflict.rs
  - src/parallel/tests/conflict.rs
  - src/vcs/git/commands/merge.rs
  - skills/cflx-resolve/SKILL.md
verifications:
  - id: resolve-continuation-tests
    requirement: Sequential resolve retries identify and communicate the exact unfinished merge phase from repository state until the final merge is verifiably complete
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust unit and temporary-Git integration test output for pre-sync, final-merge, resurrection-cleanup, and completed states
    rerun: cargo test parallel::tests::conflict
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix sequential resolve merge continuation

**Change Type**: implementation

## Problem / Context

A sequential resolve can complete the worktree pre-sync but stop before the final merge into the target branch. Conflux currently reduces that state to the generic continuation reason `Missing merge commits for change_ids (...); retrying resolve`. The next resolve attempt receives history, but not an explicit repository-state diagnosis naming the unfinished phase, worktree, required final merge subject, or archive resurrection cleanup. An agent can therefore repeat or merely describe Step 2 until the retry limit is exhausted, after which queue reconciliation falls back to manual merge wait.

The repository state already contains the authoritative evidence needed to diagnose the unfinished protocol: `MERGE_HEAD`, conflict files, branch ancestry, exact merge subjects, live change paths, and valid exact/date-prefixed archive entries. The fix must derive continuation solely from that workspace-local evidence and must not treat agent exit status or narrative output as completion.

## Proposed Solution

Introduce a repository-state-based sequential merge continuation diagnosis used after every resolve attempt. The diagnosis will classify the earliest unfinished phase for each `(revision, change_id)` and produce bounded, actionable continuation context containing the affected repository/worktree path, current evidence, required next phase, exact commit subject, and archive resurrection cleanup requirement when both live and archived forms exist.

Keep the resolve agent responsible for conflict decisions and Git mutations. Strengthen the embedded `cflx-resolve` retry contract so an agent receiving phase-specific continuation must resume at the named incomplete phase, complete all remaining sequential protocol steps in the same attempt when possible, and avoid repeating an already completed pre-sync. Existing repository-verifiable success checks remain authoritative.

This remains one change because the state classifier, retry prompt contract, embedded skill guidance, and Git-backed regression tests must ship together to prove convergence behavior.

## Acceptance Criteria

1. After each resolve attempt, Conflux distinguishes an unfinished target-branch merge, unfinished worktree pre-sync, invalid or missing pre-sync evidence, missing final merge, required archive resurrection cleanup, and fully integrated completion using current repository state.
2. A pre-sync-complete/final-merge-missing state produces continuation context naming the change, branch, worktree path, target branch, exact `Merge change: <change_id>` subject, and the instruction to proceed to final merge rather than repeat pre-sync.
3. If a live `openspec/changes/<change_id>` exists while a valid exact or date-prefixed archive entry exists, final-merge continuation explicitly requires removal of the resurrected live directory before the final commit.
4. An unfinished merge or remaining conflict reports the exact repository/worktree location and required phase-specific completion action without claiming success.
5. Resolve succeeds only when existing merge-subject, ancestry, pre-sync, clean-merge-state, and conflict checks pass; agent exit status and prose remain non-authoritative.
6. Retry diagnostics are bounded, stable, and emitted through the existing resolve output/history path without introducing durable workflow state.

## Explicit Completion Conditions

- `src/parallel/conflict.rs` derives phase-specific continuation from Git and OpenSpec tree evidence and uses it in retry history instead of the generic missing-merge reason where a more precise state is available.
- Archive detection accepts the repository's valid exact and date-prefixed archive layouts and rejects unrelated or invalid entries.
- `skills/cflx-resolve/SKILL.md` tells retrying agents to resume from the diagnosed phase and complete every remaining sequential merge step, including resurrection cleanup, before returning.
- Rust tests create representative temporary Git repositories/worktrees and fail if pre-sync-only state is accepted, if continuation recommends repeating completed work, if resurrection cleanup is omitted, or if completed final integration is rejected.
- `cargo test parallel::tests::conflict` passes.

## Out of Scope

- Automatically committing or merging on behalf of the resolve agent.
- Bypassing Git hooks, rewriting branch history, changing merge commit subject conventions, or weakening existing verification.
- Changing queue reconciliation or manual merge-wait classification after retries are genuinely exhausted.
- Repairing the currently preserved `unify-remote-operator-commands` worktree as part of this proposal.
