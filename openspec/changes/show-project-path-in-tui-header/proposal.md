---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/tui/render.rs
  - openspec/specs/tui-architecture/spec.md
verifications:
  - id: tui-project-path-header
    requirement: The TUI header shows the captured project path instead of the workspace concurrency/backend badge
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/render.rs
    evidence: Focused Rust render-test output proving the captured project path is visible and the workspaces badge is absent
    rerun: cargo test tui_header_shows_project_path --locked
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show the project path in the TUI header

**Change Type**: implementation

## Problem / Context

The TUI header currently spends persistent space on `[workspaces:<max>:<backend>]`, for example `[workspaces:three:auto]`. This execution-configuration badge is not useful during normal operation. The operator instead needs to know which project the TUI owns, especially when several Conflux instances are open.

The local TUI already captures its repository root at startup. That captured path is the correct stable identity and must not follow later process current-directory changes.

## Proposed Solution

Replace the workspace concurrency/backend badge with the captured project path. Store the startup repository root in TUI state and render it in the header after the lifecycle status.

Use the path as captured for the project, without adding configuration, path aliases, or a second repository discovery mechanism. When the full path does not fit, apply conventional middle elision: measure terminal display columns, reserve one column for `…`, keep both the path prefix and suffix, and give the suffix the extra column when the remaining width is odd. This follows the common `ElideMiddle` / path-ellipsis behavior while preserving the most distinguishing tail of the path.

Preserve existing status, dirty badge, and right-aligned version behavior. Compute the path budget from the actual header area after reserving the other visible header segments; do not rely on byte or Unicode scalar counts.

## Acceptance Criteria

- The TUI header shows the project path captured at startup.
- `[workspaces:<max>:<backend>]` is no longer rendered.
- The displayed path remains tied to the startup repository even if process current directory later changes.
- A path wider than its available header budget is rendered as a single `…` between a retained prefix and suffix, measured in terminal display columns.
- The elided path fits its assigned width for ASCII, wide Unicode, combining marks, and widths too small to retain both sides.
- Ready/Running/Error presentation, the dirty badge, and the right-aligned version remain intact.
- Narrow terminal rendering remains bounded and does not panic.

## Explicit Completion Conditions

- `AppState` receives the startup repository path from the existing `repo_root` in the local TUI runner.
- Header rendering uses that state rather than calling `current_dir()`.
- Focused unit and render tests assert the exact full path when it fits; deterministic middle elision when it does not; terminal-column safety for ASCII, wide Unicode, and combining marks; absence of `[workspaces:`; preservation of adjacent badges/version; and bounded narrow-width rendering.
- `cargo test tui_header_shows_project_path --locked` succeeds.

## Out of Scope

- Changing execution concurrency, VCS selection, or their runtime/API observability.
- Adding home-directory substitution, component-aware rewriting, or user configuration beyond width-driven middle elision.
- Changing WebUI headers or remote owner identity contracts.

## Verification Ownership

The focused repository-local render tests are the change-blocking proof. Repository-wide formatting and Clippy remain owned by the tracked path-scoped pre-commit hooks when Rust files change.
