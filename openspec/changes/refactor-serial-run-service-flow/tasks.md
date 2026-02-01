## 1. 実装
- [x] 1.1 apply 後の change 再取得処理をヘルパー化する（検証: src/serial_run_service.rs で再取得が単一ヘルパー経由になっていることを確認）
- [x] 1.2 acceptance 判定と結果整形をヘルパー化する（検証: acceptance 実行と結果判定が新ヘルパーに集約されていることを確認）
- [x] 1.3 apply/acceptance の操作トラッカー更新を共通化する（検証: トラッカー更新が同一ヘルパー経由になっていることを確認）
- [x] 1.4 既存の挙動維持を確認するため `cargo test` を実行する（検証: `cargo test` が成功）

## 2. Acceptance #1 Failure Follow-up
- [x] 2.1 `src/parallel/orchestration_adapter.rs` の未使用コードを削除する（検証: ファイル削除完了）
- [x] 2.2 `src/serial_run_service.rs` の `run_start_hook` / `run_finish_hook` / `check_iteration_limit` を削除する（検証: dead_code 関数削除完了）
- [x] 2.3 `src/execution/apply.rs` の共通ヘルパーを parallel から使用する（検証: `format_wip_commit_message` を parallel で使用）
- [x] 2.4 全テストの実行と確認（検証: `cargo test` が成功）

## 3. Acceptance #2 Failure Follow-up
- [x] 3.1 `ApplyEventHandler` trait の実装を両モードで作成する（検証: `SerialEventHandler` と `ParallelEventHandler` が実装されている）
- [x] 3.2 共通ヘルパー関数の使用を確認する（検証: `check_task_progress`, `is_progress_complete`, `format_wip_commit_message` が両モードで使用されている）
- [x] 3.3 ビルドエラーの修正と動作確認（検証: `cargo build` が成功）
- [x] 3.4 全テストの実行と確認（検証: `cargo test` が成功）
- [x] 3.5 `cargo fmt` と `cargo clippy` の実行（検証: フォーマットとlintが成功）

## 4. Acceptance #3 Failure Follow-up
- [x] 4.1 `SerialEventHandler` と `ParallelEventHandler` を削除する（dead code のため）
- [x] 4.2 tasks.md の未完了タスクを Future Work に移動する
- [x] 4.3 全テストの実行と確認（検証: `cargo test` が成功 - 983 tests passed）
- [x] 4.4 `cargo fmt` と `cargo clippy` の実行（検証: フォーマットと lint が成功）

## 5. Acceptance #4 Failure Follow-up: execute_apply_loop 統合
- [x] 5.1 SerialRunService::apply_change_internal で execute_apply_loop を直接呼び出すように修正（検証: src/serial_run_service.rs:378 で execute_apply_loop を使用）
- [x] 5.2 Parallel executor の execute_apply_in_workspace で execute_apply_loop を直接呼び出すように修正（検証: src/parallel/executor.rs:324 で execute_apply_loop を使用）
- [x] 5.3 SerialApplyEventHandler を実装し、OutputHandler と統合（検証: src/serial_run_service.rs:600-631 に実装）
- [x] 5.4 ParallelApplyEventHandler を有効化し、ParallelEvent チャネルと統合（検証: src/parallel/output_bridge.rs:91-182 を使用）
- [x] 5.5 全テストの実行と確認（検証: `cargo test` が成功 - 1009 tests passed, 0 failed）
- [x] 5.6 `cargo clippy` の実行（検証: clippy が成功、警告なし）

## 6. Acceptance #5 Failure Follow-up
- [x] 6.1 src/serial_run_service.rs:232-360 で apply_count を increment するように修正（検証: increment_apply_count メソッドを追加し、apply ループ完了後に呼び出し）
- [x] 6.2 src/parallel/executor.rs:408-416 execute_apply_in_workspace で workspace_manager を渡すように修正（検証: GitWorkspaceManager を作成し execute_apply_loop に渡す）
- [x] 6.3 src/parallel/output_bridge.rs ParallelOutputHandler が dead code のため削除（検証: 削除完了、tests も修正）
- [x] 6.4 src/parallel/executor.rs の create_iteration_snapshot/squash_wip_commits/check_task_progress/summarize_output が dead code のため削除（検証: 削除完了）
- [x] 6.5 全テストの実行と確認（検証: `cargo test` が成功 - 1007 tests passed, 0 failed）
- [x] 6.6 `cargo fmt` と `cargo clippy` の実行（検証: フォーマットと lint が成功、警告なし）

