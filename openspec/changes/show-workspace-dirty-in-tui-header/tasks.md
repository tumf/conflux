## Implementation Tasks

- [x] Add a typed, process-local TUI workspace-dirty observation and event adoption path. Completion requires unknown, clean, and dirty to remain distinguishable for presentation; successful refresh events replace the prior value; failed observations publish no false clean event; and the value is absent from reducer, persistence, and command-admission inputs. (verification: unit - add `workspace_dirty_header_state_adopts_successful_observations` and `workspace_dirty_header_state_preserves_last_success_on_failure` coverage in `src/tui/state.rs` or `src/tui/state/event_handlers/refresh.rs`; run `cargo test --lib workspace_dirty_header_state`; verification-id: workspace-dirty-header-tests)

- [x] Integrate the existing `crate::vcs::git::commands::has_uncommitted_changes` predicate into the existing five-second local TUI refresh using the startup-captured repository root. Completion requires one observation per refresh, no additional timer or status parser, and real temporary-Git-repository coverage for staged, unstaged, untracked, ignored-only, clean-after-dirty, changed process current directory, and Git-status failure cases. (verification: integration - add focused refresh tests in `src/tui/runner.rs`; run `cargo test --lib refresh_workspace_dirty`; verification-id: workspace-dirty-header-tests)

- [x] Render a red bold `[dirty]` badge in `src/tui/render.rs::render_header` after the workspaces badge only for a known dirty observation. Completion requires render-buffer tests proving dirty visibility, clean and unknown omission, preservation of Ready/Running/modal and workspaces content, and continued rendering of the right-aligned version area at representative terminal widths. (verification: unit - add focused header buffer tests in `src/tui/render.rs`; run `cargo test --lib workspace_dirty_header`; verification-id: workspace-dirty-header-tests)

- [x] Add a regression proving the presentation-only dirty signal cannot alter orchestration behavior. Completion requires identical reducer display statuses, execution marks, and command availability before and after dirty presentation events, while the rendered header alone changes. (verification: integration - add `workspace_dirty_header_is_observability_only` near TUI event/state integration tests; run `cargo test --lib workspace_dirty_header_is_observability_only`; verification-id: workspace-dirty-header-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate show-workspace-dirty-in-tui-header --archive-gate`.

## Future Work

- A WebUI-header dirty badge may be proposed separately if operators need the same always-visible signal in the browser console.
- Dirty file counts or category details require a separate UX and performance decision; this change intentionally exposes only the boolean warning.
