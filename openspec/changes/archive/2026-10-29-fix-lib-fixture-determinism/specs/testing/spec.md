## ADDED Requirements

### Requirement: Deterministic completion for the malformed shadow-judge fixture

The malformed shadow-judge runtime fixture SHALL establish completion of the actual judge task before it invokes the existing collection path. A request file or child-written output marker MUST NOT be treated as task completion. Ordering support MUST be test-only, per-analyzer-instance and opt-in solely for this fixture; it MUST NOT change non-test waiting, cancellation, cleanup, parsing or shadow authority.

#### Scenario: Completed malformed child produces one bounded diagnostic

- **GIVEN** the fixture runs the real malformed judge child with the proposal secret and the real conventional analyzer
- **WHEN** its test-only barrier observes the actual handle is finished before collection
- **THEN** the original test emits exactly one WARN failure-category record with `outcome="invalid_json"` and `expected_answers=2`, not `cancelled`
- **AND** raw secret output and unvalidated model identity are absent, and the authoritative order/dependencies remain unchanged
- **AND** neither sleep nor a substituted result/category establishes that outcome

#### Scenario: Completion never arrives

- **GIVEN** the opt-in fixture has no shadow or its actual task does not finish
- **WHEN** the barrier precondition or generous hang safeguard fails
- **THEN** the test fails explicitly rather than skipping or claiming completion
- **AND** the existing unfinished-judge runtime controls remain opt-out and continue to exercise production nonblocking cancellation unchanged

### Requirement: Shared merge-lane fixture isolation without suite serialization

The manual resolve-wait retry fixture and its lane-entering competitors SHALL coordinate through the existing process-global `merge_lock_test_mutex` at the actual lane boundary. Guard ownership MUST cover actual completion of spawned lane work; no new repository/change key or blanket suite serialization may substitute for the mutex protecting the existing process-global lane. Production lock scope, try-lock behavior, retry routing and manual versus automatic deferral semantics MUST remain unchanged.

#### Scenario: Unrelated apply completion causes a manual retry outcome

- **GIVEN** `test_manual_resolve_wait_retries_after_in_flight_apply_completes` holds its existing test guard and prepares an incomplete archived workspace
- **WHEN** unrelated in-flight Apply completes and the scheduler dispatches its real retry
- **THEN** the original dispatch assertion and `MergeDeferred(auto_resumable=false)` assertion both hold under default parallel test scheduling
- **AND** the post-archive pipeline fixture, dirty-to-clean retry fixture, and converging spawned-retry fixture use the same test guard across their actual lane operations, not their unrelated setup

#### Scenario: Intentional contention still exercises the busy branch

- **GIVEN** a contention control owns the outer test guard and intentionally holds the production merge lock
- **WHEN** it calls the existing `attempt_merge`
- **THEN** the original automatic busy-lane deferral is observed without waiting
- **AND** tests that do not touch the lane remain parallel and are not guarded merely to make the suite green

### Requirement: Conflict fixture identity is local and conflict setup is explicit

The conflicted-status fixture SHALL initialize author identity with repository-local `user.name` and `user.email` solely in its own temporary repository. Before production status assertions, it MUST establish a real content-conflicted index rather than accept any nonzero merge status. Global Git configuration, test-process identity environment, shared helpers and intentional missing-identity controls MUST remain unchanged.

#### Scenario: Status classifies a genuinely conflicted index

- **GIVEN** the temporary conflict repository has local identity and diverging edits to `conflict.txt`
- **WHEN** the fixture runs its real merge with global/system configuration disabled
- **THEN** setup asserts merge exit code 1, `MERGE_HEAD`, and unmerged stages 1, 2 and 3 for `conflict.txt`
- **AND** the existing production status paths still report `UU conflict.txt` and a dirty working directory
- **AND** identity/configuration failure is a setup failure, not evidence of a conflict

#### Scenario: Missing-identity negative controls are not repaired away

- **GIVEN** the final Apply and WIP lock-retry negative fixtures intentionally blank their own local identity
- **WHEN** they run unchanged alongside the locally prepared conflict fixture
- **THEN** identity failures remain terminal, produce no commit and are not retried
- **AND** both exact controls and the default-parallel full-lib gates pass without suite-wide identity overrides

### Requirement: Fixture repair is accepted with default-parallel full-lib evidence

This fixture-only repair SHALL retain all three original test identities and substantive assertions and SHALL pass its declared focused selections plus both default and explicit `web-monitoring` full-lib commands under ordinary default parallel scheduling. No new ignore/skip/feature exclusion, assertion weakening, suite serialization or process-global environment change may count as a repair.

#### Scenario: Focused success alone is insufficient

- **GIVEN** all exact fixture tests pass in isolation
- **WHEN** either declared full-lib command fails, selects no tests or has no completed result
- **THEN** the change is not complete and no full-suite PASS is claimed
- **AND** unrelated failures are reported without expanding this test-only scope

#### Scenario: Candidate has complete repository-local evidence

- **GIVEN** all focused gates select their expected nonzero tests and both full-lib commands complete with exit 0 and default parallel scheduling
- **WHEN** Conflux reviews the candidate diff and captured test counts/logs
- **THEN** only the specified fixture bodies and explicitly test-only support have changed, with product semantics and negative controls preserved
- **AND** logs remain external observability outputs, not target-worktree artifacts or workflow authority
