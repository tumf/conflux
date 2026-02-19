## 1. 実装
- [ ] 1.1 `git/pull` と `git/push` のリクエストに `auto_resolve` と `resolve_strategy` を解釈する処理を追加する（`src/server/api.rs` にパラメータ解析が追加されることを確認）
- [ ] 1.2 non-fast-forward 検知時に `auto_resolve=true` なら resolve_command を実行し、成功時のみ処理を継続する（`src/server/api.rs` の分岐と `src/agent/runner.rs` の呼び出しが追加されることを確認）
- [ ] 1.3 auto_resolve 実行結果をレスポンスに含める（`resolve_command_ran`、`resolve_exit_code` など、API 応答の JSON に追加されることを確認）

## 2. テスト
- [ ] 2.1 ローカルの bare repo を使って non-fast-forward を再現するテストを追加する（`src/server/api.rs` のテストで 422 とエラー内容が確認できること）
- [ ] 2.2 `auto_resolve=true` で resolve_command が実行されることを確認するテストを追加する（resolve_command を `echo resolve` などのスタブに置き換え、レスポンスの `resolve_command_ran` が true になること）
