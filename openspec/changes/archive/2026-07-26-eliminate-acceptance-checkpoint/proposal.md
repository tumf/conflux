---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - src/parallel/acceptance_state.rs
  - src/parallel/dispatch.rs
  - src/serial_run_service.rs
  - src/execution/archive.rs
  - src/execution/state.rs
  - src/parallel/merge.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/cli/spec.md
verifications:
  - id: no-acceptance-checkpoint
    requirement: Conflux never creates or consumes .cflx/acceptance-state.json
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: scripts/test-time-top10.sh
    evidence: cargo test output and repository search showing no runtime checkpoint path or state API remains
    rerun: cargo test acceptance_state
    prerequisites: []
  - id: resume-rechecks-acceptance
    requirement: Interrupted unarchived work resumes through acceptance without persisted pass state
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: scripts/test-time-top10.sh
    evidence: serial and parallel resume test output covering Applied, Archiving, stalled, and archived workspaces
    rerun: cargo test resume
    prerequisites: []
  - id: archive-without-checkpoint-cleanup
    requirement: Archive and merge complete without checkpoint cleanup or false MergeWait
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: scripts/test-time-top10.sh
    evidence: archive and post-archive merge regression test output
    rerun: cargo test post_archive
    prerequisites: []
---

# Eliminate the Acceptance Checkpoint

**Change Type**: implementation

## Problem / Context

Conflux currently creates `<workspace>/.cflx/acceptance-state.json` to persist acceptance status, revision binding, retry count, prior findings, and semantic fingerprints. The file is runtime bookkeeping, but its lifecycle participates in resume and archive routing and requires write, read, stale-state, Git-ignore, commit-exclusion, and cleanup behavior.

In the observed failure, the checkpoint was present in Git history, its post-archive deletion made an otherwise complete worktree dirty, and post-archive verification incorrectly sent the change to manual `MergeWait`. Adding more rules to exclude or normalize this generated file preserves the unnecessary state machine.

## Proposed Solution

Stop creating `.cflx/acceptance-state.json` entirely and remove runtime dependence on it.

- Keep acceptance status, revision, retry count, findings, and semantic baseline in memory only during one active orchestration run.
- On restart, route an unarchived completed implementation through acceptance again instead of trusting or reconstructing a prior PASS.
- Continue to use repository-visible `tasks.md` acceptance follow-up tasks for repair work.
- Continue to use the existing tracked `APPLY_BLOCKED/marker.md` contract only after acceptance enters a resumable stalled hold.
- Determine archive and merge completion from workspace file state, Git state, and base-branch tree evidence.
- Remove checkpoint cleanup from archive, merge, queue, and workspace cleanup paths.

This is one atomic proposal because removing persistence without changing resume routing would permit archive without verified acceptance, while changing routing without removing all checkpoint lifecycle calls would retain the original failure mode.

## Acceptance Criteria

- Conflux does not create, read, update, or delete `.cflx/acceptance-state.json` in serial or parallel execution.
- No archive, merge, queue, cleanup, semantic fingerprint, or resume path contains special handling for that file.
- During one active run, apply followed by acceptance PASS proceeds directly to archive without a disk checkpoint.
- After restart, an `Applied` workspace with complete tasks runs acceptance before archive.
- After restart, an incomplete archive/`Archiving` workspace does not infer a prior PASS from missing state; it runs acceptance again before finalizing archive unless repository evidence already proves archive completion.
- An already archived or base-integrated change proceeds to resolve/merge/terminal handling without redundant acceptance.
- Acceptance FAIL continues to persist actionable repair findings in `tasks.md`; a stalled hold continues to persist its tracked blocker marker.
- Restart before a stalled marker exists may reset in-memory retry count and semantic baseline, but it must not skip acceptance or archive unverified work.
- A valid archive cannot enter manual `MergeWait` because acceptance checkpoint cleanup dirtied the worktree.
- Real unrelated dirty files and invalid archive evidence continue to block merge.

## Explicit Completion Conditions

- Runtime checkpoint types and functions are removed or reduced so no acceptance JSON checkpoint serialization API remains.
- Repository search finds no production reference to `.cflx/acceptance-state.json`.
- Serial and parallel tests prove uninterrupted PASS handoff, restart acceptance rerun, stalled marker recovery, archived resume, and incomplete-task apply routing.
- Archive and merge regression tests reproduce the original post-archive sequence without creating or deleting the checkpoint and without false manual `MergeWait`.
- Tests prove genuine dirty worktree and invalid archive evidence remain blockers.
- Canonical specifications no longer require an acceptance checkpoint or pre-stall retry-context reconstruction after restart.
- `cflx openspec validate eliminate-acceptance-checkpoint --strict --evidence warn` and standard Rust quality gates pass.

## Out of Scope

- Persisting acceptance PASS in another hidden file, database, cache, commit trailer, or external state directory.
- Skipping acceptance after restart when archive completion is not repository-verifiable.
- Removing `tasks.md` acceptance follow-ups or `APPLY_BLOCKED/marker.md` stalled evidence.
- Rewriting historical commits that already contain `.cflx/acceptance-state.json`.
- Weakening archive-layout or unrelated dirty-worktree checks.
