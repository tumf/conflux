## 1. server ログ配信

- [x] 1.1 runner の stdout/stderr を LogEntry に変換して server の配信キューに追加する（`src/server/runner.rs` に変換処理が追加されていることを確認）
- [x] 1.2 server の WebSocket が Log イベントを配信できるようにする（`src/server/api.rs` の ws 配信でログが送信されることを確認）
- [x] 1.3 ログ量の上限を設定し、サーバ側で古いログをトリムする（`N` 行保持が実装されていることを確認）

## 2. remote client 受信とTUI反映

- [x] 2.1 WebSocket クライアントが Log イベントをデシリアライズする（`src/remote/ws.rs` の受信処理で Log を処理することを確認）
- [x] 2.2 TUI イベントに Log を渡してログパネルへ反映する（`src/tui/orchestrator.rs` / `src/tui/state.rs` に反映経路があることを確認）

## 3. テスト

- [x] 3.1 Log イベントのシリアライズ/デシリアライズ単体テストを追加する（Log payload が round-trip することを確認）
- [x] 3.2 remote TUI が Log を受け取ってログパネルに追加するテストを追加する（state のログ件数が増えることを確認）

## Acceptance #1 Failure Follow-up

- [x] `RemoteLogEntry` に spec 必須フィールド（`project_id`, `operation`, `iteration`）を追加し、server→WebSocket→remote client のシリアライズ/デシリアライズを更新する（`src/remote/types.rs` に `project_id: Option<String>`, `operation: Option<String>`, `iteration: Option<u32>` を追加）
- [x] リモートログが change 行プレビューに紐づくよう、`change_id` を remote change の表示 ID 形式に正規化して送受信する（`src/server/runner.rs` の `make_log_entry` で `project_id` を設定、`src/tui/runner.rs` でログ変換時に `project_id` を `change_id` として使用、`src/tui/state.rs` の `get_latest_log_for_change` でプレフィックスマッチング `"<project_id>::"` を追加）

## Acceptance #2 Failure Follow-up

- [x] WebSocket の Log payload で `operation` / `iteration` キーが常に含まれるように修正する（現状は `#[serde(skip_serializing_if = "Option::is_none")]` と `make_log_entry(..., operation=None, iteration=None)` によりキー自体が欠落し、spec の「ログは少なくとも `operation` / `iteration` を含む」を満たせないため、`null` でも必ず出力する）
