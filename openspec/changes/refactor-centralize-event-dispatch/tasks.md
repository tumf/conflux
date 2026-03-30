## 1. 事前検証（Characterization Tests）
- [ ] 1.1 Web `apply_execution_event()` の全 match arm をカバーする characterization test を追加（ProcessingStarted, AcceptanceStarted, ChangeArchived, MergeCompleted, MergeDeferred, ResolveStarted, ResolveFailed の少なくとも7パス）
- [ ] 1.2 TUI `handle_event()` のステータス遷移パスをカバーする characterization test を追加
- [ ] 1.3 Reducer `apply_execution_event()` との結果一致を検証するクロスチェックテストを追加（同一イベントシーケンスで TUI/Web/Reducer が同じ最終ステータスになることの確認）
- [ ] 1.4 全テスト（`cargo test`）通過を確認

## 2. イベントディスパッチの一方向化
- [ ] 2.1 `tui/orchestrator.rs` および `orchestrator.rs` 内の `apply_execution_event` 3重呼び出しパターンを「Reducer のみに送信 → フロントエンドに変更通知」パターンに変更
- [ ] 2.2 Reducer のステート変更後にフロントエンド（TUI/Web）へ通知する仕組みを導入（例: Reducer 更新後に `StateChanged` 通知を broadcast）
- [ ] 2.3 全テスト通過を確認

## 3. Web apply_execution_event のステータス遷移削除
- [ ] 3.1 `WebState::apply_execution_event()` からステータス書き換え match arm（`queue_status = Some(...)` の全箇所）を削除
- [ ] 3.2 `ChangeStatus` の `queue_status` を Reducer の `display_status()` から生成するよう `update_from_shared_state()` メソッドを追加
- [ ] 3.3 ログ追加・worktree 更新など UI 固有処理のみ `apply_execution_event()` に残す
- [ ] 3.4 全テスト通過を確認

## 4. TUI handle_event のステータス遷移委譲
- [ ] 4.1 `AppState::handle_event()` 内のステータス書き換え（`queue_status = QueueStatus::...`）箇所を特定
- [ ] 4.2 ステータス遷移部分を削除し、Reducer 更新通知後に `apply_display_statuses_from_reducer()` で反映するパターンに統一
- [ ] 4.3 ログ追加、UI モード変更、elapsed_time 計測など UI 固有処理のみ残す
- [ ] 4.4 全テスト通過を確認、`cargo clippy -- -D warnings` クリア
