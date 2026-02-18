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
