## Implementation Tasks

- [x] 1. 特性化テスト: 分割前に `cargo test --lib server::api` を実行し、全テストが通ることを記録する（verification: テスト結果をログとして保持）
- [x] 2. `src/server/api.rs` を `src/server/api/mod.rs` にリネームし、ビルドが通ることを確認する（verification: `cargo build` 成功）
- [x] 3. 共通ヘルパー (`error_response`, `now_rfc3339`, 型定義等) を `api/helpers.rs` に抽出する（verification: `cargo build` 成功、テスト全通過）
- [x] 4. プロジェクト CRUD ハンドラを `api/projects.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 5. Git sync 関連を `api/git_sync.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 6. グローバル制御 + change selection を `api/control.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 7. Worktree 操作を `api/worktrees.rs` を抽出する（verification: `cargo test` 全通過）
- [x] 8. ファイル操作を `api/files.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 9. ターミナルセッション管理を `api/terminals.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 10. プロポーザルセッション管理を `api/proposals.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 11. WebSocket ハンドラを `api/ws.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 12. ダッシュボード静的アセット配信を `api/dashboard.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 13. テストを各サブモジュール内 `#[cfg(test)]` に配置し直す（verification: `cargo test --lib server::api` 全通過）
- [x] 14. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- 各ハンドラのエラー型を統一する（別 proposal で扱う）

## Acceptance #1 Failure Follow-up

- [x] `test_stats_and_logs_endpoints_return_data` が `POST /api/v1/projects` で 201 を再び返すように修正し、`cargo test test_stats_and_logs_endpoints_return_data -- --nocapture` を再実行して通過を確認する
- [ ] `src/server/api/mod.rs` に残っている API テストを責務別サブモジュールへ移し、`src/server/api/mod.rs` にはルーター構築と共有ロジックのテストだけを残す

## Acceptance #2 Failure Follow-up

- [ ] `src/server/api/mod.rs` に残っている API 別テスト（auth / projects / files / git sync / worktrees / proposal session など）を対応するサブモジュールへ移し、`mod.rs` にはルーター構築と共有ロジックのテストだけを残す
- [ ] 変更をコミット可能な状態まで整理し、受け入れ確認時に `git status --porcelain` が空になるようにする

## Implementation Blocker #1
- category: other
- summary: `mod.rs` に集中している統合テストを責務別サブモジュールへ移管する際、共有テストヘルパーの公開範囲と所有先を決めないと重複/循環依存を回避できない
- evidence:
   - src/server/api/mod.rs:671 に巨大な `mod tests` が存在し、67件の API テストが `make_router` / `make_state` / Git テストヘルパーに密結合している
   - src/server/api/projects.rs:1 ほか各サブモジュールは `super::*` 前提で、現状 `#[cfg(test)]` 向け共通テストユーティリティが公開されていない
   - openspec/changes/refactor-split-server-api/tasks.md:29 のタスクは「対応するサブモジュールへ移す」ことを要求するが、共通ヘルパー配置方針が未定義
- impact: テスト移管を一括で実施すると、ヘルパー重複実装または `mod.rs` への逆依存が発生し、保守性とコンパイル安定性を損なう
- unblock_actions:
   - `src/server/api/test_support.rs`（`#[cfg(test)]`）を新設し、`make_state` / `make_router` / Git fixture helper の共通化方針を先に確定する
   - どのテストを「サブモジュール単位のユニット」にし、どれを「API ルーター統合テスト」として残すかを tasks.md で明示的に分割する
- owner: server-api maintainers
- decision_due: 2026-04-04

## Acceptance #3 Failure Follow-up

- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を対応するサブモジュールまたは共通 `test_support` に移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `tasks.md` の完了状態を実装実態と一致させ、`Acceptance #1 Failure Follow-up` の完了済みチェックを誤表示しないよう修正する

## Acceptance #4 Failure Follow-up

- [ ] `src/server/api/mod.rs` に残っている API 別テスト（auth / projects / files / proposal session など）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む `tasks.md` の完了状態を実装実態と一致させ、未完了のテスト移管タスクを `[x]` のままにしない
- [ ] 受け入れ確認前に変更を整理し、`git status --porcelain` が空の状態で再度 acceptance を実行する

## Acceptance #5 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで未コミット変更を整理してから受け入れ確認を再実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を対応するサブモジュールまたは共通 `test_support` に移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `tasks.md` の `Acceptance #1 Failure Follow-up` で完了扱いになっているテスト移管項目を実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #6 Failure Follow-up

- [ ] `git status --porcelain` が空の状態になるまで `tasks.md` を含む未コミット変更を整理してから再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（auth / projects / files / proposal session など）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` の完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #7 Failure Follow-up

- [ ] `git status --porcelain` が空の状態になるまで `tasks.md` を含む未コミット変更を整理してから再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` の完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #8 Failure Follow-up

- [ ] `git status --porcelain` が空の状態になるまで `tasks.md` を含む未コミット変更を整理してから再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` の完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #9 Failure Follow-up

- [ ] `git status --porcelain` が空の状態になるまで `tasks.md` を含む未コミット変更を整理してから再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` の完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #10 Failure Follow-up

- [ ] `git status --porcelain` が空の状態になるまで `tasks.md` を含む未コミット変更を整理してから再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / files / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` の完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と証拠を一致させる

## Acceptance #11 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] pre-commit 相当ゲート（`prek run --all-files` または documented equivalent）が `openspec/changes/refactor-split-server-api/tasks.md` を自動修正しない状態まで整え、hook が fail せず完了することを確認する
- [x] `src/server/api/control.rs` の `#[cfg(test)] mod tests` をファイル末尾へ移動するか本体アイテムを test module より前へ再配置し、`cargo clippy -- -D warnings` の `clippy::items-after-test-module` を解消する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と実ファイル上の証拠を一致させる

