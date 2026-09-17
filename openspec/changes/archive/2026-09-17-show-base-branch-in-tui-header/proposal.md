---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/tui-architecture/spec.md
  - src/tui/state.rs
  - src/tui/runner.rs
  - src/tui/render.rs
verifications:
  - id: tui-header-base-branch-tests
    requirement: The TUI header renders the startup-captured base branch after the project path without displacing existing header signals or overflowing narrow widths
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/render.rs
    evidence: cargo test header_base_branch --lib
    rerun: cargo test header_base_branch --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show the base branch in the TUI header

**Change Type**: implementation

## Problem / Context

The TUI header identifies the owned repository with its startup-captured project path, but it does not show the base branch into which Conflux will integrate changes. Operators running several repository instances cannot confirm that branch from the header.

The current header already reserves fixed lifecycle, dirty-state, and version segments, then gives the remaining display-column budget to a middle-elided project path. The base branch must join that startup identity without being re-read during redraw.

## Proposed Solution

At TUI startup, resolve the current attached branch from the same repository root used as the project identity. Store it as presentation-only process-local state in `AppState`. Render the non-empty value immediately after the project path as `[<branch>]`, before `[dirty]`.

Reserve the branch segment before calculating the project-path budget. Preserve the branch text when the path must shrink. If startup cannot resolve an attached branch, retain current header behavior and render no branch segment rather than inventing a value or failing TUI startup.

The current production header at base revision `e0a2c8eef868aa31bc0a37d6cc77c700b6a04db1`, implemented by `src/tui/render.rs`, is the visual authority. This change adds one styled text segment and otherwise preserves its existing colors, borders, ordering, right-aligned version, and truncation behavior.

## Acceptance Criteria

- A TUI started from attached branch `main` displays the project path followed immediately by `[main]`.
- The displayed branch is captured at startup and rendering does not query Git or the ambient current directory.
- A later branch or current-directory change does not retarget the already-running header.
- An unresolved or detached startup branch produces no branch segment and does not prevent startup.
- Path middle-elision reserves room for the branch, lifecycle status, `[dirty]`, and right-aligned version when terminal width permits.
- Narrow widths, wide Unicode, and combining marks remain bounded and panic-free.
- Existing workspace concurrency/backend badge remains absent.

## Explicit Completion Conditions

- `AppState` owns read-only presentation accessors for the startup-captured project path and optional base branch.
- The local TUI runner resolves the attached branch once from `repo_root` and supplies it to `AppState` before rendering.
- `render_header` places `[<branch>]` after the path and includes its display width in fixed-segment reservation.
- Focused final-buffer tests cover normal ordering, immutable captured identity, unresolved branch fallback, path elision with the branch present, dirty/version preservation, and narrow-width safety.
- `cargo test header_base_branch --lib` passes.
- Archive promotion updates the canonical `TUI Header Project Identity` requirement.

## Retired Scenarios

- tui-architecture: TUI Header Project Identity / Header identifies the owned project
- tui-architecture: TUI Header Project Identity / Later current-directory changes do not retarget the header
- tui-architecture: TUI Header Project Identity / Existing header signals survive project identity replacement
- tui-architecture: TUI Header Project Identity / Long project path is middle-elided by display width

Each is retitled rather than removed: the header identity these four describe now
includes the base branch, so the same behavior is asserted under the widened
titles in this change's delta. `Header identifies the owned project and base
branch` keeps the visible path and the absent concurrency/backend badge; `Later
repository identity changes do not retarget the header` keeps the captured path
against a later current-directory change and adds the branch; `Existing header
signals survive branch identity addition` keeps lifecycle status, the dirty
badge, right-aligned version, and narrow-width safety; and `Long project path
yields width to the branch segment` keeps the single `…` between retained
prefix and suffix, the assigned column bound, and the suffix-favoring odd-budget
split. `Unicode and tiny budgets remain safe` keeps its title. No coverage is
dropped.

## Out of Scope

- Changing the base branch during a running TUI.
- Adding branch selection or branch-management controls.
- Changing integration, scheduling, resume, or Git workflow authority.
- Changing header colors, borders, lifecycle labels, dirty semantics, or version placement.
- Reintroducing the workspace concurrency/backend badge.
