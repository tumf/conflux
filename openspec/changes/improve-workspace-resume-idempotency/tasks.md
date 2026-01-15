# Tasks: Improve Workspace Resume Idempotency

## Phase 1: 状態検出モジュール実装

- [x] Task 1.1: `WorkspaceState` enum 定義 (`src/execution/state.rs`)
- [x] Task 1.2: `is_merged_to_main()` 実装
- [x] Task 1.3: `get_latest_wip_snapshot()` 実装
- [x] Task 1.4: `has_apply_commit()` 実装
- [x] Task 1.5: `detect_workspace_state()` 実装
- [x] Task 1.6: Unit tests 追加（11テスト）

## Phase 2: Resume処理修正

- [x] Task 2.1: `src/execution/mod.rs` にモジュール追加
- [x] Task 2.2: `execute_group()` の状態検出追加
- [x] Task 2.3: `WorkspaceState::Merged` の処理実装
- [x] Task 2.4: `WorkspaceState::Archived` の処理修正
- [x] Task 2.5: `WorkspaceState::Applied` の処理実装
- [x] Task 2.6: `WorkspaceState::Applying` の処理実装
- [x] Task 2.7: `WorkspaceState::Created` の処理実装

## Phase 4: Documentation

- [x] Task 4.1: README更新（状態遷移図追加）
- [x] Task 4.2: AGENTS.md更新

## 検証タスク

- [x] Task V.1: 全テスト実行 (`cargo test`, `cargo clippy`, `cargo fmt`)

## Future Work

E2E tests and manual testing are deferred for future implementation:
- Task 3.1: E2E test - Apply中断後の再開
- Task 3.2: E2E test - 手動Archive後の再開
- Task 3.3: E2E test - 手動Merge後の再開
- Task 3.4: E2E test - 冪等性検証
- Task V.2: 手動検証（実際のワークスペースでテスト）
