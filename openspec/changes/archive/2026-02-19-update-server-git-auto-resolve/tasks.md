## 1. 実装
- [x] 1.1 `git/pull` と `git/push` のリクエストに `auto_resolve` と `resolve_strategy` を解釈する処理を追加する（`src/server/api.rs` にパラメータ解析が追加されることを確認）
- [x] 1.2 non-fast-forward 検知時に `auto_resolve=true` なら resolve_command を実行し、成功時のみ処理を継続する（`src/server/api.rs` の分岐と `run_resolve_command` ヘルパーの呼び出しが追加されることを確認）
- [x] 1.3 auto_resolve 実行結果をレスポンスに含める（`resolve_command_ran`、`resolve_exit_code` など、API 応答の JSON に追加されることを確認）

## 2. テスト
- [x] 2.1 ローカルの bare repo を使って non-fast-forward を再現するテストを追加する（`src/server/api.rs` のテストで 422 とエラー内容が確認できること）
- [x] 2.2 `auto_resolve=true` で resolve_command が実行されることを確認するテストを追加する（resolve_command を `echo resolve` などのスタブに置き換え、レスポンスの `resolve_command_ran` が true になること）

## Acceptance #1 Failure Follow-up
- [x] `git status --porcelain` が空になるように未コミット変更を解消する（現在の差分: `openspec/changes/update-server-git-auto-resolve/tasks.md`, `src/server/api.rs`, `src/server/mod.rs`）。
- [x] `cflx server` 実行時に `resolve_command` を実際に注入できるように設定経路を実装する（`src/server/mod.rs` の `run_server` で `AppState.resolve_command` が `None` 固定のため、`src/server/api.rs` の `git_pull`/`git_push` 内 `state.resolve_command` 分岐が実運用フローで到達不能）。
  - `src/config/mod.rs`: `ServerConfig` に `resolve_command: Option<String>` フィールドを追加
  - `src/config/mod.rs`: `ServerConfig::apply_cli_overrides()` に `resolve_command` パラメータを追加
  - `src/server/mod.rs`: `run_server()` で `config.resolve_command` を `AppState.resolve_command` に注入
  - `src/cli.rs`: `ServerArgs` に `--resolve-command` CLI オプションを追加
  - `src/main.rs`: CLI 引数を `apply_cli_overrides()` 経由で `ServerConfig` に渡す
