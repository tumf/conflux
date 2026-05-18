## Implementation Tasks

- [ ] Add repo-root-based active and rejected change listing for local TUI refresh, replacing cwd-relative listing in the refresh path (verification: unit - tests in `src/openspec.rs` prove listing from an explicit base path still finds active and rejected rows when process cwd points elsewhere)
- [ ] Wire local TUI auto-refresh to pass the captured `repo_root` through active and rejected change update logic without changing remote-mode refresh bypass behavior (verification: unit/integration - tests around `src/tui/runner.rs` or refresh helpers verify local refresh uses explicit repo root and remote mode still bypasses local refresh)
- [ ] Preserve new active change state as unselected `not queued` with `is_new = true` and rejected rows as read-only non-new rows (verification: unit - `src/tui/state/processing_logic.rs` or refresh handler tests cover active new row, rejected new row, marker removal reactivation, and unchanged cursor index)
- [ ] Add a Running-mode visible new-change indicator when `new_change_count > 0`, independent of whether the appended row is inside the visible list viewport (verification: unit - `src/tui/render.rs` TestBackend render test shows `New: 1` or equivalent in Running mode with logs panel enabled and many changes)
- [ ] Emit a TUI log entry when active changes are newly detected, while keeping that log observability-only and not feeding queue/scheduler state (verification: unit - `src/tui/state/processing_logic.rs` or `src/tui/state/event_handlers/refresh.rs` test confirms the log entry appears once per newly detected active change and no `selected`/queue status changes are introduced by logging)
- [ ] Run affected Rust verification and formatting checks (verification: manual - run `cargo test` for affected modules/tests and `cargo fmt --check`; if full `cargo test` is too slow, document the targeted commands and why broader coverage was not run)

## Future Work

- Manual dogfood: run local TUI, add a valid `openspec/changes/<id>` with `proposal.md` during Running mode, and confirm the row or new-change indicator is visible without moving the cursor.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate`
