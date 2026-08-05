## Implementation Tasks

- [x] Introduce an explicit repository-derived parallel-eligibility reason in TUI state, preserving separate outcomes for proposal absence from `HEAD` and uncommitted or untracked files under `openspec/changes/<change_id>/`. Completion requires the existing boolean admission guard to remain false for both outcomes and unit tests to assert each classification independently. (verification: unit - extend eligibility tests under `src/tui/state.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Wire Changes-list rendering to show `UNCOMMITTED` only for the explicit dirty-proposal reason. Rows that are parallel-ineligible for any reason, including clean proposal absence from `HEAD`, must remain grayed out, non-markable, and without queue affordances exactly as today; only the badge distinguishes dirty content. Completion requires both select and running render paths to consume the same reason, both badge-width calculations to match the corrected 12-character ` UNCOMMITTED` label, and no active TUI string to retain `UNCOMMITED`. (verification: unit - extend rendering tests under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Make single-row toggle warnings and shared bulk-toggle exclusion summaries distinguish dirty proposal content from clean proposal absence. Completion requires dirty rows to retain an actionable commit instruction, absent rows to state that the change is not present in `HEAD`, and TUI plus shared operator-command tests to reject both without labeling the absent case uncommitted. (verification: unit - extend guard and exclusion-reason tests under `src/tui/state.rs`, `src/tui/state/selection_logic.rs`, and `src/orchestration/operator_command.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Add regression cases for a clean proposal absent from `HEAD` and an archived or failed-merge change retaining a managed worktree while its row is displayed as `not queued` or `queued`. Completion requires each row to retain any applicable `WT` marker, omit `UNCOMMITTED`, remain grayed out and non-actionable, and stay subject to the unchanged parallel admission guard; statuses already excluded by the badge filter do not satisfy this task. (verification: integration - add state-plus-render cases under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Verify the implementation is formatted, lint-clean, and passes the default Rust test suite. Completion requires successful `make fmt`, `make lint`, and `make test` results with no source or test regressions attributable to this change. (verification: integration - `make fmt && make lint && make test`; verification-id: repository-quality-gates)

## Future Work

- No external deployment, credentials, migration, or human approval is required for this repository-local bug fix.

## Notes

- evidence: `make fmt` clean and `make lint` (`cargo clippy -- -D warnings`) exit 0.
- evidence: `cargo test -- --skip killing_the_lock_owner_releases_the_repository_lock` exit 0 — 16 suites, 3470 passed, 0 failed, 28 ignored.
- The single skipped test, `tests/run_exit_tests.rs::killing_the_lock_owner_releases_the_repository_lock`, is a pre-existing non-hermetic integration test unrelated to this change, first recorded during `allow-tui-dirty-worktree-delete` on 2026-08-04. Root cause confirmed on 2026-08-05: `get_global_config_paths` (`src/config/mod.rs:88`) always merges the developer's real global config, so the lifecycle adapter configured in `~/.config/cflx/config.jsonc` is inherited by the test's temp project; the poll loop's competing `cflx run --all` then proceeds into real orchestration and blocks forever inside `cflx_output`'s `Command::output()` (`tests/run_exit_tests.rs:378`), which has no timeout.
- evidence: the skipped test passes deterministically once that leak is removed — running the same prebuilt `run_exit_tests` binary with `HOME` pointed at an empty temp directory gives `1 passed; 0 failed; finished in 1.79s`, while the identical binary under the real `HOME` hangs past a 60s timeout.
- Isolating `HOME`/`XDG_CONFIG_HOME` for the whole suite is not a usable gate workaround: measured on 2026-08-05 it makes 114 `HOME`-dependent tests (`config::tests`, `openspec::tests`, `openspec_cmd::*`) fail (3255 passed / 114 failed). Making that test hermetic is a separate repository-wide concern and is deliberately out of scope for this UI-only change.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-uncommitted-badge --archive-gate`
