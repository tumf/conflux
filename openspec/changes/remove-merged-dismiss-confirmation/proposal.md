---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state/merged_row_dismissal.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - src/tui/types.rs
  - src/tui/state/modal_logic.rs
  - openspec/specs/tui-merged-row-dismissal/spec.md
verifications:
  - id: immediate-dismissal-tests
    requirement: "Changes-view d and D dismiss eligible merged rows immediately without a confirmation state"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state/merged_row_dismissal.rs
    evidence: "cargo test tui:: --lib"
    rerun: "cargo test tui:: --lib"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Remove merged-row dismissal confirmation

**Change Type**: implementation

## Premise / Context

- `dismiss-merged-rows` shipped process-local row suppression and refresh filtering correctly.
- Its implementation and canonical spec also shipped a confirmation modal, despite the operator requirement that `d` and `D` act immediately.
- The smallest repair keeps dismissal storage, refresh behavior, cursor repair, hints, Worktrees deletion, and safety boundaries unchanged while deleting only the merged-dismissal confirmation path.
- The production TUI at revision `ecddffc1191cbf8069f5b4eab6cdf5935196333d` is visual authority; this change removes an overlay and introduces no new visual surface.

## Problem / Context

In the Changes view, pressing `d` or `D` currently opens `ConfirmDismissMergedRow` or `ConfirmDismissAllMergedRows` and waits for `Y`. The required interaction has no confirmation step because dismissal affects only process-local presentation and is restored by restarting the TUI.

## Proposed Solution

- Make `d` derive the currently focused exact-`merged` ID and apply the existing local dismissal mutation in the same key-handling turn.
- Make `D` derive all exact-`merged` IDs from the full Changes-list projection and apply the same mutation immediately.
- Remove the two dismissal variants from `ModalState`, their validity logic, modal key routing, popup rendering, modal logs, and confirmation-specific tests.
- Retain one shared state mutation for dismissed-ID recording, row removal, cursor repair, known-ID/NEW cleanup, selected-log-filter cleanup, refresh suppression, and active-ID reuse.
- Modify the canonical dismissal requirement so it explicitly forbids a confirmation overlay or second key press.

## Acceptance Criteria

- `d` on a focused `merged` row removes that row before the key event returns and leaves `app.modal` unchanged.
- `D` removes every projected `merged` row, including off-viewport rows, before the key event returns and leaves non-merged rows unchanged.
- Ineligible `d` and `D` remain silent no-ops.
- No `ConfirmDismissMerged*`, dismissal confirmation popup, or dismissal-specific `Y/N/Esc` branch remains.
- Refresh suppression, active ID reuse, cursor repair, local logs, and existing context-aware hints continue to work.
- Worktrees-view `d` and `D` retain their existing delete confirmation behavior.
- Repository, archive, Git, reducer, API/Web, queue, mark, and lifecycle state remain untouched by dismissal.

## Explicit Completion Conditions

- Source and tests contain no merged-row dismissal confirmation modal types or rendering paths.
- Key-routing tests assert row removal immediately after one `d` or `D` key event and assert no modal is opened.
- State/refresh/render regression tests cover individual, bulk, mixed-status, empty-result, repeated refresh, active reuse, cursor/filter cleanup, and hint visibility.
- `cargo test tui:: --lib` passes.
- Strict validation and archive gate pass.

## Out of Scope

- Changing worktree deletion confirmations.
- Durable dismissal across TUI restarts.
- Dismissing statuses other than exact `merged`.
- Changing refresh, reducer, API, archive, queue, mark, or lifecycle semantics.
