---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/resolve_state.rs
  - src/parallel/conflict.rs
  - src/parallel/merge.rs
  - src/task_parser.rs
  - openspec/specs/parallel-merge/spec.md
verifications:
  - id: merge-authorization-tests
    requirement: Sequential final merge guidance is withheld when repository-visible tasks are incomplete, while complete changes retain the existing merge path.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: change-acceptance
    automation: src/parallel/resolve_state.rs
    evidence: Focused Rust tests exercise active and archived task evidence, retry suppression, safe refusal, and the complete-task control path.
    rerun: cargo test --locked parallel::resolve_state
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent unfinished final merge

**Change Type**: implementation

## Problem

Sequential post-archive resolution currently derives `FinalMergeMissing` from Git topology and emits an imperative final-merge action even when repository-visible `tasks.md` remains incomplete. If an agent safely declines that merge without changing Git state, the next resolve attempt receives the same instruction and can override the safety stop.

This allowed an archived 6/7-task change with a red focused suite to be merged into the base branch. The immediate defect is not conflict resolution itself: Conflux authorizes and repeats a final merge before proving task completion.

## Proposed Solution

Add one deterministic merge-authorization outcome to sequential resolve classification. Before emitting final-merge guidance, inspect the selected change's repository-visible task progress from its active or archived `tasks.md`.

When tasks are incomplete or task evidence cannot safely establish completion:

- classify the change as merge not authorized;
- emit no `git merge` or final-commit instruction;
- make the state ineligible for another agent attempt;
- terminate through the existing evidence-withheld/manual-action path;
- retain the repository and worktree unchanged.

When all tasks are complete, preserve the existing topology checks and final merge path.

A typed resolver refusal within the current batch acts only as a monotonic stop latch. It cannot authorize work and cannot be overridden by another attempt in the same batch. Narrative output is not parsed.

## Acceptance Criteria

- Active and archived changes with incomplete tasks never receive imperative final-merge guidance.
- An unchanged incomplete state does not launch another resolve agent attempt.
- A typed safety refusal is not replaced by the same action for another agent in the same batch.
- Conflict resolution and merge authorization remain separate: conflicts may be resolved while final merge remains withheld.
- Complete-task changes retain the existing sequential merge behavior.
- Blocking one change does not authorize mutation of that change or unrelated changes.

## Explicit Completion Conditions

- Sequential classification has one typed non-authorized outcome reused by task evidence and typed refusal.
- The outcome is non-agent-actionable and reaches the existing evidence-withheld/manual-action terminal path.
- Focused tests cover active 6/7, archived 6/7, no second attempt, typed refusal latching, conflict-resolved-but-not-authorized, and complete-task merge control behavior.
- `cargo test --locked parallel::resolve_state` passes.
- `cflx openspec validate prevent-unfinished-final-merge --archive-gate` passes before archive.

## Out of Scope

- Acceptance-result revision binding and stale-acceptance gating.
- New durable merge receipts or out-of-worktree workflow state.
- Parsing agent prose as authority.
- Repository-wide quality gates in final-merge classification.
- Repairing or reverting the historical downstream repository merge.
- Redesigning all resolve failure classifications.

## Design Constraint

The change must remain workspace-derived and comply with `openspec/CONSTITUTION.md`. It may reuse ephemeral in-process refusal latching, but must not add durable external workflow state.

## Future Work

Record acceptance execution revisions without gating first. Evaluate a freshness gate only after observing real pre-sync behavior and false-block risk.
