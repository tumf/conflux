# Fixture-only design

## Fixed evidence and scope

Proposal inspection base: `323a110abd0a41d62fd75292375908a71a6df9bc`; historical full-lib run: `f618a3e7c3d4afb4f8c8ff0fa1e5bc83658ab411`. Line numbers below orient review at that base, not immutable acceptance hashes.

Historical diagnostics are external observation only:

- `/home/tumf/work/acceptance-checks/crlf-full-lib-f618a3e7-results/CLASSIFICATION.md`
- Adjacent `summary.json`, `controls-summary.json`, `serial-summary.json`, `individual-0.log`, `individual-2.log`, `executor-parallel.log`, `scoped-git-identity.log` and `source-comparison.json`.
- `/home/tumf/work/acceptance-checks/lib-fixture-proposal-c7cddf2f-48be-40ec-abb2-dc84d116c375/git-identity-control.json`: independent real Git control, missing identity exit 128/empty status versus repository-local identity exit 1/`UU`/stages 1,2,3.

These paths are investigation evidence, not dependencies, workflow-control state, implementation tasks, or permanent gate inputs. Acceptance is reproducible from tracked tests and the candidate alone.

## 1. Malformed judge: finished task, not written request

`src/analyzer.rs:2698-2703` writes the request file before the script body. The target at 3096-3139 releases its conventional analyzer on that request file. At 329-343 the real analyzer spawns the shadow, awaits the conventional command, then collects. `src/judge_command.rs:201-213` checks the task's `is_finished` and otherwise sends cancellation. Child stdout publication, shell exit, output draining, and completion of the async judge task are distinct events. Even `barrier_judge`'s shell `judge-done` marker is not an actual task-completion guarantee and must not be copied as the repair.

Selected implementation (test-only):

1. Add a private boolean field named `wait_for_shadow_judge_completion` to `ParallelizationAnalyzer` behind `#[cfg(test)]`, initialized to false behind the same cfg in `new`. No production field, CLI option, environment switch or global mutable hook.
2. In `analyze_with_callback`, immediately after `execute_analysis_command` returns and immediately before the existing `match shadow`, add a single `#[cfg(test)]` block. When the opt-in field is true, require `shadow.as_ref()` to exist, then await `handle.is_finished()` by `tokio::task::yield_now()` until true, wrapped in `tokio::time::timeout(Duration::from_secs(30), ...)`. Timeout or absent shadow panics with a fixture-precondition message. The timeout is a generous hang guard only, not proof of correctness. This block neither consumes nor replaces the handle/result. No sleeps or wall-clock speed assertion.
3. Only `a_bounded_judge_failure_emits_one_redacted_category` sets the field true before invoking the existing `analyze` helper / `analyze_with_inflight`. Replace its request-file-polling conventional command with immediate `printf` of `ANALYZE_JSON`. The fixture-only barrier supplies the ordering, not subprocess timing.
4. Preserve the actual `prose-judge` child, proposal secret, malformed stdout, production collect/parser/tracing, authoritative result and redaction assertions. Assert the judge really ran and strengthen the failure-category selection to exactly one WARN with `invalid_json`/`expected_answers=2`; disallow a `cancelled` outcome in this fixture. No fabricated `JudgeCommandOutput` or calling `trace_judge_failure` directly.

The false/default path (including all slow/hung/error cancellation controls) is unchanged. Do not edit `JudgeHandle::collect_or_cancel`, production waiting, cleanup, retry or parser behavior. Test seam state is instance-local so unrelated analyzers cannot become waiting analyzers. A cooperative yield observes a monotonic task-completion condition; it is not a fixed-count retry or a sleep substitute for ordering. Every unfinished loop iteration MUST execute `tokio::task::yield_now().await`; never replace it with a synchronous `is_finished()` busy-spin, which can starve the current-thread runtime and its process/IO driver.

## 2. Merge lane: same mutex and lifetime as the actual shared resource

`src/parallel/mod.rs:138-167` defines **two** locks: `GLOBAL_MERGE_LOCK` protects production lane execution; `merge_lock_test_mutex()` is the outer fixture-serialization lock. Neither is keyed by repository path. `attempt_merge` at `src/parallel/merge.rs:895-899` uses `try_lock` on the production lock and legitimately returns an automatic deferral when busy. The manual-resolve target is declared at `executor.rs:7257` and already has the correct outer test lock at 7258, so merely adding a lock to that test is a no-op repair.

Close these concrete unguarded entry sites, using **the existing** `crate::parallel::merge_lock_test_mutex().lock().await`, not a new mutex or key:

| Fixture | Why it reaches the actual lane | Guard lifetime |
|---|---|---|
| `parallel_pass_to_archive_to_merge_never_creates_or_cleans_an_acceptance_checkpoint` in `src/parallel/tests/executor.rs` (9399) | Direct `attempt_merge` at 9511 after its independent apply/acceptance/archive setup | Acquire immediately before the direct attempt, hold through the awaited attempt and event assertions; do not serialize earlier pipeline preparation |
| `test_dirty_to_clean_resolve_wait_wakes_retry_without_new_trigger` in `src/parallel/tests/executor.rs` (11174) | Existing workspace plus `maybe_dispatch_resolve_wait_retry` reaches `retry_deferred_merges_for` then `attempt_merge`; dirty preflight does not protect it because the production lock is acquired first | Acquire before the first retry call at 11207, hold through removal of its own dirty file, second retry and assertions |
| `deferred_retry_repromotes_and_converges_to_merged_without_user_action` in `src/parallel/tests/auto_resolve.rs` (360) | `handle_merge_result_with_tx` promotes a real existing-workspace retry via a spawned executor, which reaches `attempt_merge` | Acquire before the transition that promotes the retry, hold through receiving its **actual** `MergeResult` and handling it; receiving the result establishes lane work has returned |

