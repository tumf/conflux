---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state/event_handlers/output.rs
  - src/events.rs
  - openspec/specs/tui-error-handling/spec.md
verifications:
  - id: tui-fatal-event-classification
    requirement: Error-channel events remain fatal regardless of diagnostic message content
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for TUI error-handler and rendering regression tests
    rerun: cargo test tui::state::event_handlers::output && cargo test tui::render
    prerequisites: []
---

# Remove TUI Fallback Message Classifier

**Change Type**: implementation

## Problem / Context

The dependency-analysis fallback producer now emits successful fallback diagnostics as typed warning log events. The TUI nevertheless retains a defensive classifier that searches every global error message for `RECOVERABLE_ANALYSIS_FALLBACK_MARKER` and downgrades a match to a warning.

A genuine fatal error can contain that marker through wrapped error context, quoted diagnostics, or future message composition. The substring match would then preserve `AppMode::Running` after orchestration had actually stopped, hide retry controls, and recreate the scheduler/TUI lifecycle mismatch in the opposite direction.

## Proposed Solution

Remove message-content classification from the global error handler. Treat every event delivered through the global error channel as fatal. Preserve successful dependency-analysis fallback behavior exclusively through its existing warning-log event path.

Remove marker exports or classifier helpers only when they are no longer required by the producer warning message or tests. Do not add another string-prefix, exact-string, or regex classifier.

## Acceptance Criteria

- A successful dependency-analysis fallback delivered through the warning-log event path preserves `AppMode::Running`, active context, and running controls.
- Every global error event enters `AppMode::Error` even when its message contains the recoverable fallback marker or quotes a fallback warning.
- Fatal error diagnostics remain error-level and retry controls remain visible.
- Producer warning wording remains operator-visible without becoming workflow-control input.

## Explicit Completion Conditions

- `src/tui/state/event_handlers/output.rs` no longer decides fatality from fallback marker text.
- Regression tests inject a fatal error containing the marker and prove that mode, log level, and header controls are fatal.
- Existing production-path fallback warning tests continue to prove Running state preservation.
- `cargo test tui::state::event_handlers::output` and `cargo test tui::render` pass.
- `cargo fmt --check` and repository lint pass.

## Out of Scope

- Redesigning the full event enum.
- Changing scheduler fallback selection or metadata dependency behavior.
- Changing warning diagnostic deduplication.
