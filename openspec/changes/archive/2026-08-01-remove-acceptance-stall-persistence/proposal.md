---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/runtime-state/spec.md
  - src/parallel/acceptance_state.rs
  - src/parallel/dispatch.rs
  - src/execution/state.rs
  - src/serial_run_service.rs
  - src/orchestration/state.rs
verifications:
  - id: acceptance-stall-inmemory
    requirement: Acceptance stall is in-memory only, never persisted to disk, and restart runs acceptance again
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: cargo test output
    rerun: cargo test --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Remove acceptance stall disk persistence

**Change Type**: implementation

## Problem / Context

`AcceptanceStallRecord` is currently persisted to `~/.local/state/cflx/acceptance-stalls/<repo>/<change>.json` as a JSON file. On restart, `preflight_acceptance_stall()` (serial) and `reconcile_acceptance_stall()` (parallel) reload this file and restore `WaitState::Stalled`, suppressing ordinary dispatch.

This contradicts the core constitutional principle that workflow state must be derivable from the workspace alone. Constitution law 1a introduced a narrow exception for this persistence, but in practice it causes:

- **Stall permanence**: restart does not clear the stalled state. The change stays stuck until explicit operator retry.
- **Spec violation**: `runtime-state` already requires "deleting `~/.local/state/cflx/**` MUST NOT change the next action". The persistence violates this.
- **`parallel-execution` conflict**: the spec says "After process restart, an applied but unarchived workspace MUST run acceptance again" (line 1554), but the persistent stall record suppresses this re-run.

## Proposed Solution

1. **Amend constitution law 1a**: Remove the narrow runtime pause/resume exception. All runtime state stays in-memory.
2. **Remove disk persistence in code**: Stop calling `AcceptanceStallStore::save()`, stop loading on restart. Keep the struct and store as dead code for one release cycle, then remove in a follow-up.
3. **Update specs**: Align `parallel-execution` and `orchestration-state` specs with in-memory-only stall semantics.
4. **Ignore existing stall files**: Never load `~/.local/state/cflx/acceptance-stalls/` entries again. They are also never deleted, so a concurrently running older Conflux sharing the same state directory does not lose its holds.

## Acceptance Criteria

- Acceptance stall is recorded only in `OrchestratorState` in-memory (`WaitState::Stalled`).
- No JSON file is written to `~/.local/state/cflx/acceptance-stalls/` during acceptance.
- Process restart always re-runs acceptance for applied-but-unarchived workspaces.
- Existing stale stall files under `~/.local/state/cflx/acceptance-stalls/` are never read and never deleted.
- Explicit operator retry of a stalled change still works (in-memory: consumes the hold, resumes acceptance).
- Constitution law 1a is removed.

## Explicit Completion Conditions

- `persist_acceptance_stall()` (in `src/execution/state.rs`) and its caller in `src/parallel/dispatch.rs` no longer write to disk.
- `record_acceptance_stall()` in `src/serial_run_service.rs` no longer writes to disk; marks in-memory stalled state only.
- `preflight_acceptance_stall()` in `src/serial_run_service.rs` no longer loads from disk.
- `reconcile_acceptance_stall()` in `src/parallel/acceptance_state.rs` no longer reads disk records.
- `load_valid_acceptance_stall()` in `src/execution/state.rs` is removed or refactored.
- No startup or dispatch code reads, writes, or deletes files under `~/.local/state/cflx/acceptance-stalls/`.
- Unit tests confirm: no file I/O during stalled hold lifecycle.
- Integration test: restart a run where a change was stalled mid-run; acceptance re-runs.
- `openspec/CONSTITUTION.md` law 1a is removed and law 1 is amended to remove the dangling reference and clarify ephemeral in-memory state is permitted.
- `openspec/specs/parallel-execution/spec.md` MODIFIED to remove out-of-worktree persistence language.
- `openspec/specs/orchestration-state/spec.md` MODIFIED to replace out-of-worktree persistence with in-memory-only semantics.

## Out of Scope

- Removing `AcceptanceStallStore` struct itself (dead-code cleanup in follow-up).
- Changing the acceptance agent verdict format (separate concern).
- Changing cflx-accept skill (separate proposal `clarify-post-integration-nonblocking`).
