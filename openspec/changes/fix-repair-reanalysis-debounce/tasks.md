## Implementation Tasks

- [x] Distinguish archived-dirty repair additions from normal queue additions in scheduler reconciliation. Completion condition: `src/parallel/queue_state.rs` returns or records whether reconciliation added reducer queued changes, archived-dirty repair candidates, or both. verification: unit - targeted scheduler reconciliation tests assert repair additions are classified separately from normal reducer queue additions.

- [x] Prevent archived-dirty repair rediscovery from indefinitely extending normal queue debounce. Completion condition: repeated discovery of the same unchanged repair candidate does not refresh normal queue-change debounce as if the user added a new queued change. verification: unit - a regression test simulates repeated archived-dirty repair discovery and asserts `perform_reanalysis_and_dispatch` can reach analysis instead of remaining in `debounce_active` forever.

- [x] Add a repair-specific re-analysis trigger or equivalent bounded debounce bypass. Completion condition: repair-driven re-analysis is represented separately from ordinary queue notification, either with a dedicated `ReanalysisReason` or with an equivalent explicit scheduler path. verification: unit - tests assert repair-triggered re-analysis bypasses or bounds debounce while ordinary queue notification debounce behavior remains unchanged.

- [x] Deduplicate or rate-limit repeated unchanged repair reconciliation diagnostics. Completion condition: repeated scheduler loops over the same unchanged repair candidate do not emit unbounded user-visible `Queue reconciliation discovered archived dirty workspace without reducer queued intent` style logs. verification: unit - test coverage checks repeated reconciliation of the same repair candidate is suppressed, summarized, or otherwise bounded without suppressing the first useful diagnostic.

- [x] Preserve workspace-derived resume semantics. Completion condition: the fix uses only workspace file state, workspace git state, base-branch tree comparison, and in-memory loop-local scheduler bookkeeping; it does not add durable workflow-control files outside worktrees. verification: integration - review source diff and tests to confirm no new durable workflow-control state is introduced under `~/.local/state/cflx` or similar paths.

- [x] Add or update regression tests for archived-dirty repair candidate scheduling. Completion condition: `src/parallel/tests/executor.rs` or adjacent parallel scheduler tests cover archived-dirty repair candidates discovered without reducer queued intent, repeated rediscovery, and successful analysis progression. verification: integration - run the specific new/updated tests with `cargo test <test-name> -- --exact`.

- [x] Run the relevant Rust verification suite. Completion condition: targeted tests pass, and the broader default test command selected by the implementer passes or any heavy tests are explicitly excluded per repository policy. (verification: integration - run targeted `cargo test` commands for the new tests and an appropriate broader `cargo test`/module test command.)

## Future Work

Operational cleanup of already-running stale processes or old orphaned worktrees is not part of this proposal.

If future evidence shows other scheduler no-progress loops unrelated to archived-dirty repair candidates, create a separate proposal for a generic scheduler no-progress circuit breaker.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-repair-reanalysis-debounce --archive-gate`
