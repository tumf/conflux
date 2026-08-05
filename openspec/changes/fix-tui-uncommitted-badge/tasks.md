## Implementation Tasks

- [ ] Introduce an explicit repository-derived parallel-eligibility reason in TUI state, preserving separate outcomes for proposal absence from `HEAD` and uncommitted or untracked files under `openspec/changes/<change_id>/`. Completion requires the existing boolean admission guard to remain false for both outcomes and unit tests to assert each classification independently. (verification: unit - extend eligibility tests under `src/tui/state.rs`; verification-id: tui-eligibility-reason-tests)
- [ ] Wire Changes-list rendering and key-hint/actionability logic to show `UNCOMMITTED` and suppress dirty-row queue affordances only for the explicit dirty-proposal reason, without making other ineligible rows queueable. Completion requires both select and running render paths to consume the same reason and no active TUI string to retain `UNCOMMITED`. (verification: unit - extend rendering tests under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [ ] Add regression cases for a clean proposal absent from `HEAD` and an archived or failed-merge row retaining a managed worktree. Completion requires each row to retain any applicable `WT` marker, omit `UNCOMMITTED`, and remain subject to the unchanged parallel admission guard. (verification: integration - add state-plus-render cases under `src/tui/render.rs`; verification-id: tui-eligibility-reason-tests)
- [ ] Verify the implementation is formatted, lint-clean, and passes the default Rust test suite. Completion requires successful `make fmt`, `make lint`, and `make test` results with no source or test regressions attributable to this change. (verification: integration - `make fmt && make lint && make test`; verification-id: repository-quality-gates)

## Future Work

- No external deployment, credentials, migration, or human approval is required for this repository-local bug fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-uncommitted-badge --archive-gate`
