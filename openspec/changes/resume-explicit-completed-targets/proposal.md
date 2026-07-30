---
change_type: implementation
priority: high
dependencies: []
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/cli/spec.md"
  - "openspec/specs/parallel-execution/spec.md"
  - "openspec/specs/runtime-state/spec.md"
  - "src/cli.rs"
  - "src/orchestrator.rs"
  - "src/execution/state.rs"
  - "src/vcs/mod.rs"
  - "src/vcs/git/mod.rs"
  - "tests/run_exit_tests.rs"
verifications:
  - id: explicit-target-resume-tests
    requirement: Repeating an explicit parallel target set classifies active, base-integrated, resumable-worktree, and unknown IDs from repository evidence without server-side target recomputation.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: cargo test output for explicit_target_resume unit and real Git/worktree cases
    rerun: cargo test --features heavy-tests --test e2e_git_worktree_tests explicit_target_resume
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: resume explicit completed targets idempotently

**Change Type**: implementation

## Problem / Context

`filter_requested_changes()` validates positional and `--change` targets only against the current active OpenSpec change list. Once a requested change is archived and integrated into base, it disappears from that list. Repeating the same explicit target set after interruption therefore rejects the already completed ID as unknown before existing worktree resume logic can run.

A supervisor should be able to restart the exact requested set without inspecting active/archive/worktree state or deciding which IDs remain. That workflow decision belongs to cflx and must remain derivable from workspace files, workspace Git state, and base-branch tree comparison under the Constitution.

The repository already has `is_merged_to_base()`, which verifies an archive entry in the base-branch tree and absence of the active change directory without relying on commit messages. Its current Boolean surface must be factored into typed `Completed`, `NotCompleted`, `Contradictory`, and `EvidenceError` evidence so contradictions and Git read failures cannot collapse into unknown. The repository also has managed worktree discovery. These facts can classify explicit targets before dispatch without introducing durable external state.

## Proposed Solution

Replace active-list-only explicit target filtering in cumulative parallel run with repository-evidence target resolution.

For each deduplicated requested ID, preserving request order, Conflux shall classify:

1. `active`: present in the current active OpenSpec list; include as ordinary work;
2. `already_completed`: base-branch tree contains an exact or date-prefixed archive entry and no active change directory; skip dispatch successfully;
3. `resumable_workspace`: a cflx-managed worktree exists and its own file/Git state contains the requested active or archive evidence needed by existing resume detection; register it for normal resume routing;
4. `unknown`: none of the above evidence exists; reject the invocation before creating, deleting, or mutating worktrees.

Workspace or branch naming alone is insufficient resume evidence. Contradictory evidence, such as archive plus active directory in base or a managed worktree whose contents do not identify the requested change, fails safely rather than being treated as completed.

The behavior applies to explicit cumulative parallel targets whether or not `-u` is enabled. Without `-u`, classification reads the captured current local base before dispatch. With `-u`, Conflux first completes the mandatory initial upstream base-lane checkpoint, then classifies against the resulting current cumulative base before any change-worktree creation or reuse registration. `--all`, serial mode, dry-run side-effect suppression, duplicate rejection, dependency selection, and existing phase detection remain unchanged except for reporting already-completed/resumable classifications.

## Acceptance Criteria

1. Repeating the same explicit parallel target set after one target was archived and integrated skips that target as `already_completed` and continues remaining work without an unknown-ID error.
2. Base completion is proven by a typed repository-tree evidence result: matching archive entry plus absence of the active change directory is `Completed`, neither completion condition is `NotCompleted`, archive plus active directory is `Contradictory`, and branch/Git read failure is `EvidenceError`; commit subject, logs, events, and server DB are not accepted as proof.
3. An existing cflx-managed worktree becomes a resume target only when its file/Git state identifies the requested change and existing workspace state detection can route it; branch/workspace name alone is insufficient.
4. A target with no active, completed, or valid managed-worktree evidence remains an error, and all unknown/duplicate diagnostics are reported together before mutation.
5. Request order is retained for active/resumable processing and terminal reporting; already-completed IDs are retained in a separate ordered classification rather than silently discarded.
6. Contradictory or unreadable evidence fails safely with an actionable diagnostic and does not create a replacement workspace or mark the target complete.
7. `--no-resume` does not erase or bypass valid base-integrated completion, but rejects a target that is only recoverable from an existing worktree instead of deleting that worktree implicitly.
8. Parallel dry-run performs the same read-only classification against the current local base and reports active, already-completed, resumable, and unknown results without network fetch or worktree mutation/cleanup.
9. Serial target filtering and `--all` behavior remain unchanged; a real `-u` run executes its initial upstream checkpoint before the shared classification and, even when every target classifies already completed, still follows normal upstream zero-change recovery/finalization when recognized unpublished history exists.

## Explicit Completion Conditions

- A typed target-resolution model represents requested ID, classification, active metadata or workspace identity, and repository-evidence failure.
- `Orchestrator` captures the attached base identity at startup; ordinary runs resolve explicit targets from that local base, while enabled real `-u` runs complete the initial upstream checkpoint then resolve against the resulting current cumulative base, all before change-worktree creation/reuse registration.
- Base-integrated classification factors the existing `is_merged_to_base()` tree contract into typed `Completed`, `NotCompleted`, `Contradictory`, and `EvidenceError` results and tests exact/date-prefixed archives, active-directory contradiction, missing branch, and command failure.
- Managed-worktree classification uses existing discovery plus worktree-local state evidence and cannot pass from a matching name alone.
- Parallel scheduler initialization accepts active and resumable targets, excludes already-completed targets from dispatch, and retains all classifications for output/terminal consumers.
- Real Git/worktree tests repeat identical target sets across active, active-plus-managed-worktree precedence, archived+integrated, uncommitted base archive, archived-not-integrated worktree, malformed worktree, unknown, duplicate, dry-run, `--no-resume`, post-initial-checkpoint `-u`, and all-completed unpublished-recovery cases.
- `cargo test --features heavy-tests --test e2e_git_worktree_tests explicit_target_resume`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- Reconstructing requested targets from a server database or external checkpoint.
- Treating arbitrary branches/worktrees as cflx-managed resume state based only on names.
- Changing serial execution target semantics in this change.
- Automatically deleting contradictory, unreadable, or `--no-resume` worktrees.
- Reopening or rerunning a base-integrated completed change.
