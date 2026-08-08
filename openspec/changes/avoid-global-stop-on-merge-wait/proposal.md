---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/parallel-merge/spec.md
  - src/parallel/types.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/modal_tests.rs
  - src/tui/state.rs
verifications:
  - id: merge-wait-notification-tests
    requirement: "Change-scoped post-archive resolve exhaustion remains visible and retryable without opening a blocking popup or changing the global execution lifecycle"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering ResolveFailed presentation with and without unrelated active work, retained merge-wait retry state, absence of warning popup ownership, and unchanged RunFatal popup/error behavior"
    rerun: "cargo test --lib tui::state::event_handlers::errors::tests && cargo test --lib parallel::tests::change_local_merge_error_scope && cargo fmt --all -- --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Avoid blocking the TUI for change-scoped merge wait failures

**Change Type**: implementation

## Premise / Context

- Bounded post-archive resolve exhaustion is already classified as `ResolveExhausted` with scheduler disposition `ContinueWithErrors`; only `RunFatal` maps to `AbortRun`.
- Canonical `tui-error-handling` requires `ResolveFailed` to keep the affected change in `merge wait` without entering global TUI Error, while unrelated active work continues.
- `src/tui/state/event_handlers/errors.rs` currently calls `show_warning_popup` for every `ResolveFailed`. The popup owns input until dismissed, which makes a change-local recoverable failure feel like a whole-run stop even though scheduler execution continues.
- Removing that popup also removes persistent full-diagnostic access inside the TUI after the bounded log entry scrolls out; the process file log remains the durable diagnostic source.
- Warning-popup presentation is explicitly non-authoritative under the constitution and canonical specs; removing this popup does not change merge safety, retry routing, or repository evidence.

## Problem / Context

The orchestration and reducer layers already scope bounded resolve exhaustion correctly, but the TUI presentation still uses a modal warning popup for that event. The popup overlays the running UI and consumes input until the operator closes it. This overstates severity, interrupts monitoring and controls for unrelated work, and visually conflicts with the existing `ContinueWithErrors` contract.

The diagnostic must remain visible and attributable to the failed change. Removing all notification would hide the explicit retry requirement. The smallest consistent behavior is to retain the structured warning/error log and `merge wait` row while avoiding a blocking overlay for change-scoped `ResolveFailed` events.

## Proposed Solution

Change the TUI handling of change-scoped `ResolveFailed` so it:

- keeps the affected row in `merge wait` and preserves the existing explicit retry path;
- retains the existing structured change-associated error-level diagnostic in the bounded TUI log and process file log;
- applies the same non-modal behavior to automatic post-archive exhaustion and operator-initiated manual resolve failures that emit change-scoped `ResolveFailed`;
- does not open `warning_popup`, does not consume popup input, and does not request graceful or immediate global stop;
- preserves `Running` while other active work exists and preserves the existing transition to `Select` when no active work remains;
- leaves typed `RunFatal` handling unchanged, including global Error presentation and stop/abort semantics;
- leaves other warning popups, including `on_merged` hook failures and destructive confirmations, unchanged.

## Acceptance Criteria

1. A change-scoped `ResolveFailed` caused by bounded post-archive exhaustion displays the affected change as `merge wait` and retains its structured diagnostic in the log.
2. Handling that event does not create a warning popup, does not capture operator input, and does not invoke global stop behavior.
3. The TUI remains `Running` while unrelated active work exists; when none remains, the existing non-fatal transition to `Select` remains valid.
4. Pressing the existing explicit merge-retry action for the affected `merge wait` row remains available after the failure.
5. A typed global `RunFatal` still enters TUI Error and retains its existing abort/stop presentation.
6. Existing popups for unrelated warning classes are not removed or reclassified.

## Explicit Completion Conditions

- `src/tui/state/event_handlers/errors.rs` no longer calls `show_warning_popup` from the change-scoped `ResolveFailed` path.
- The `ResolveFailed` path continues to attach `change_id` to the retained diagnostic and to set the row to `merge wait` without storing a change-level terminal `error` status.
- TUI regression tests prove the warning popup remains absent after `ResolveFailed`, ordinary input is not overlay-blocked, execution remains `Running` with other active work, and retry state remains available.
- Existing regression coverage proves `RunFatal` still enters global Error and popup behavior for other event classes remains intact.
- The declared repository-local verification command exits successfully.

## Scope Rationale

This is one presentation-layer correction. Scheduler disposition, reducer transitions, retry ownership, and merge safety are already implemented and specified. Splitting would create an unnecessary second proposal with no independently deployable behavior.

## Out of Scope

- Changing resolve retry count, `EvidenceWithheld` classification, repository cleanliness checks, or merge algorithms.
- Automatically retrying manual `merge wait` outcomes.
- Changing scheduler `ContinueWithErrors`, `CompletedWithErrors`, or `AbortRun` semantics.
- Removing warning popups for hooks, destructive actions, or genuine global failures.
- Adding new popup buttons, stop controls, configuration, or durable UI workflow state.
- Adding persistent full-diagnostic storage or a new details view inside the TUI; after bounded log eviction, operators use the existing process file log.

## Verification Ownership

Requirement-specific tests remain explicit because the tracked Rust pre-commit hooks are path-scoped and do not run for proposal-only commits. The implementation change will touch Rust paths, so the existing tracked rustfmt and clippy hooks will also run at commit time; they do not replace the behavior-specific tests above.