## Acceptance #12 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] pre-commit 相当ゲート（`prek run --all-files` または documented equivalent）に加えて `cargo fmt --check` を通し、通常のコミット前品質ゲートがすべて成功する状態を確認する
- [x] `src/server/api/control.rs` の `#[cfg(test)] mod tests` をファイル末尾へ移動するか本体アイテムを test module より前へ再配置し、`cargo clippy -- -D warnings` の `clippy::items-after-test-module` を解消する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / projects / proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、チェックリスト表示と実ファイル上の証拠を一致させる
- [ ] `cargo test` を通し、少なくとも `src/config/defaults.rs` の `test_cleanup_old_logs_retains_exactly_n_days` と `src/config/mod.rs` の `test_hooks_deep_merge` が失敗しない状態を確認する

## Acceptance #13 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` をファイル末尾へ移動するか本体アイテムを test module より前へ再配置し、`cargo clippy -- -D warnings` と `prek run --all-files` の clippy failure を解消する

## Acceptance #14 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` をファイル末尾へ移動するか本体アイテムを test module より前へ再配置し、通常のコミット前 hook（`prek run --all-files`）が成功することを確認する

## Acceptance #15 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、`cargo clippy -- -D warnings` と `prek run --all-files` の `clippy::items-after-test-module` failure を解消する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #16 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、通常のコミット前 hook（`prek run --all-files`）が成功することを確認する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #17 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、`prek run --all-files` の `clippy::items-after-test-module` failure を解消する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #18 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、通常のコミット前 hook（`prek run --all-files`）が成功することを確認する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #19 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、通常のコミット前 hook（`prek run --all-files`）が成功することを確認する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #20 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも auth / proposal session / projects 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` 表示しない
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、`prek run --all-files` の `clippy::items-after-test-module` failure を解消する
- [ ] `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を archive 前品質ゲートとして再実行し、すべて成功した証拠を確認する

## Acceptance #21 Failure Follow-up

- [ ] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する
- [ ] `src/server/api/control.rs` の `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、`cargo clippy -- -D warnings` と `prek run --all-files` の `clippy::items-after-test-module` failure を解消する
- [x] `tests/no_backup_files_test.rs` の `heavy_real_boundary_suites_stay_feature_gated` が分割後の `src/server/api/mod.rs` もしくは対応サブモジュール群を参照するよう更新し、削除済みの `src/server/api.rs` を読まない状態で `cargo test --test no_backup_files_test` を通す
- [ ] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも `test_add_project_without_repo_root_setup_succeeds_without_marker` / `test_add_project_setup_failure_returns_422_and_rolls_back_registry` / `test_app_state_resolve_command_comes_from_top_level_config` を含む auth・projects・proposal session 由来）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [ ] `Acceptance #1 Failure Follow-up` を含む完了済みチェックを実装実態に合わせて修正し、テスト移管未完了の項目を `[x]` のまま残さない
- [ ] `test_global_control_run_records_call` / `test_projects_state_includes_sync_metadata_fields_after_monitor_refresh` / `test_toggle_all_change_selection_remarks_error_changes_for_next_run` が再び通るように修正し、`cargo test` と archive 前品質ゲート（`cargo fmt --check` / `cargo clippy -- -D warnings` / `prek run --all-files`）を再実行してすべて成功した証拠を確認する

## Acceptance #22 Failure Follow-up

- [x] `git status --porcelain` が空になるまで `openspec/changes/refactor-split-server-api/tasks.md` を含む未コミット変更を整理し、クリーンな作業ツリーで再度 acceptance を実行する（`git status --porcelain` が空であることを確認済み）
- [x] `src/server/api/control.rs` で `#[cfg(test)] mod tests` より後ろにある `list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ移動し、`cargo clippy -- -D warnings` と `prek run --all-files` の `clippy::items-after-test-module` failure を解消する
- [x] `src/server/api/mod.rs` に残っている API 別テスト（少なくとも `test_add_project_without_repo_root_setup_succeeds_without_marker` / `test_add_project_setup_failure_returns_422_and_rolls_back_registry` / `test_global_control_run_records_call` / `test_projects_state_includes_sync_metadata_fields_after_monitor_refresh` / `test_toggle_all_change_selection_remarks_error_changes_for_next_run` / `test_app_state_resolve_command_comes_from_top_level_config`）を責務別サブモジュールまたは共通 `test_support` へ移し、`mod.rs` にはルーター構築・共有ロジックのテストだけを残す
- [x] `Acceptance #1 Failure Follow-up` と実装タスク 13 の完了状態を実装実態に合わせて修正し、`src/server/api/mod.rs` に API 別テストが残っている間はテスト移管完了を `[x]` のまま残さない
- [x] `server::api::tests::test_list_worktrees_with_real_project` が再び通るよう修正し、`cargo test --lib server::api` を再実行して全体成功を確認する
- [x] archive 前品質ゲートとして `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` を再実行し、すべて成功した証拠を確認する（再検証で 4 コマンドすべて通過）

## Rejecting Recovery Tasks

- [ ] Investigate blocker in openspec/changes/refactor-split-server-api/REJECTED.md and implement a non-rejection recovery path before rerunning apply
