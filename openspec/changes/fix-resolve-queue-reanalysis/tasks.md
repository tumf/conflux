## Implementation Tasks

- [x] Update scheduler entry conditions so queued work enters `perform_reanalysis_and_dispatch()` even when active resolve work makes current available slots zero. (verification: unit - add or update `src/parallel/tests/manual_resolve.rs` / `src/parallel/tests/auto_resolve.rs` to fail if `src/parallel/orchestration.rs` skips re-analysis solely due to zero slots)
- [x] Refactor `perform_reanalysis_and_dispatch()` capacity handling so queue classification and dependency analysis can run before dispatch suppression, while selected ordinary apply dispatch remains blocked when recalculated available slots are zero. (verification: unit - test that analysis callback is invoked with queued candidates during active resolve and no `ApplyStarted` event is emitted while capacity is zero)
- [x] Preserve slot recovery behavior so resolve completion bypasses stale queue debounce and eligible queued changes dispatch without a new queue notification. (verification: integration - scheduler test simulates resolve counter dropping from one to zero with queued eligible work and asserts `ApplyStarted` is emitted on the next scheduling evaluation)
- [x] Preserve blocked-only and waiter semantics while changing analysis timing. (verification: unit - existing blocked-only / resolve-wait / reject-wait tests continue to pass, and add coverage if any regression is found during implementation)
- [x] Ensure diagnostics distinguish zero-capacity dispatch suppression from no dispatchable candidates. (verification: unit - event/log assertion observes capacity-gated diagnostic when analysis ran but dispatch is suppressed)
- [x] Run focused verification for parallel scheduler behavior. (verification: integration - run relevant tests such as `cargo test parallel::tests::manual_resolve parallel::tests::auto_resolve parallel::tests::executor --features heavy` or the repository-specific focused equivalents discovered during implementation)

## Future Work

- Broader manual TUI dogfooding can confirm visual status transitions during a real resolve session, but repository tests are the required completion evidence for this change.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-queue-reanalysis --archive-gate`
