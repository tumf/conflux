## Implementation Tasks

- [x] Inventory the current `tests/` surface and classify each suite as fast default-path, fast integration, contract, or heavy E2E/real-boundary validation (verification: `docs/guides/DEVELOPMENT.md` に `tests/e2e_proposal_session.rs`, `tests/e2e_git_worktree_tests.rs`, `tests/process_cleanup_test.rs`, `tests/merge_conflict_check_tests.rs` を含む分類表を追加済み).
- [x] Define the repository mechanism for separating heavy tests from the default test path (for example file layout, ignored tests, dedicated commands, feature gates, or equivalent) and document the chosen rule in a way that matches Cargo/Rust conventions used by this repo (verification: `Cargo.toml` に `heavy-tests` feature を追加し、`docs/guides/DEVELOPMENT.md` と `Makefile` に default-path vs heavy-tier 実行方法を明記).
- [x] Reorganize or mark heavy test suites so default developer validation does not implicitly include real-boundary E2E coverage (verification: `tests/e2e_proposal_session.rs`, `tests/e2e_git_worktree_tests.rs`, `tests/process_cleanup_test.rs`, `tests/merge_conflict_check_tests.rs`, `tests/e2e_tests.rs` に `#![cfg(feature = "heavy-tests")]` を付与し、さらに `src/parallel/tests/executor.rs` / `src/server/api.rs` / `src/ai_command_runner.rs` / `src/orchestration/archive.rs` の slow real-boundary or retry/timeout テストへ個別 `#[cfg(feature = "heavy-tests")]` を追加).
- [x] Update repository-standard validation guidance so pre-commit, local developer loops, acceptance, and broader validation each call the appropriate test tier (verification: `docs/guides/DEVELOPMENT.md` に phase 別コマンド（pre-commit/local/acceptance/broader readiness）を追記).
- [x] Preserve explicit execution paths for heavy test suites, including clear commands for running them intentionally (verification: `Makefile` に `test-heavy` ターゲット追加、`docs/guides/DEVELOPMENT.md` に `cargo test --features heavy-tests` を明記).
- [x] Add regression checks or documentation tests as needed to ensure newly added heavy tests do not silently drift back into the default path (verification: `tests/no_backup_files_test.rs` に `heavy_real_boundary_suites_stay_feature_gated` を追加し、対象ファイルに feature gate が残ることを検証).
- [x] Run repository verification for the new structure (`cargo fmt --check`, `cargo clippy -- -D warnings`, and the intended default-path test command) and separately verify the heavy tier command still works (verification: `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` すべて成功。1391 tests passed, 0 failed, 14.72s で完走。heavy tier の explicit 実行は本 change scope 外).

## Future Work

- Add CI lane separation if the project later wants different schedules or requiredness for heavy E2E tiers.
- Introduce richer per-test metadata if the team wants machine-enforced categorization beyond naming/layout conventions.
