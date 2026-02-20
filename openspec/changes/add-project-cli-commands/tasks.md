## 1. CLI サブコマンド定義

- [ ] 1.1 `src/cli.rs` に `project` サブコマンド（add/remove/status/sync）と引数構造体を追加する（確認: `src/cli.rs` に `Commands::Project` と `ProjectCommands` が定義されている）
- [ ] 1.2 `--json` フラグを `project` 配下に追加する（確認: `src/cli.rs` の clap 定義で `--json` が使用可能）
- [ ] 1.3 clap 解析テストを追加する（確認: `src/cli.rs` のテストで `cflx project add/remove/status/sync` がパースできる）

## 2. 接続先解決と認証非対応ガード

- [ ] 2.1 `--server` 未指定時にグローバル設定の `server.bind`/`server.port` を解決する関数を追加する（確認: `src/main.rs` もしくは新規ヘルパーで URL 生成ロジックが存在）
- [ ] 2.2 `--server-token`/`--server-token-env` が指定された場合は即時エラーにする（確認: 送信前にエラーを返す分岐がある）
- [ ] 2.3 グローバル設定で `server.auth.mode=bearer_token` の場合も project 実行を拒否する（確認: 設定チェックが実装され、未対応メッセージが出る）
- [ ] 2.4 上記の URL 解決・認証ガードの単体テストを追加する（確認: 新規テストが期待値を検証する）

## 3. サーバ API クライアント実装（認証なし）

- [ ] 3.1 `src/remote/client.rs` に `list_projects/add_project/delete_project/git_sync` を追加する（確認: それぞれが `/api/v1/projects` 系を呼び出す）
- [ ] 3.2 `src/remote/test_helpers.rs` に任意レスポンスを返し、リクエスト内容を検証できるモック HTTP サーバを追加する（確認: テストで method/path/body を検証できる）
- [ ] 3.3 認証ヘッダを送らないことのテストを追加する（確認: モックで `Authorization` ヘッダが存在しないことを検証する）

## 4. コマンド実行と出力

- [ ] 4.1 `src/main.rs` に `project` サブコマンドの実行分岐を追加し、API 呼び出しを行う（確認: 各サブコマンドが対応するクライアントメソッドを呼ぶ）
- [ ] 4.2 人間向け出力と `--json` 出力を実装する（確認: JSON はサーバ応答をそのまま出力し、human は要約表示）
- [ ] 4.3 失敗時のエラー整形（401/404/422/接続失敗）を実装する（確認: エラー内容が明示され、exit code が非 0 になる）
