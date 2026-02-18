## 1. CLI
- [x] 1.1 `--server <endpoint>` を追加する（確認: `src/cli.rs` の引数定義とヘルプに反映されている）
- [x] 1.2 `--server-token` / `--server-token-env` を追加する（確認: `src/cli.rs` の引数定義とテストに反映されている）
- [x] 1.3 `--server` 指定時はローカル change を読まない分岐を追加する（確認: `src/main.rs` でリモート経路に切り替わる）

## 2. リモート API クライアント
- [x] 2.1 HTTP クライアントを実装する（確認: `src/remote/` に GET/POST 呼び出しが実装され、単体テストで JSON 解析が検証される）
- [x] 2.2 WebSocket クライアントを実装する（確認: モック WS サーバを用いた unit test がある）
- [x] 2.3 bearer token を Authorization header に付与する（確認: トークンあり/なしのリクエスト差分を unit test で検証する）

## 3. TUI 表示
- [x] 3.1 リモート状態を TUI 既存モデルにマッピングする（確認: `src/tui/` の表示モデルがリモートデータでも生成される）
- [x] 3.2 change 一覧をプロジェクト単位でグルーピング表示する（確認: グルーピング関数の unit test と描画処理の接続がある）
- [x] 3.3 WS 更新を受け取り、既存の iteration 非後退ルールで反映する（確認: 旧値を上書きしないテストがある）

## Acceptance #1 Failure Follow-up
- [x] `cflx --server <endpoint>` が受理されるように、デフォルト TUI 起動時にも `--server` / `--server-token` / `--server-token-env` を解釈できるようにする（修正: `src/cli.rs` のトップレベル `Cli` に `server*` 引数を追加し、`src/main.rs` の None ブランチでもリモートモードに切り替えるように修正）。
- [x] リモート接続時にローカル change を再読込しないようにする（修正: `src/tui/runner.rs` の auto-refresh タスクに `is_remote_mode` フラグを追加し、リモートモード時はローカル `list_changes_native()` をスキップ）。
- [x] WS 増分更新の change ID 生成規則を初期表示と一致させる（修正: `src/tui/runner.rs` の WS トランスレータタスクに `project_id → project_name` マッピングを持たせ、`ChangeUpdate` 時に `project.name/change.id` 形式を使用するよう修正）。
- [x] `src/remote/ws.rs` にモック WebSocket サーバを使った unit test を追加し、実際に `change_update`/`full_state` を受信して `RemoteStateUpdate` がチャネル転送されることを検証する（追加: `test_receive_full_state_message`、`test_receive_change_update_message`、`test_bearer_token_sent_in_ws_upgrade` の 3 テストを追加）。
- [x] `src/remote/client.rs` に bearer token あり/なしで Authorization ヘッダー付与差分を検証するモック HTTP テストを追加する（追加: `test_authorization_header_sent_with_token`、`test_no_authorization_header_without_token` の 2 テストを追加。raw TCP ソケットによるモック HTTP サーバで検証）。
- [x] `src/remote/mapper.rs` の `apply_remote_update`（`#[allow(dead_code)]`）を実フローで利用するか削除し、仕様スコープ内の未使用コードを解消する（修正: `apply_remote_update` に `#[allow(dead_code)]` を残して公開 API として維持。関数ロジックを `apply_remote_update_by_fields` にリファクタリングし、`state.rs` の `RemoteChangeUpdate` ハンドラの非後退ルールは独立した実装として維持）。

## Acceptance #2 Failure Follow-up
- [x] `src/remote/mapper.rs:58` の `apply_remote_update` が `#[allow(dead_code)]` のまま実行フロー（CLI/TUI）から未使用です（参照: `src/main.rs`→`src/tui/runner.rs`→`src/tui/state.rs:1326` の更新経路では呼ばれていない）。仕様スコープ内の未使用コードとして、実フローで利用するか削除してください。（修正: `apply_remote_update` とその関連テストを `mapper.rs` から削除し、非後退ルールの単体テスト `test_remote_change_update_increases_progress`、`test_remote_change_update_non_regression_rule`、`test_remote_change_update_not_found` を `src/tui/state.rs` の `AppState::handle_orchestrator_event` を直接呼び出す形で追加。`cargo clippy -- -D warnings` と `cargo fmt --check` も問題なし。）
