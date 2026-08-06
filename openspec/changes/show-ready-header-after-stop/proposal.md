---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/changes/archive/update-tui-header-loop-state/
  - openspec/changes/archive/2026-01-19-tui-stopped-queue-policy/
  - openspec/changes/archive/2026-08-03-separate-tui-execution-modal-state/
  - src/tui/render.rs
  - src/tui/types.rs
verifications:
  - id: stopped-ready-header-regressions
    requirement: "An internally stopped but inactive orchestration renders the Ready header while retaining stopped-mode resume controls and execution semantics"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Ratatui buffer-test output covering cyan stopped Ready presentation, resume controls, modal-free Error behavior, running/stopping mappings, and modal label precedence"
    rerun: "cargo test --lib stopped_mode_header_shows_ready_with_resume_controls -- --list | grep -q stopped_mode_header_shows_ready_with_resume_controls && cargo test --lib stopped_mode_header_shows_ready_with_resume_controls && cargo test --lib error_mode_header_remains_unlabeled_without_modal -- --list | grep -q error_mode_header_remains_unlabeled_without_modal && cargo test --lib error_mode_header_remains_unlabeled_without_modal && cargo test --lib overlay_header_label_is_presentation_only -- --list | grep -q overlay_header_label_is_presentation_only && cargo test --lib overlay_header_label_is_presentation_only && cargo test --lib test_running_header_counts_only_in_flight_changes -- --list | grep -q test_running_header_counts_only_in_flight_changes && cargo test --lib test_running_header_counts_only_in_flight_changes && cargo test --lib test_stopping_mode_header_shows_stopping -- --list | grep -q test_stopping_mode_header_shows_stopping && cargo test --lib test_stopping_mode_header_shows_stopping && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show Ready header after stop

**Change Type**: implementation

## Problem / Context

The TUI header communicates whether orchestration is currently executing. The internal `AppExecutionMode::Stopped` value exists to preserve resume-specific command admission and controls after an operator stop; it is not a distinct running condition that should be exposed as a new header execution status.

The current renderer hides the header status in stopped mode, while older canonical sections still require a gray `Stopped` label. Both presentations obscure the useful operator fact: orchestration is no longer running and is ready for a marked resume. The status panel already retains the more specific `F5: resume` control, so the header can report `Ready` without losing stop/resume semantics.

## Proposed Solution

Project internal `AppExecutionMode::Stopped` to the existing cyan `[Ready]` header label. Keep `AppExecutionMode::Stopped`, `OperatorMode::Stopped`, app-mode/API token `stopped`, lifecycle `idle` projection, execution marks, and F5 resume routing unchanged.

The header remains a presentation projection rather than a one-to-one dump of internal control state:

- `Select` and `Stopped` display `[Ready]`;
- `Running` displays `[Running]` or `[Running N]`;
- `Stopping` displays `[Stopping]`;
- `Error` retains its current no-label behavior;
- active modal labels retain presentation precedence without mutating execution mode.

## Split Rationale

This proposal is independent from `fix-force-stop-reducer-reconciliation`. Header mapping can be implemented and verified without changing reducer state, while reducer reconciliation can ship without changing header presentation. Neither proposal consumes repository output from the other, so no hard dependency is declared and both may run in parallel.

## Acceptance Criteria

1. Internal stopped mode renders the same cyan `[Ready]` header label as Select mode.
2. Stopped mode continues to render resume-specific status controls, including the configured start-key label followed by `resume`.
3. Stopped mode retains `OperatorMode::Stopped` and its stopped-specific execution-mark admission rules; F5 continues through the existing `start_marked()` dispatch shared with Select mode, and this presentation change does not alter start/resume routing.
4. The header never displays a new `[Stopped]` execution status.
5. Running, active-count, Stopping, Error, QR, and confirmation-modal header behavior remain unchanged.
6. Rendering `[Ready]` does not mutate `AppExecutionMode`, API `app_mode`, external lifecycle projection, queue intent, or execution marks.
7. Canonical CLI requirements no longer conflict about hidden versus gray stopped labels and instead define Ready as the stopped header projection.

## Explicit Completion Conditions

- `src/tui/render.rs::render_header` maps only `AppExecutionMode::Stopped` to the existing Ready text/color path while leaving Error and overlay mappings unchanged.
- A `stopped_mode_header_shows_ready_with_resume_controls` buffer test proves `[Ready]` is cyan via the existing `fg_at` helper, resume controls coexist, `[Stopped]` is absent, and the internal mode remains `Stopped` after rendering.
- A modal-free `error_mode_header_remains_unlabeled_without_modal` regression proves Error renders neither `[Ready]` nor `[Stopped]`; existing tests continue to prove Running counts only active rows, Stopping remains visible, and modal labels are presentation-only.
- The CLI spec delta consistently updates `Running Mode Dashboard`, `TUI Layout Structure`, and `TUI Stopped Mode` without changing stop/resume command semantics.
- The commands declared by `stopped-ready-header-regressions` pass.

## Out of Scope

- Renaming or removing internal `AppExecutionMode::Stopped`, `OperatorMode::Stopped`, API `app_mode: stopped`, or external lifecycle mappings.
- Changing stopped-mode F5, Space, bulk mark, queue intent, or execution-mark behavior.
- Fixing stale per-change `accepting` state; that independent reducer correction is covered by `fix-force-stop-reducer-reconciliation`.
- Changing Error-mode header or retry controls.
- Reconciling unrelated canonical inconsistencies such as Stopping punctuation, historical Error-label requirements, or exact resume log text outside the three modified requirements.
- Adding a new TUI header state, badge, color token, or configuration option.
