---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/render.rs
  - openspec/specs/tui-key-hints/spec.md
verifications:
  - id: focused-merged-dismiss-hint-render
    requirement: "A focused merged row visibly advertises d: dismiss at an ordinary terminal width in Select and Running Changes views"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/render.rs
    evidence: "cargo test --lib tui::render::tests::a_focused_merged_row_advertises_both_dismissal_hints -- --exact && cargo test --lib tui::render::tests::running_mode_shows_the_same_target_dependent_hints -- --exact"
    rerun: "cargo test --lib tui::render::tests::a_focused_merged_row_advertises_both_dismissal_hints -- --exact && cargo test --lib tui::render::tests::running_mode_shows_the_same_target_dependent_hints -- --exact"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Lock focused merged-row dismissal hint visibility at ordinary width

**Change Type**: implementation

## Premise / Context

- The Changes title already composes `d: dismiss` only when the focused row has display status exactly `merged`.
- A `merged` row is terminal, so active-row and mark hints do not precede its local dismissal hint. The hint is currently rendered near the start of the Changes title and is visible at 120 columns in both Select and Running modes.
- Existing dismissal render tests use a 180-column buffer. They prove composition but do not lock the user-visible ordinary-width behavior against future title-order regressions.
- The `d` action, process-local dismissal behavior, status eligibility, bulk `D` action, and Worktrees-view key routing are already implemented and remain unchanged.

## Problem / Context

The requested discoverability behavior exists, but its regression tests use an unusually wide terminal. A later title-composition change could move `d: dismiss` beyond the visible border without failing the current tests.

## Proposed Solution

Change the existing merged-row dismissal render fixture from 180 to 120 columns. Keep the current title composition and production behavior unchanged. The existing Select, Running, non-`merged`, empty-target, and bulk-target assertions will then exercise final rendered cells at an ordinary width.

Do not add wrapping, a second help row, alternate wording, a new layout abstraction, or production-code reordering.

## Acceptance Criteria

- With a focused `merged` row, the rendered Changes title visibly contains `d: dismiss` at 120 columns in Select mode.
- The same visible hint is present at 120 columns in Running mode.
- A focused non-`merged` row does not show `d: dismiss`.
- `D: dismiss all merged` retains its existing eligibility and behavior.
- Existing mark, run, resolve, kill, edit, log, QR, app-level, and Worktrees-view controls keep their behavior.
- No production behavior, dismissal state, key routing, orchestration state, reducer, API, Web projection, Git, archive, or log behavior changes.

## Explicit Completion Conditions

- The existing merged-row dismissal final-buffer tests run at 120 columns.
- Focused Select and Running dismissal tests pass at that width.
- The complete merged-row dismissal test group passes.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and the repository test suite pass.
- `cflx openspec validate show-focused-merged-dismiss-hint --archive-gate` passes before archive.

## Out of Scope

- Changing `d` or `D` behavior.
- Changing title composition order.
- Persisting hidden rows across TUI restarts.
- Redesigning the full title or adding responsive help surfaces.
- Changing Worktrees-view deletion hints.
- Changing `/api/v2`, Web UI, reducer, scheduler, or lifecycle behavior.
