---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/testing/spec.md
  - src/analyzer.rs
  - src/judge_command.rs
  - src/parallel/mod.rs
  - src/parallel/merge.rs
  - src/parallel/tests/executor.rs
  - src/parallel/tests/auto_resolve.rs
  - src/vcs/git/commands/status_policy_repository_tests.rs
verifications:
  - id: judge-completion-fixture
    requirement: The malformed-judge fixture observes real task completion and one redacted invalid_json diagnostic
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/analyzer.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring analyzer::tests::shadow_judge_runtime::a_bounded_judge_failure_emits_one_redacted_category -- --exact --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring analyzer::tests::shadow_judge_runtime::a_bounded_judge_failure_emits_one_redacted_category -- --exact --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: judge-runtime-controls
    requirement: Shadow results remain non-authoritative and an unfinished judge is still cancelled without delaying analysis
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/analyzer.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring analyzer::tests::shadow_judge_runtime:: -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring analyzer::tests::shadow_judge_runtime:: -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: manual-resolve-fixture
    requirement: Apply completion wakes one real resolve-wait retry with its original manual deferral assertion intact
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/executor.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring parallel::tests::executor::test_manual_resolve_wait_retries_after_in_flight_apply_completes -- --exact --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring parallel::tests::executor::test_manual_resolve_wait_retries_after_in_flight_apply_completes -- --exact --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: merge-lane-fixture-isolation
    requirement: Competing base-lane fixtures share the actual test lock boundary while deliberate busy-lane controls still pass
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/executor.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring parallel:: -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring parallel:: -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: conflicted-status-fixture
    requirement: A locally identified fixture produces a genuine unmerged index before production status classifies it
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/vcs/git/commands/status_policy_repository_tests.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring vcs::git::commands::status_policy::native_git_status_optional_locks_repository::production_status_paths_preserve_conflicted_classification -- --exact --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring vcs::git::commands::status_policy::native_git_status_optional_locks_repository::production_status_paths_preserve_conflicted_classification -- --exact --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: final-commit-missing-identity-control
    requirement: Intentional missing-identity final-commit failures remain terminal and are not retried
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/execution/final_commit_lock_retry_git_tests.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring execution::final_commit_lock_retry::git_tests::final_apply_commit_lock_does_not_retry_configuration_failure -- --exact --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring execution::final_commit_lock_retry::git_tests::final_apply_commit_lock_does_not_retry_configuration_failure -- --exact --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: wip-missing-identity-control
    requirement: Intentional missing-identity WIP failures remain terminal and are not retried
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/execution/wip_lock_retry_git_tests.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring execution::wip_lock_retry::git_tests::transient_wip_commit_lock_does_not_retry_identity_failure -- --exact --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring execution::wip_lock_retry::git_tests::transient_wip_commit_lock_does_not_retry_identity_failure -- --exact --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: default-parallel-full-lib
    requirement: All default nonignored lib tests pass together with the harness default parallel scheduling
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: "cargo test --locked --offline --lib -- --nocapture"
    rerun: "cargo test --locked --offline --lib -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: web-monitoring-parallel-full-lib
    requirement: The explicit web-monitoring full-lib configuration passes with default parallel scheduling
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: "cargo test --locked --offline --lib --features web-monitoring -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Make the three pre-existing lib fixtures deterministic

**Change Type**: implementation

## Problem and confirmed evidence

The default-parallel full-lib observation at `f618a3e7c3d4afb4f8c8ff0fa1e5bc83658ab411` failed with 4855 passed, 3 failed, 36 ignored. The three failures are fixture preconditions, not permission to weaken product behavior. Read-only source inspection at proposal base `323a110abd0a41d62fd75292375908a71a6df9bc` confirms the same paths:

1. `analyzer::tests::shadow_judge_runtime::a_bounded_judge_failure_emits_one_redacted_category` waits for `prose-judge.request`. `write_fake_judge` creates that file **before** emitting the malformed stdout. Neither the file nor a marker written after stdout establishes completion of the Rust judge task and its drains. `JudgeHandle::collect_or_cancel` correctly cancels any task not yet finished. The isolated failure logged `outcome="cancelled"`, not `invalid_json`.
2. `parallel::tests::executor::test_manual_resolve_wait_retries_after_in_flight_apply_completes` already holds `merge_lock_test_mutex`. The production lane is nevertheless a different, process-global mutex. Other fixture callers reach it without holding the test mutex, including the post-archive pipeline test and dirty-to-clean retry in `executor.rs`, and the converging retry in `auto_resolve.rs`. Busy `attempt_merge` correctly returns an auto-resumable deferral before archive verification; the test requires a manual deferral. Exact/serial passes versus parallel-subset failure confirm parallel interference, but the historical log does **not** identify which competing test owned the lock.
3. `vcs::git::commands::status_policy::native_git_status_optional_locks_repository::production_status_paths_preserve_conflicted_classification` gives identity only to helper-created Git children, not its direct `git merge`. That command disables global/system Git config and fails before creating a conflict when no identity exists; checking only nonzero exit misclassifies setup failure as a conflict. The isolated test passes with identity. A separate disposable Git control confirms missing identity => exit 128 and empty status, repository-local identity => exit 1 with `UU conflict.txt` and unmerged stages 1/2/3.

