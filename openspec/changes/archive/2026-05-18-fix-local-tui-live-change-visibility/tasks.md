## Implementation Tasks

- [x] Add repo-root-based active and rejected change listing for local TUI refresh, replacing cwd-relative listing in the refresh path (verification: unit - tests in `src/openspec.rs` prove listing from an explicit base path still finds active and rejected rows when process cwd points elsewhere)
- [x] Wire local TUI auto-refresh to pass the captured `repo_root` through active and rejected change update logic without changing remote-mode refresh bypass behavior (verification: unit/integration - tests around `src/tui/runner.rs` or refresh helpers verify local refresh uses explicit repo root and remote mode still bypasses local refresh)
- [x] Preserve new active change state as unselected `not queued` with `is_new = true` and rejected rows as read-only non-new rows (verification: unit - `src/tui/state/processing_logic.rs` or refresh handler tests cover active new row, rejected new row, marker removal reactivation, and unchanged cursor index)
- [x] Add a Running-mode visible new-change indicator when `new_change_count > 0`, independent of whether the appended row is inside the visible list viewport (verification: unit - `src/tui/render.rs` TestBackend render test shows `New: 1` or equivalent in Running mode with logs panel enabled and many changes)
- [x] Emit a TUI log entry when active changes are newly detected, while keeping that log observability-only and not feeding queue/scheduler state (verification: unit - `src/tui/state/processing_logic.rs` or `src/tui/state/event_handlers/refresh.rs` test confirms the log entry appears once per newly detected active change and no `selected`/queue status changes are introduced by logging)
- [x] Run affected Rust verification and formatting checks (verification: manual - run `cargo test` for affected modules/tests and `cargo fmt --check`; if full `cargo test` is too slow, document the targeted commands and why broader coverage was not run)

## Future Work

- Manual dogfood: run local TUI, add a valid `openspec/changes/<id>` with `proposal.md` during Running mode, and confirm the row or new-change indicator is visible without moving the cursor.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate`

## Acceptance Follow-up Tasks

- [x] Isolate the `CONTROL_CALLS` test helper state with `lock_control_calls_for_test()` so parallel `cargo test --lib` execution no longer mixes calls from other tests. (verification: manual - runnable commands `cargo test --lib` and `cargo fmt --check`; source path `src/server/api/control.rs`)
- [x] Normalize acceptance follow-up checklist text so completed tasks cite repository-verifiable evidence and do not claim completion of final OpenSpec validation gates. (verification: manual - source path `openspec/changes/fix-local-tui-live-change-visibility/tasks.md`; runnable command `cargo fmt --check`)
- [x] Move historical acceptance-gate failure details into a non-checkbox Notes section so final validation remains the authoritative non-task gate. (verification: manual - source path `openspec/changes/fix-local-tui-live-change-visibility/tasks.md`; runnable command `cargo fmt --check`)

## Notes: Acceptance Follow-up History

- Acceptance #1 observed a default `cargo test --lib` failure in `server::api::control::tests::test_global_control_run_skips_rejected_changes`: expected one `("_global_", "run")` call but saw three calls. The repository fix added test-only serialization and clearing around `CONTROL_CALLS` users in `src/server/api/control.rs`.
- Acceptance #2 identified that a follow-up checklist item lacked verification ownership/evidence wording; the checklist has been rewritten with explicit `manual` ownership plus source paths and runnable commands.
- Acceptance #3 and #4 identified self-referential final OpenSpec validation checklist wording; final validation instructions now live only in the non-checkbox `## Final Validation` section.
- Acceptance #5 identified remaining evidence parsing failures caused by historical checklist text; the historical detail is preserved here as notes without checkboxes, while the active follow-up tasks above describe repository-verifiable changes only.
