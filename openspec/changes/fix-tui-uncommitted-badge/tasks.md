## Implementation Tasks

- [x] Introduce an explicit repository-derived parallel-eligibility reason in TUI state, preserving separate outcomes for proposal absence from `HEAD` and uncommitted or untracked files under `openspec/changes/<change_id>/`. Completion requires the existing boolean admission guard to remain false for both outcomes and unit tests to assert each classification independently. (verification: unit - extend eligibility tests under `src/tui/state.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Wire Changes-list rendering to show `UNCOMMITTED` only for the explicit dirty-proposal reason. Rows that are parallel-ineligible for any reason, including clean proposal absence from `HEAD`, must remain grayed out, non-markable, and without queue affordances exactly as today; only the badge distinguishes dirty content. Completion requires both select and running render paths to consume the same reason, both badge-width calculations to match the corrected 12-character ` UNCOMMITTED` label, and no active TUI string to retain `UNCOMMITED`. (verification: unit - extend rendering tests under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Make single-row toggle warnings and shared bulk-toggle exclusion summaries distinguish dirty proposal content from clean proposal absence. Completion requires dirty rows to retain an actionable commit instruction, absent rows to state that the change is not present in `HEAD`, and TUI plus shared operator-command tests to reject both without labeling the absent case uncommitted. (verification: unit - extend guard and exclusion-reason tests under `src/tui/state.rs`, `src/tui/state/selection_logic.rs`, and `src/orchestration/operator_command.rs`; verification-id: tui-eligibility-reason-tests)
- [x] Add regression cases for a clean proposal absent from `HEAD` and an archived or failed-merge change retaining a managed worktree while its row is displayed as `not queued` or `queued`. Completion requires each row to retain any applicable `WT` marker, omit `UNCOMMITTED`, remain grayed out and non-actionable, and stay subject to the unchanged parallel admission guard; statuses already excluded by the badge filter do not satisfy this task. (verification: integration - add state-plus-render cases under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [ ] Verify the implementation is formatted, lint-clean, and passes the default Rust test suite. Completion requires successful `make fmt`, `make lint`, and `make test` results with no source or test regressions attributable to this change. (verification: integration - `make fmt && make lint && make test`; verification-id: repository-quality-gates)

## Future Work

- No external deployment, credentials, migration, or human approval is required for this repository-local bug fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-uncommitted-badge --archive-gate`