The source/log locations and precise lock boundary are recorded in `design.md`. No repaired Rust test or full-suite pass is claimed by this proposal.

## Selected solution and observable outcome

- Add an opt-in, **per-analyzer-instance, `#[cfg(test)]` completion barrier**, enabled only by the malformed-judge fixture. After the conventional command completes and before the existing collection call, wait for `JudgeHandle::is_finished()` with cooperative task yields and a generous hang timeout. The existing collection/parser/redaction path still runs, with a real child and real malformed stdout. No sleeps, shell-marker completion inference, extra production wait, or changed cancellation rule.
- Use the existing **same process-global test mutex**, `crate::parallel::merge_lock_test_mutex`, at the actual lane-entering fixture boundary. Close the three named missing guard sites in `design.md`; retain a guard until awaited lane work / the spawned retry result has finished. Do not key by repository/change ID: the protected production lock is process-global. Do not acquire the production mutex around a call that tries to acquire it itself. Keep the default harness parallel, and leave tests that never enter the lane unguarded.
- Set `user.name` and `user.email` with `git config --local` **inside this conflict fixture's temporary repository** before creating its history/merge. Require Git's actual content-conflict exit code and unmerged index stages as setup assertions, then preserve `UU conflict.txt` and the production dirty verdict. Do not change shared Git helpers or either intentional missing-identity fixture.

## Boundary and preserved contracts

Implementation is confined to the target fixture bodies and the directly necessary test-only support described in `design.md`: `src/analyzer.rs` test module and explicitly `#[cfg(test)]` instance seam; `src/parallel/tests/executor.rs`; `src/parallel/tests/auto_resolve.rs`; and `src/vcs/git/commands/status_policy_repository_tests.rs`. No non-test analyzer, judge runner, scheduler, locking, Git status, retry, or identity semantics may change. `src/judge_command.rs`, the production `global_merge_lock`, and `attempt_merge` are read-only contract references, not implementation targets.

Preserve exact original test names and every substantive assertion; strengthening to exactly one failure diagnostic and verifying conflict setup is allowed. No ignore/skip/early-success path, assertion deletion/relaxation, suite-wide identity, process-global environment mutation, `--test-threads=1`, broad serial annotation, timing-based sleep, or new heavy-test classification is a fix. Existing intentional busy-lane behavior, slow-judge cancellation/reaping, no-authority shadow results, optional-lock suppression, missing-identity rejection and retry classification remain unchanged.

## Acceptance Criteria and Explicit Completion Conditions

1. The malformed-judge test still invokes `analyze_with_inflight` through the real conventional command and child judge; its opt-in barrier observes the actual handle finished before collection. It emits exactly one WARN failure-category record with `outcome="invalid_json"` and `expected_answers=2`, not `cancelled`, and exposes neither the secret nor an unvalidated model. The authoritative order/dependencies are unchanged. A stalled barrier fails the test, never counts as completion.
2. The original manual-resolve fixture still proves apply completion wakes the scheduler retry and produces `MergeDeferred(auto_resumable=false)` without accepting an automatic busy deferral instead. Named competitors use the same test guard with the lifetime specified in `design.md`. Deliberate contention tests still obtain the production lock under the outer test guard and assert `auto_resumable=true`.
3. The status fixture establishes a real content-conflicted index before testing production status. Both existing missing-identity negative tests pass unchanged, alone and in the full suite.
4. Every declared focused gate selects nonzero tests (each `--exact` gate selects exactly one); both full-lib gates execute all default nonignored tests with ordinary parallel scheduling and exit 0. No new ignores, filtering, global Git configuration or identity environment override may conceal a failure. `web-monitoring` is currently a default feature; both the default spelling and explicit-feature spelling remain required gates.
5. Conflux Acceptance owns the nine structured gates. Each runs once directly per candidate; full-lib is an explicitly required, bounded repository-local regression gate (the recorded default execution took 19 seconds, not a heavy/container suite), not a repetition/stability task for Apply. Gate artifacts stay in the existing external acceptance evidence store. Existing unrelated failures remain failures and are reported rather than repaired beyond this boundary.
6. Change-scoped diff review confirms only the above fixture/test-only surfaces and this proposal's specification artifacts changed. No historical hash of evolving production code becomes a permanent regression gate. Strict proposal validation is necessary but is not implementation evidence.

## Single-proposal rationale and dependencies

The user explicitly requests one dedicated fixture-hardening change. The three fixture repairs are small independent mechanics, but the requested acceptance unit is the same default-parallel full-lib failure set; keep them in one bounded test-only proposal, not mixed with product fixes. `dependencies: []`: no output from the queued-diagnostics or graceful-stop proposals is consumed. They are separate work; overlapping test files must be reconciled without overwriting their changes at later integration.

## Out of Scope

Product bug fixes, production lock re-keying, new scheduler policy, generalized fixture infrastructure, all-feature/heavy-test sweeps, suppressing unrelated suite failures, CI/global Git/environment changes, deployments, owner replacement, merging/pushing, selecting this change for execution, or manually implementing it during proposal authoring. Conflux implements after separate handoff.

The tracked `.pre-commit-config.yaml` is path-filtered for Rust fmt/clippy and contains no unconditional full-lib test gate; it cannot replace the declared commands. Normal commits must still run repository hooks. No constitution amendment, operational gate, migration, or external dependency is required.
