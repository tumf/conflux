## Implementation Tasks

- [x] Preserve dependency data through all one-change analysis paths. (verification: unit - `cargo test analyzer --lib` includes a single-change fast-path test showing `AnalysisResult.dependencies` contains `route -> policy` without executing `analyze_command`; completion condition: one-change LLM skipping remains an optimization only for inference, not for metadata dependency retention.)
- [x] Ensure scheduler dispatch gating runs for a lone queued change before apply can start. (verification: unit - focused `select_changes_for_dispatch` test in `src/parallel/tests/executor.rs` or equivalent constructs a one-change analysis result with `route -> policy` and asserts no selected dispatch when `policy` is unresolved; completion condition: `route` is not returned by dispatch selection while unresolved blockers remain.)
- [x] Classify active-but-not-queued dependency targets as blocking repository-local dependencies instead of allowing the dependent change to proceed. (verification: unit - add or update a focused test in `src/parallel/tests/executor.rs` that creates `openspec/changes/policy/proposal.md` while only `route` is queued, then asserts dependency-blocked behavior via `cargo test active_not_queued --lib`; completion condition: the diagnostic identifies `policy` and apply is not eligible until base-branch resolution evidence exists.)
- [x] Preserve satisfied archived dependency behavior for the single queued path. (verification: unit - add or update a focused test in `src/parallel/tests/executor.rs` that places `policy` under `openspec/changes/archive/` and asserts `route` can be selected when no other blockers exist via `cargo test archived_dependency --lib`; completion condition: archived dependencies remain satisfied and are not reported as missing.)
- [x] Fail closed for missing, rejected, terminal-error, and in-flight dependencies in the single queued path. (verification: unit - add or update focused tests in `src/parallel/tests/executor.rs` that assert missing/rejected/in-flight blockers prevent selection and emit or record dependency-blocked diagnostics via `cargo test dependency_block --lib`; completion condition: no blocked dependency class can lead to `ApplyStarted` before resolution.)
- [x] Add an integration-style regression test for scheduler events proving a lone queued dependent change does not emit `ApplyStarted`. (verification: integration - add or update a Rust async test in `src/parallel/tests/executor.rs` or `tests/` that observes emitted events for `route -> policy` with `policy` active/unmerged and asserts `DependencyBlocked` occurs while `ApplyStarted` does not; run with `cargo test single_queued --lib`; completion condition: the test would fail if apply starts before dependency resolution.)
- [x] Run focused and default repository verification. (verification: manual - run `cargo test analyzer --lib`, the focused `cargo test single_queued --lib` / dependency gating tests, `cargo test --lib`, and any configured lint/typecheck command documented in `AGENTS.md` or project config; completion condition: commands pass or any unrelated pre-existing failure is documented with exact output.)

## Future Work

- Consider improving user-facing labels for active-but-not-queued dependencies if the existing diagnostic wording is insufficient after the gating fix.

## Final Validation

Expected archive gate: `cflx openspec validate fix-single-queued-dependency-gating --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] コミットパスの pre-commit hook が失敗します。実行コマンド: `$(git rev-parse --git-path hooks/pre-commit)`。hook 内の clippy が `-D warnings` により `src/openspec_cmd.rs:1034` と `src/openspec_cmd.rs:1163` の `[].into_iter()` を `clippy::useless_conversion` としてエラーにしていたため、空の queued dependency iterator を `std::iter::empty::<&str>()` に置き換えました。検証: pre-commit hook と最終検証コマンドを再実行する。
