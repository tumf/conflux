---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/changes/archive/2026-08-03-separate-tui-execution-modal-state/
  - src/events.rs
  - src/lifecycle_integration.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/tui/lifecycle.rs
  - src/tui/runner.rs
  - src/tui/state.rs
verifications:
  - id: tui-lifecycle-tests
    requirement: "Persistent TUI lifecycle reporting distinguishes active execution from blocked/stalled-only waiting without introducing a second lifecycle authority"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering typed snapshot precedence, reducer-to-TUI status synchronization, repeated publication deduplication, and unchanged fallback behavior"
    rerun: "cargo test --lib tui::lifecycle:: && cargo test --lib tui::runner:: && cargo test --lib lifecycle_integration:: && cargo fmt --check && cargo clippy -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix TUI blocked lifecycle reporting

**Change Type**: implementation

## Problem / Context

The persistent TUI can remain in `AppExecutionMode::Running` while every remaining change is waiting in canonical `blocked` or `stalled` state and no change is actively executing. This is intentional scheduler behavior: the process stays available for later repository changes or explicit retries instead of emitting `AllCompleted`.

External lifecycle reporting currently loses that distinction. `TuiLifecycleSnapshot` captures execution mode, modal state, stop mode, and current change, but not reducer-synchronized change statuses. Its projection therefore maps every `Running` snapshot to `working`, even when the TUI and `/api/v2` correctly show a blocked/stalled-only wait.

The non-interactive event projection already knows that blocker events are semantically blocked, but wiring that event sink into the TUI would create competing publishers. The TUI frame loop would publish `working` again from its incomplete snapshot on the next frame. The fix must make the existing typed TUI snapshot authoritative rather than add another authority.

## Proposed Solution

Extend `TuiLifecycleSnapshot::from_app` with two typed row-status facts evaluated after reducer-to-TUI synchronization: whether any row is active or queued, and whether any row is `blocked` or `stalled`. Reuse the canonical active-status helper and do not parse rendered terminal content.

Apply this projection order:

1. A user-decision modal reports `blocked`.
2. `Running` with any active or queued change reports `working`, including mixed active/waiting rows.
3. `Running` with no active or queued change and at least one `blocked` or `stalled` row reports `blocked`.
4. Other zero-active `Running` snapshots retain the existing `working` fallback.
5. `Stopping`, `Select`, `Stopped`, and `Error` retain their existing mappings.

Keep the projection observability-only. It must not mutate reducer state, scheduler state, queue intent, retry routing, acceptance, archive, merge, or resume decisions.

## Atomic Scope Rationale

The canonical lifecycle requirement and the typed TUI projection describe one externally visible contract. Shipping either alone would preserve a specification/implementation mismatch, so the spec delta, projection change, and regression coverage belong in one change.

## Acceptance Criteria

1. A persistent TUI in `Running` with no active or queued change and at least one reducer-synchronized `blocked` or `stalled` row reports external lifecycle `blocked`.
2. Any active canonical row status takes precedence over blocked/stalled rows and reports `working`.
3. A queued row also preserves `working`, matching the canonical row state while work remains admitted for possible dispatch.
4. User-decision modals remain the highest-priority `blocked` signal, and QR remains transparent to the underlying lifecycle.
5. `Stopping` remains `working`; `Select` and `Stopped` remain `idle`; `Error` remains `blocked`.
6. A `Running` snapshot with no active, queued, blocked, or stalled rows retains the existing `working` fallback.
7. Repeated frame publication of an unchanged blocked/stalled-only snapshot is deduplicated and does not alternate back to `working`.
8. Lifecycle reporting continues to use typed in-memory state and remains a one-way observability output.

## Explicit Completion Conditions

- `src/tui/lifecycle.rs` captures the two typed row-status facts required by the precedence above after reducer-to-TUI synchronization and reuses the canonical active-status vocabulary rather than defining a divergent active list.
- The TUI frame loop remains the sole TUI lifecycle publisher; no second `LifecycleEventSink`, screen scraper, or adapter-specific branch is added.
- Unit tests cover blocked-only, stalled-only, mixed active/waiting, queued/waiting, empty-running fallback, stopping, QR, and user-decision precedence.
- A reducer-path test applies blocker events, synchronizes the resulting canonical display cache into `AppState`, and proves a still-`Running` TUI projects `blocked` for both blocked and stalled outcomes.
- A publication test proves consecutive equivalent blocked snapshots emit one semantic transition and never emit an intervening `working` transition.
- `cargo test --lib tui::lifecycle::`, `cargo test --lib tui::runner::`, `cargo test --lib lifecycle_integration::`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Changing or using the `conflux-herder` wrapper.
- Changing the external adapter, Herdr status authority, lifecycle protocol, or JSON schema.
- Changing persistent scheduler behavior, `AppExecutionMode`, or the TUI `[Running]` presentation.
- Reclassifying canonical blocker facts or broadening this change to error, merge-wait, or resolve-pending semantics.
- Reclassifying a candidate that remains canonically `queued` while scheduler eligibility is temporarily unavailable; this change follows the typed row state rather than duplicating scheduler classification.
- Changing the non-interactive `cflx run` lifecycle event mapping.
- Using lifecycle output as workflow-control input.
