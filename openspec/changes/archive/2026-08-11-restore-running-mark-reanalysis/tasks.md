## Implementation Tasks

- [x] Add a process-local mark-settlement notifier owned alongside `ExecutionMarkStore`. TUI `OperatorCommandService::apply_execution_mark` and API/coordinator `set_execution_mark` and `set_all_execution_marks` notify it after accepted standalone operator writes; do not reroute Space or bulk `x` through the API coordinator. Notifications arm one 10-second deadline only when a live dynamic-queue scheduler exists; system revocations, no-ops, refusals, and Start-admission mark writes do not arm or restart it. Notification must not acquire the reducer or operator transaction recursively, and settlement must run on a separate task. Complete when paused-time unit tests in the orchestration lib target, all named `running_mark_reanalysis_*`, prove both service paths, no early settlement, deadline reset, system-revocation non-reset, one final settlement, persistent-idle continuation, finite-run discard with one operator-visible abandonment outcome, and restart-empty state. (verification: unit - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

- [x] Classify deadline settlement from current marks and one coherent reducer/operator view, deriving additions only for marked, loadable, ordinary `not queued` rows. Do not call the queue service for active, admitted, queued, error, retry, merge/resolve wait, terminal, unavailable, or otherwise ineligible rows; terminal-error queue service calls can create explicit retry edges. Complete when `src/orchestration` lib-target tests named `running_mark_reanalysis_*` prove every exclusion, duplicate no-op behavior, additive-only unmark behavior, and a lifecycle transition racing settlement. (verification: unit - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

- [x] Wire stable additions through the existing shared queue command path so reducer queue intent, `DynamicQueue`, queue hooks, authoritative outcomes, and scheduler notification retain existing cardinality and ordering. Complete when `src/orchestration` lib-target tests named `running_mark_reanalysis_*` prove each actual membership addition produces exactly one reducer transition and dynamic mutation, while an empty plan produces none and no path emits dequeue, retry, resolve, cancellation, or stop. (verification: integration - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

- [x] Route local TUI Space and bulk `x`, plus equivalent accepted standalone shared operator mark commands, into the coordinator without a frontend timer or new key. Exclude marks written as part of Start admission. Complete when `src/tui` lib-target adapter tests named `running_mark_reanalysis_*` prove equivalent commands schedule the same settlement, overlays retain input ownership, a parked persistent scheduler remains eligible, a process without a live scheduler remains mark-only, and rejected Start leaves no delayed queue effect. (verification: integration - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

- [x] Preserve scheduler semantics after stable admission: a real queue addition creates the existing queue-addition reanalysis edge, analyzes queued candidates during active resolve and at zero capacity, and dispatches only when capacity is available. Complete when `src/parallel/tests/running_mark_reanalysis.rs` lib-target tests named `running_mark_reanalysis_*` observe `AnalysisStarted` before resolve completion, no `ApplyStarted` at zero capacity, and later dispatch after a capacity transition without another mark or Start action. (verification: integration - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

- [x] Add a `test-running-mark-reanalysis` Make target that lists lib-target tests, fails if zero `running_mark_reanalysis_*` tests are discovered, and then runs the same focused lib-target tests. Add an archive-preparation scenario-set comparison that fails if promotion drops any canonical scenario not explicitly replaced by this change. Complete when a temporary unmatched filter is proven non-zero/fail-safe, the canonical-vs-promoted comparison retains all unrelated scenarios, and the real target passes without running heavy tests or relying on short wall-clock thresholds. (verification: integration - `make test-running-mark-reanalysis`; verification-id: running-mark-reanalysis-tests)

## Future Work

- Making the 10-second mark stability interval configurable.
- Adding mark-driven admission to product surfaces that do not use the shared operator command boundary.

## Notes

Verification evidence for this apply iteration, all run in the foreground:

- `make test-running-mark-reanalysis` — 35 focused `running_mark_reanalysis_*` lib-target tests passed in 0.41s, followed by the canonical-vs-promoted scenario-set comparison (2 capability deltas).
- Discovery gate proven fail-safe: `make test-running-mark-reanalysis MARK_REANALYSIS_FILTER=no_such_test_zzz` exits non-zero with `FAIL: no 'no_such_test_zzz' lib-target test was discovered` before running anything.
- Scenario-set guard proven fail-safe: a probe copy of the `tui-architecture` delta with `Prevent duplicate additions` removed and undeclared makes `scripts/check-scenario-set.py` exit 1 naming that scenario.
- `make test` — full default suite passed (no heavy tier).
- `make lint` — `cargo clippy -- -D warnings` clean.
- `cargo fmt --all -- --check` — clean.
- `cflx openspec validate restore-running-mark-reanalysis --strict` — passed.

Two defects found and fixed while verifying the previous iteration's work:

- The rejected-Start adapter test wrote its ineligibility straight into the shared `ParallelRuntime`, which `AppState::set_parallel_runtime` then republished away from the app's own rows, so the Start fence admitted the target and queued it. The arrangement now sets `parallel_eligibility` on the row and publishes from there, as production does.
- The Space/bulk-`x` comparison hung: both harnesses arm in the same paused instant, so the first `tokio::time::advance` settled both, and the second helper call then waited for a pass count that was already in the past. Both pass subscriptions are now taken before the single advance.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate restore-running-mark-reanalysis --archive-gate`
