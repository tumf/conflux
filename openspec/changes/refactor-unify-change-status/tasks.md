## 1. 事前検証（Characterization Tests）
- [ ] 1.1 TUI `ChangeState` のステータス遷移をカバーする characterization test を追加（Applying → Accepting → Archiving → Archived の典型パス）
- [ ] 1.2 Web `apply_execution_event()` のステータス遷移をカバーする characterization test を追加（ProcessingStarted → AcceptanceStarted → ChangeArchived の典型パス）
- [ ] 1.3 `apply_display_statuses_from_reducer()` のダウングレード防止ロジックの characterization test を追加
- [ ] 1.4 全テスト（`cargo test`）が通ることを確認

## 2. TUI QueueStatus enum の廃止
- [ ] 2.1 `tui::state::ChangeState` から `queue_status: QueueStatus` フィールドを削除し、`display_status()` を Reducer から取得するメソッドに置き換え
- [ ] 2.2 `tui::types::QueueStatus` enum を削除
- [ ] 2.3 `apply_remote_status()` 関数を削除（Reducer からの読み取りで不要になるため）
- [ ] 2.4 `apply_display_statuses_from_reducer()` をシンプルな Reducer 参照に置き換え
- [ ] 2.5 TUI render.rs 内の `QueueStatus` 参照を `display_status()` 文字列ベースの分岐に変更
- [ ] 2.6 全テスト通過を確認

## 3. Web ChangeStatus のステータス導出化
- [ ] 3.1 `web::state::ChangeStatus.queue_status` を Reducer の `display_status()` から導出するよう変更
- [ ] 3.2 `web::state::apply_execution_event()` 内のステータス書き込み match arm を削除（ステータスは Reducer 側が管理済み）
- [ ] 3.3 `ChangesRefreshed` イベントハンドラの `queue_status` 保存ロジックを簡素化
- [ ] 3.4 全テスト通過を確認

## 4. TUI ダブルライト箇所の排除
- [ ] 4.1 `resolve_merge()` 内の `change.queue_status = QueueStatus::ResolveWait` 直接代入を削除し、Reducer コマンド発行のみにする
- [ ] 4.2 `apply_parallel_eligibility()` 内の `queue_status` 直接書き換えを Reducer コマンド経由に変更
- [ ] 4.3 他の TUI 内で `queue_status` を直接書き換えている箇所を洗い出し、Reducer 経由に統一
- [ ] 4.4 全テスト通過を確認、`cargo clippy -- -D warnings` クリア
