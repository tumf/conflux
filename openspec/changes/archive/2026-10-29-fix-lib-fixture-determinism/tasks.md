## Implementation Tasks

- [x] 1.1 Add the instance-local `#[cfg(test)]` judge-completion opt-in exactly as specified in design section 1, enable it only in the malformed-judge fixture, remove that fixture's request-file/sleep polling, and preserve its real child/analyzer path while strengthening the one-category/redaction assertion. Done when actual `JudgeHandle::is_finished()` precedes collection, stalled completion fails with the generous hang guard, and the exact original test passes without changing production cancellation. (verification: integration - real child completion and diagnostic assertions in `src/analyzer.rs`; verification-id: judge-completion-fixture)
- [x] 1.2 Close the three named merge-lane fixture guard gaps in design section 2 using the existing process-global `merge_lock_test_mutex`, holding it across the complete lane-critical call and actual spawned retry result where applicable, without guarding non-lane setup or unrelated tests. Done when the target's existing manual-deferral assertion remains intact, all three entry sites cooperate with its existing lock, and deliberate global-lock contention still has its original automatic-deferral behavior. (verification: integration - `src/parallel/tests/executor.rs`, `src/parallel/tests/auto_resolve.rs` and genuine busy-lane controls; verification-id: merge-lane-fixture-isolation)
- [x] 1.3 Configure Git identity only in the conflict fixture's temporary repository and assert merge exit 1, `MERGE_HEAD`, and unmerged stages 1/2/3 for `conflict.txt` before preserving the existing production `UU`/dirty checks. Done when missing identity cannot masquerade as conflict setup and no common helper, process environment, or intentional missing-identity fixture is changed. (verification: integration - `src/vcs/git/commands/status_policy_repository_tests.rs` actual Git conflict/status assertions; verification-id: conflicted-status-fixture)

## Regression Tasks

- [x] 2.1 Verify the opt-in barrier is disabled for other analyzers and preserve the existing slow/error judge, non-authoritative result, and redaction controls unchanged. Done when the shadow runtime selection contains the original target and cancellation controls, selects nonzero tests, and passes without adding sleep-based ordering, ignores or fake results. (verification: integration - `src/analyzer.rs` existing shadow runtime controls; verification-id: judge-runtime-controls)
- [x] 2.2 Verify the original manual-resolve test still observes the actual scheduler retry and `MergeDeferred(auto_resumable=false)` with no alternate auto-resumable success condition. Done when its exact selector runs one passing test and the fix's Git diff only changes test-boundary guarding for the named competing lane fixtures, not production lock or event semantics. (verification: integration - `src/parallel/tests/executor.rs` exact scheduler retry regression; verification-id: manual-resolve-fixture)
- [x] 2.3 Preserve and verify both intentional missing-identity negative tests unchanged. Done when the final Apply identity-failure control and WIP identity-failure control each run one passing test with no author/committer override and still reject without retry/commit. (verification: integration - `src/execution/final_commit_lock_retry_git_tests.rs` and `src/execution/wip_lock_retry_git_tests.rs` unchanged terminal identity-failure controls; verification-id: final-commit-missing-identity-control; verification-id: wip-missing-identity-control)

## Acceptance Notes

3.1 Conflux Acceptance must execute both `default-parallel-full-lib` and `web-monitoring-parallel-full-lib` once each on the candidate, with the ordinary default harness parallelism, alongside all seven focused gates. These are mandatory structured change-blocking gates, not an Apply repetition task and not replaced by exact-test success. Record exit codes and selected/pass/fail/ignored counts with externally stored raw logs. The default feature already includes web-monitoring; both requested invocation spellings are nevertheless explicit. No `RUST_TEST_THREADS`/`--test-threads=1` or suite-wide Git identity workaround is permitted. A failed full-lib gate remains a failure; report unrelated failures without broadening this fixture-only change.

Change-scoped review must confirm only the allowlisted fixture/test-only surfaces and proposal artifacts changed. It must also confirm no new skip/ignore/feature gate, no weakened assertions, no process-global mutation, no sleeps as completion proof, and no product semantics change. Do not pin unrelated production blobs forever as a test.

## Notes

Apply verification evidence (single runs, candidate working tree):

- `analyzer::tests::shadow_judge_runtime::` selected 8 tests, all passed, including the exact malformed-judge target, which now reports exactly one WARN `invalid_json` record with `expected_answers=2`.
- `parallel::` selected 594 tests: 594 passed, 0 failed, 4 ignored.
- These exact gates each selected 1 test and passed: the conflicted-status fixture, manual-resolve fixture, final-commit missing-identity control, and WIP missing-identity control.
- `cargo clippy --locked --offline --lib --tests --features web-monitoring -- -D warnings` and `cargo fmt -- --check` were clean.
- `cargo test --locked --offline --lib` passed 4872, failed 0, ignored 36.
- `cargo test --locked --offline --lib --features web-monitoring` passed 4871, failed 1, ignored 36. The one failure was `vcs::git::commands::worktree::tests::test_worktree_remove_blocks_on_teardown_failure` (`src/vcs/git/commands/worktree.rs:2258`, unexpected error type). It is unrelated to this change: the file is untouched, and it passes alone with `--exact`. The test writes a teardown script and executes it at once, which fits a parallel exec race on a freshly written script. Per the proposal it is reported here, not repaired. The full-lib run was not repeated because no repository repair justifies a rerun. Acceptance owns the authoritative full-lib gates.

## Final Validation

Archive validation itself is the final OpenSpec structural gate: `cflx openspec validate fix-lib-fixture-determinism --archive-gate`. Authoring check: `cflx openspec validate fix-lib-fixture-determinism --strict --evidence warn`. Neither command proves that the Rust implementation or declared tests ran. All implementation checkboxes remain pending until Conflux produces repository evidence.

## Out of Scope

No manual Rust implementation during proposal authoring, no unrelated fixture/product changes, no full-suite serialization or global identity setting, no merge/push/mark/start/owner restart. No post-integration or external prerequisite is required.
