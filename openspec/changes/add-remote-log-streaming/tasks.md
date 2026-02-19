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