The dirty-to-clean fixture uses `maybe_dispatch_resolve_wait_retry` (not the `_with_tx` variant), which awaits retry inline and does not spawn it; keeping the guard through those calls and assertions therefore covers all of its lane work. Do not switch that fixture to a spawned path as part of this repair. The converging `_with_tx` fixture instead must retain its guard through receipt of the actual spawned result as specified above.

Keep the already guarded target and deliberately contended lane tests unchanged except for explanatory fixture comments if necessary. The unit tests with missing/stale/no workspace or only synthetic merge outcomes return before `attempt_merge`; do not add mutexes to them merely because their names mention retries. No blanket serialization of `parallel::`, analyzer, Git tests, or full lib.

Do not hold `GLOBAL_MERGE_LOCK` while invoking `attempt_merge`: that would force the unwanted busy branch. Do not acquire the outer test mutex inside `attempt_merge` or nested helpers: callers already hold it, so that would deadlock. Do not invent per-repository test-lock keys while the runtime lock is process-global, or change production to use per-repository locks in this fixture proposal. Preserve legitimate busy-lane, resolve-counter-first and manual-continuation behavior.

Evidence precision: source shows concrete uncoordinated consumers and the exact early-return mechanism; the historical parallel failure does not prove which consumer held the lane in that run. Conflux must prove the repair with the declared default-parallel subset/full-lib gates, not assert a deadlock or claim a prior instrumented holder trace.

## 3. Conflict status: satisfy identity and prove conflict before observation

The test helper at `status_policy_repository_tests.rs:18-36` supplies child-local author/committer identity. The merge at 399-405 bypasses that helper and masks global/system config. Its nonzero assertion at 406 accepts missing-identity failure (128) as if it were a content conflict (1).

Inside **only** `production_status_paths_preserve_conflicted_classification`, after `git init`, call the existing fixture helper for `git config --local user.name cflx` and `git config --local user.email cflx@example.com`. Keep signing disabled and global/system config disabled on the direct merge. Repository-local configuration is visible to every child of this fixture and is deleted with that fixture normally; no test-process environment changes and no developer Git config changes.

Before production status calls, assert `merged.status.code() == Some(1)` with captured setup stdout/stderr for failure diagnosis, then use real `git ls-files --unmerged -- conflict.txt` to require three entries for that exact path with stages 1, 2 and 3. Check `MERGE_HEAD` exists. These setup assertions fail immediately on identity/configuration failure rather than misreporting status semantics. Preserve the existing production `UU conflict.txt` and dirty classification assertions.

Do not put identity defaults into a shared fixture initializer, runner environment or product command. The unchanged negative controls are:

- `execution::final_commit_lock_retry::git_tests::final_apply_commit_lock_does_not_retry_configuration_failure` (`src/execution/final_commit_lock_retry_git_tests.rs:466`), which deliberately blanks local identity.
- `execution::wip_lock_retry::git_tests::transient_wip_commit_lock_does_not_retry_identity_failure` (`src/execution/wip_lock_retry_git_tests.rs:216`), which deliberately blanks local identity.

They remain terminal failures without commit/retry; suite-wide author/committer environment overrides invalidate them and are forbidden.

## Verification matrix and ownership

The frontmatter is the sole command authority. No command is inferred from a narrative task, and no operational artifact is written into a target worktree.

| Required result | Task | Verification IDs | Ownership |
|---|---|---|---|
| Real completed malformed child, one redacted failure | 1.1 | `judge-completion-fixture` | integration, Conflux Acceptance |
| Non-authoritative and unfinished-judge behavior unchanged | 1.1, 2.1 | `judge-runtime-controls` | integration, Conflux Acceptance |
| Original manual resolve dispatch/manual deferral intact | 1.2, 2.2 | `manual-resolve-fixture` | integration, Conflux Acceptance |
| Same process-global test key, correct background lifetime, busy controls | 1.2, 2.2 | `merge-lane-fixture-isolation` | integration, Conflux Acceptance |
| Real conflict before status observation | 1.3 | `conflicted-status-fixture` | integration, Conflux Acceptance |
| Intentional missing identity preserved | 1.3, 2.3 | `final-commit-missing-identity-control`, `wip-missing-identity-control` | integration, Conflux Acceptance |
| All default lib tests together, no serial/environment workaround | 3.1 | `default-parallel-full-lib` | integration, Conflux Acceptance |
| Explicit web-monitoring configuration together | 3.1 | `web-monitoring-parallel-full-lib` | integration, Conflux Acceptance |

All gates are finite direct repository-local commands. The full-lib observations are required because isolated tests cannot expose this cross-fixture interference. Run each once on the candidate in normal default test parallelism; no fixed-count stability loop. Acceptance records command exit, selected test counts, pass/fail/ignored counts and external raw log location. The previous 36 ignored tests are an observation, not a new allowed-ignore budget; the change may not add ignores or move these tests behind feature gates. Existing hooks remain enabled; their path-filtered Rust checks do not replace this matrix.

Failure handling is fail-closed: timeout, absent judge, wrong Git exit/index precondition, wrong event kind, failed/missing/zero-test gate, or changed product semantics prevents completion. Report any unrelated full-lib failure separately without expanding this proposal or claiming PASS.
