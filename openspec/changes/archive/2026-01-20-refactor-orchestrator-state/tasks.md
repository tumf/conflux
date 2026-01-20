## 1. 共有ステートの基盤実装（前回完了分）
- [x] 1.1 `src/orchestration/state.rs` にExecutionEventの更新関数 `apply_execution_event` を追加し、状態更新の単一経路を実装する
- [x] 1.2 `src/web/state.rs` のDTOを `OrchestratorStateSnapshot` にリネームし、名称衝突を解消する
- [x] 1.3 TUIのstate documentationを更新し、共有ステートとの関係を文書化する

## 2. 動作確認（前回完了分）
- [x] 2.1 `cargo test` を実行して既存テストが通ることを確認する（`cargo test` の成功ログを確認）


## 3. 共有ステートの実運用フローへの統合
- [x] 3.1 `src/orchestrator.rs` に共有ステートのインスタンス（`shared_state: OrchestratorState`フィールド）を追加
- [x] 3.2 `src/orchestrator.rs` の `new()` および `with_config()` で共有ステートを初期化（空のchange_idsで仮初期化）
- [x] 3.3 `src/orchestrator.rs` の `run()` メソッドでフィルタリング後のchange_idsで共有ステートを再初期化（line 290）
- [x] 3.4 `src/orchestrator.rs` のrunループで、ExecutionEvent発行時に `shared_state.apply_execution_event` を呼び出し：
  - ProcessingStarted: line 379（新しいchangeの処理開始時）
  - ApplyStarted: line 492（apply操作開始時）
  - ApplyCompleted: line 512（apply操作完了時、apply_count更新）
  - ChangeArchived: line 432（archive成功時、pending→archivedへ移動）
- [x] 3.5 `src/orchestration/state.rs` のドキュメントを更新し、統合状況と使用方法を明記
- [x] 3.6 統合後、`cargo test` を実行して全863テストが通ることを確認（833 + 25 + 2 + 3）


## 4. TUI/Web Integration with Shared State
- [x] 4.1 Add `shared_orchestrator_state` field to `WebState` (Option<Arc<RwLock<OrchestratorState>>>)
- [x] 4.2 Add `set_shared_state()` method to `WebState` for injecting shared state reference
- [x] 4.3 Update `OrchestratorStateSnapshot::from_changes` to optionally accept shared state and populate apply counts from it (added `from_changes_with_shared_state` method)
- [x] 4.4 Inject shared state into WebState: wrapped shared_state in Arc<RwLock<>>, updated Orchestrator.set_web_state to inject reference, updated main.rs call site with await
- [x] 4.5 Add `shared_orchestrator_state` field to TUI `AppState` (Option<Arc<RwLock<OrchestratorState>>>)
- [x] 4.6 Add `set_shared_state()` method to TUI AppState for injecting shared state reference
- [x] 4.7 Update documentation in `src/orchestration/state.rs` to reflect TUI/Web integration patterns (updated Integration Status and added Usage/Integration Pattern sections)
- [x] 4.8 Verify `cargo test` passes after integration (863 tests: 833 + 25 + 2 + 3, all passing)


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ~~ACCEPTANCE: FAIL~~ → **RESOLVED**
  2) ~~FINDINGS:~~ → **全て修正完了**
  3) - ~~TUIの変更一覧が共有ステート由来になっていません~~ → **修正完了**: `src/tui/runner.rs` の `run_tui_loop` で共有ステート初期化し、`AppState::set_shared_state` 経由で注入
  4) - ~~Webのスナップショットが共有ステート由来になっていません~~ → **修正完了**: TUI起動時に `WebState::set_shared_state` を呼び出し、共有ステートを注入
  5) - ~~共有ステートの実運用統合がTUI/Webで欠落しています~~ → **修正完了**: `src/tui/orchestrator.rs` の `run_orchestrator` と `run_orchestrator_parallel` で全ExecutionEventを共有ステートに適用 (ProcessingStarted, ApplyStarted, ApplyCompleted, ProcessingError, ChangeArchived)


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  - ~~ACCEPTANCE: FAIL~~ → **RESOLVED**
  - ~~FINDINGS:~~ → **修正完了**
  - - ~~`src/tui/state/events/helpers.rs:17` `AppState::update_changes()` が `fetched_changes` と `task_parser` の値だけで `completed_tasks/total_tasks` を更新しており、`self.shared_orchestrator_state` を参照しません。~~ → **修正完了**: `update_changes()` で `shared_orchestrator_state` を参照し、apply_count（iteration_number）を各変更に反映するように実装。共有ステートが利用可能な場合は `try_read()` で読み取り、apply_count > 0 の変更に `iteration_number` を設定。
  - - ~~`src/tui/state/mod.rs:114` の `shared_orchestrator_state` は `AppState::set_shared_state()` で設定されるだけで参照箇所がありません（`src/tui/runner.rs:276` の `run_tui_loop` で注入はされるが未使用）。~~ → **修正完了**: `src/tui/state/events/helpers.rs` の `update_changes()` メソッドで実際に `shared_orchestrator_state` を参照してメタデータ（apply_count）を取得するように実装。共有ステートがTUI更新フローで実際に消費される。


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  - ~~ACCEPTANCE: FAIL~~ → **RESOLVED**
  - ~~FINDINGS:~~ → **修正完了**
  - - ~~`src/web/state.rs:278` (WebState::update) builds the snapshot with `from_changes_with_shared_state`, but `src/web/state.rs:292` and `src/web/state.rs:297` immediately overwrite `queue_status`/`iteration_number` with existing values, discarding shared-state-derived data so the Web snapshot no longer derives from the shared orchestration state required by `openspec/changes/refactor-orchestrator-state/specs/web-monitoring/spec.md`.~~ → **修正完了**: `src/web/state.rs:293-301` で条件付き保存ロジックに変更。共有ステートが `queue_status` と `iteration_number` を提供した場合（`Some` 値）はそれを優先し、`None` の場合のみ古いキャッシュ値にフォールバックする。これにより、共有オーケストレーションステート由来のデータが優先され、仕様要件を満たすようになった。全863テスト合格（833 + 25 + 2 + 3）。
