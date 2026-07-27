---
change_type: implementation
priority: high
dependencies:
  - fix-recoverable-analysis-fallback-event
references:
  - src/tui/state/event_handlers/output.rs
  - src/tui/state/event_handlers/mod.rs
  - src/tui/render.rs
  - src/tui/runner.rs
  - src/tui/orchestrator.rs
  - openspec/specs/tui-error-handling/spec.md
  - openspec/changes/fix-recoverable-analysis-fallback-event/
verifications:
  - id: tui-analysis-fallback-running-state
    requirement: TUI remains visibly Running while scheduler execution continues after recoverable dependency-analysis fallback
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: targeted TUI event-handler and rendering test output covering fallback warning, AppMode, header controls, and subsequent execution events
    rerun: cargo test analysis_fallback_running
    prerequisites:
      - fix-recoverable-analysis-fallback-event
---

# Keep TUI Running During Recoverable Analysis Fallback

**Change Type**: implementation

## Problem / Context

When LLM dependency analysis fails but metadata-dependency-only fallback succeeds, the scheduler continues processing changes. In the observed run, however, the TUI consumed the recoverable diagnostic as a global error. `AppState::handle_error` changed the application mode from `Running` to `Error` and cleared `current_change`, so the header stopped showing the running controls while background work continued.

This creates a misleading split-brain operator experience: execution is alive, but the primary lifecycle indicator suggests it stopped and offers retry controls. The existing `fix-recoverable-analysis-fallback-event` proposal corrects the producer-side event classification. This separate proposal owns TUI lifecycle and rendering consistency, including regression protection against future recoverable diagnostics being routed as fatal state.

## Proposed Solution

Keep the TUI in `AppMode::Running` whenever dependency analysis degrades successfully and scheduler work remains active. Present the fallback as a visible warning in the log without clearing the current running context. Ensure the status/header region continues to render running controls and elapsed execution information, and that subsequent processing, acceptance, archive, or completion events are handled normally.

Add a TUI-focused regression fixture that begins in `Running`, applies the recoverable analysis fallback diagnostic produced by the dependency-analysis path, renders the status/header, and then processes a later lifecycle event. The test must fail if the diagnostic changes the app to `Error`, removes the running controls, clears active context, or prevents later event handling.

## Acceptance Criteria

- A successful metadata dependency fallback while orchestration is active leaves the TUI in `AppMode::Running`.
- The status/header continues to show running controls and elapsed orchestration state instead of retry/error controls.
- The fallback reason and continued metadata-based execution remain visible as a warning log entry.
- Recoverable fallback presentation does not clear `current_change`, active rows, queue state, or reducer-derived scheduler state.
- Subsequent apply, acceptance, archive, merge, completion, stop, and refresh events remain processable after the warning.
- A genuinely fatal global execution error still transitions the TUI to `AppMode::Error` and shows retry controls.
- TUI presentation state remains non-authoritative and does not alter scheduler decisions.

## Explicit Completion Conditions

- TUI event handling has an explicit non-fatal path for the recoverable analysis fallback diagnostic supplied by `fix-recoverable-analysis-fallback-event`.
- A targeted state test starts with `AppMode::Running`, applies the fallback diagnostic, and asserts mode, current context, queue/reducer snapshot, and warning log remain correct.
- A render test asserts the status/header retains the `Esc: stop` running controls and does not expose the `retry` error controls after fallback.
- A sequence test applies a later real lifecycle event after fallback and proves the TUI continues updating normally.
- Existing tests prove `OrchestratorEvent::Error` still enters fatal error mode for genuine global failures.
- `cargo fmt --check`, targeted TUI tests, `cargo clippy -- -D warnings`, and `cargo test` pass.

## Dependencies

This change depends on `fix-recoverable-analysis-fallback-event`, which defines and emits the warning-only successful fallback event. It can be reviewed independently but should be implemented after that producer-side contract is available.

## Out of Scope

- Changing dependency analysis, fallback ordering, or dependency safety semantics.
- Retrying or repairing malformed LLM responses.
- Redesigning the complete TUI lifecycle model or all global error classifications.
- Suppressing genuine fatal execution errors or changing scheduler control state from TUI presentation.
