## 実装タスク

- [x] 1. `ExecutionMode` enum (`Serial` | `Parallel`) を `src/orchestration/state.rs` に追加 (検証: `cargo test --lib orchestration::state`)
- [x] 2. `OrchestratorState` に `execution_mode` フィールドと `with_mode()` コンストラクタを追加 (検証: `OrchestratorState::with_mode(ids, 0, ExecutionMode::Parallel)` がコンパイル可能)
- [x] 3. `apply_execution_event()` の `ChangeArchived` ハンドラをモード分岐: Serial→Archived terminal, Parallel→MergeWait (検証: 新規テスト通過)
- [x] 4. `orchestrator.rs::run_parallel()` で `OrchestratorState` を `ExecutionMode::Parallel` で初期化 (検証: grep で `with_mode` 呼び出し確認)
- [x] 5. `tui/orchestrator.rs::run_orchestrator_parallel()` で `OrchestratorState` を `ExecutionMode::Parallel` で初期化 (検証: grep で `with_mode` 呼び出し確認)
- [x] 6. パラレルモードのライフサイクルテスト追加: archive → merge wait → merged (検証: `cargo test --lib orchestration::state`)
- [x] 7. シリアルモードの既存動作維持テスト追加: archive が terminal (検証: `cargo test --lib orchestration::state`)
- [x] 8. パラレルモードの遅延イベント非退行テスト追加 (検証: `cargo test --lib orchestration::state`)
- [x] 9. 全既存テスト通過確認 (検証: `cargo test`)
- [x] 10. clippy 通過確認 (検証: `cargo clippy -- -D warnings`)
- [x] 11. `orchestration-state` スペックを更新し、モード依存の ChangeArchived セマンティクスを反映

## Acceptance #1 Failure Follow-up

- [x] 全変更ファイル (`src/orchestration/state.rs`, `src/orchestrator.rs`, `src/tui/orchestrator.rs`) およびプロポーザルディレクトリ (`openspec/changes/fix-parallel-archived-terminal-state/`) を git commit する