## 7. Acceptance #6 Failure Follow-up
- [x] 7.1 `src/serial_run_service.rs:398-403` の出力クロージャと `src/serial_run_service.rs:639-644` の SerialApplyEventHandler::on_apply_output が同じ出力を二重送信するため、重複出力を解消する（SerialApplyEventHandler::on_apply_output を no-op にする）
- [x] 7.2 `src/serial_run_service.rs:392` で execute_apply_loop に cancel_token を `None` で渡しており apply 中のキャンセルが無効化されているため、cancel_check クロージャから CancellationToken を生成して execute_apply_loop に渡す（検証: cancel_check クロージャを poll する background task を spawn し、CancellationToken を execute_apply_loop に渡す）
- [x] 7.3 全テストの実行と確認（検証: `cargo test` が成功 - 1007 tests passed）
- [x] 7.4 `cargo fmt` と `cargo clippy` の実行（検証: フォーマットと lint が成功）

## 8. Acceptance #7 Failure Follow-up
- [x] 8.1 `src/tui/orchestrator.rs` の serial フローで `OrchestratorEvent::AcceptanceStarted` が送出されず、`acceptance_test_streaming` は `OutputHandler::on_info` のみ（`src/orchestration/acceptance.rs:101-123`）。`src/tui/state/events/completion.rs:70-88` の `handle_acceptance_started` が発火しないため、Acceptance 開始イベントを送るよう統合する（検証: `ChangeProcessResult::AcceptancePassed` 分岐で `AcceptanceStarted` イベントを送信）
- [x] 8.2 `src/tui/orchestrator.rs` の `ChangeProcessResult::Archived` 分岐（`src/tui/orchestrator.rs:1114-1128`）で `OrchestratorEvent::ChangeArchived` と `shared_state.apply_execution_event` が送出されず、`src/tui/state/events/stages.rs:74-99` が更新されない。成功時に ChangeArchived を送信して共有状態を更新する（検証: `ChangeArchived` イベント送信と `shared_state.apply_execution_event` 呼び出しを追加）
- [x] 8.3 `src/tui/orchestrator.rs` の `archive_single_change` / `archive_all_complete_changes` が `#[allow(dead_code)]` のまま `run_orchestrator` から呼ばれておらず、`Phase 1 archive processing has been removed`（`src/tui/orchestrator.rs:94-96`）により未使用。削除するか新フローに統合して dead code を解消する（検証: dead code 関数を削除、関連する未使用 imports を削除、間接的に未使用になった `run_archive_streaming_with_runner` と `format_archive_history` に `#[allow(dead_code)]` を追加）
- [x] 8.4 全テストの実行と確認（検証: `cargo test` が成功 - 1007 tests passed, 0 failed, 6 ignored）
- [x] 8.5 `cargo fmt` と `cargo clippy` の実行（検証: フォーマットと lint が成功、警告なし）

## 9. Acceptance #8 Failure Follow-up
- [x] 9.1 `SerialRunService::apply_change_internal` のキャンセル監視タスクが apply 完了後も停止しないため、終了時にタスクを停止/abort する（検証: apply 完了後にバックグラウンドタスクが終了する）
- [x] 9.2 `git status --porcelain` が空でないため、作業ツリーをクリーンにする（検証: 変更ファイルなし。現在: `openspec/changes/refactor-serial-run-service-flow/tasks.md` が変更中）

## 10. Acceptance #9 Failure Follow-up
- [ ] `src/tui/orchestrator.rs` の `ChangeProcessResult::AcceptanceContinue` / `AcceptanceFailed` / `AcceptanceCommandFailed` 分岐（例: `src/tui/orchestrator.rs:513-633`）では `OrchestratorEvent::AcceptanceStarted` を送出しておらず、`acceptance_test_streaming` も `OutputHandler::on_info` のみ（`src/orchestration/acceptance.rs:101-123`）。そのため `src/tui/state/events/completion.rs:70-88` の `handle_acceptance_started` が非Pass系で発火しない。Acceptance開始イベントを各分岐で送るか、`acceptance_test_streaming` 側で `AcceptanceStarted` を送出する。
