## 1. 設定と CLI の整理

- [ ] 1.1 `server.resolve_command` を受け付けないバリデーションを追加する（`src/config/mod.rs` の設定検証で `server.resolve_command` がエラーになることを確認）
- [ ] 1.2 `cflx server --resolve-command` を廃止する（`src/cli.rs` のサーバ引数から削除され、実行時に不明オプションエラーになることを確認）

## 2. サーバの resolve_command 参照先変更

- [ ] 2.1 サーバの AppState にトップレベル `resolve_command` を渡す（`src/server/mod.rs` で設定の `resolve_command` が AppState に入ることを確認）
- [ ] 2.2 auto_resolve の実行でトップレベル `resolve_command` を使用する（`src/server/api.rs` で `state.resolve_command` がトップレベル由来になることを確認）

## 3. テスト更新

- [ ] 3.1 `server.resolve_command` が設定エラーになるテストを追加する（`src/config/mod.rs` の設定テストでエラー文言を確認）
- [ ] 3.2 `auto_resolve` がトップレベル `resolve_command` を使うことを確認するテストを追加する（`src/server/api.rs` のテストで `resolve_command_ran=true` を確認）
