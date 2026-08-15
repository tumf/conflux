---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-merge/spec.md
  - src/parallel/merge.rs
  - src/parallel/resolve_state.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/command_handlers.rs
verifications:
  - id: manual-resolve-recovery
    requirement: A manual retry continues an identity-verified unfinished target merge instead of deferring on its own MERGE_HEAD
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused Rust regression tests exercise ResolveFailed, manual retry admission, repository classification, and completion
    rerun: cargo test --lib manual_resolve -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix manual resolve recovery for unfinished target merges

**Change Type**: implementation

## Problem / Context

Sequential resolve can exhaust its bounded agent retries after Git has already created a conflict-free target merge and left `MERGE_HEAD` present. The TUI correctly returns the change to `merge wait`, but pressing `M` starts the ordinary merge-attempt path, whose base-dirty guard sees `MERGE_HEAD` and emits another manual deferral before sequential resolve can inspect the repository.

The workspace remains dirty by design, while the only advertised recovery control is inert. The operator must currently commit or abort outside Conflux even though repository evidence is sufficient for Conflux to resume safely.

## Proposed Solution

Treat an explicit manual resolve retry as continuation of the selected change's existing resolve state. After the retry intent is admitted and the base-mutation lane is exclusively owned, route the request into repository-derived sequential resolve classification before rejecting the target merely because `MERGE_HEAD` exists.

The resolver must retain all existing fail-closed identity, topology, conflict-stage, resurrection-cleanup, and terminal checks. Unrelated dirty changes, an unowned or ambiguous `MERGE_HEAD`, unresolved conflicts, and unreadable evidence remain blocked without mutation.

## Acceptance Criteria

- After bounded resolve exhaustion leaves an identity-verified, conflict-free target merge in progress, the selected change is shown as `merge wait` and pressing `M` admits a new resolve attempt.
- The retry reaches sequential repository classification despite the existing `MERGE_HEAD`, performs only the required continuation action, and re-verifies completion.
- A successful retry commits with exact subject `Merge change: <change_id>`, clears `MERGE_HEAD`, and advances the change through the normal merged lifecycle.
- An unrelated dirty workspace, ambiguous or foreign `MERGE_HEAD`, unresolved conflict, or invalid topology remains unmodified and reports actionable failure evidence.
- Other changes may continue running; recovery remains change-scoped and does not convert the TUI to a global error state.

## Explicit Completion Conditions

- A regression test reproduces `ResolveFailed` followed by `M` while the target has a conflict-free `MERGE_HEAD`, and fails if the retry is short-circuited as `MergeDeferred`.
- Repository-level tests prove the valid continuation path reaches a clean committed state and invalid/foreign evidence performs no commit or cleanup.
- Focused verification `cargo test --lib manual_resolve -- --nocapture` passes.
- `cflx openspec validate fix-manual-resolve-unfinished-merge --strict --evidence warn` emits no warnings.

## Out of Scope

- Increasing the configured resolve retry count.
- Parsing agent output to infer success.
- Automatically committing after exhaustion without explicit retry intent.
- Repairing the separate TUI queued-follow-up issue tracked as `conflux-2ju6`.
