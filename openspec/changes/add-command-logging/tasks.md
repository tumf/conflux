# タスク一覧：コマンド実行のログ出力追加

## 準備

- [ ] すべてのCommand::new()呼び出し箇所を洗い出す（ripgrepで検索済み）
- [ ] 既存のログ出力パターンを確認し、統一フォーマットを決定

## 実装

### Phase 1: VCS コマンドのログ追加（高優先度）

- [ ] `src/vcs/jj/mod.rs`: すべてのjjコマンド実行前にdebug!でログ出力
  - snapshot_working_copy (Line 93)
  - create new change (Line 108)
  - workspace creation (Line 211, 237)
  - workspace editing (Line 315, 346)
  - log retrieval (Line 384)
  - forget/cleanup (Line 431, 783)
  - workspace squashing (Line 662, 671, 687)
  - is_jj_available check (Line 736)

- [ ] `src/vcs/jj/commands.rs`: jj補助コマンドのログ追加
  - get_repo_root (Line 57)

- [ ] `src/parallel/executor.rs`: VCSコマンドのログ追加（すでにdebug!がある箇所は確認のみ）
  - progress commit作成 (Line 54, 76, 93, 107)
  - workspace commit (Line 581, 599, 608, 625, 647)
  - conflict resolution (Line 864, 878, 896, 918, 925, 940, 962, 984)

- [ ] `src/parallel/cleanup.rs`: cleanup時のVCSコマンドにログ追加
  - jj workspace forget (Line 98)
  - git worktree remove (Line 123)

- [ ] `src/parallel/mod.rs`: workspace初期化コマンドにログ追加
  - jj workspace add (Line 907)

### Phase 2: Agent/Hook コマンドのログ追加（中優先度）

- [ ] `src/agent.rs`: 未対応のコマンドにログ追加
  - run_apply() 非streaming版 (Line 140付近の実装確認)
  - execute_shell_command_streaming() 内部のCommand生成箇所 (Line 276, 302, 412, 427, 453, 466)

- [ ] `src/hooks.rs`: フック実行コマンドのログ追加（既にinfo!があるか確認）
  - Windows版 cmd.exe (Line 535)
  - Unix版 /bin/sh (Line 541)

### Phase 3: その他のコマンドログ追加（低優先度）

- [ ] `src/cli.rs`: CLIツール存在確認コマンドにログ追加
  - jj version check (Line 196)
  - git version check (Line 210)

- [ ] `src/tui/runner.rs`: TUI内でのコマンド実行にログ追加
  - shell command (Line 554)

### Phase 4: テストとドキュメント

- [ ] 既存のテストを実行し、すべて通過することを確認
  - `cargo test`
  - 特に `tests/e2e_tests.rs` での動作確認

- [ ] ログ出力の確認
  - `RUST_LOG=debug cargo run -- run --dry-run` でdebugレベルのログが出ることを確認
  - `RUST_LOG=info cargo run -- run --dry-run` でinfoレベルのみ出ることを確認

- [ ] コードフォーマットとlint
  - `cargo fmt`
  - `cargo clippy -- -D warnings`

- [ ] AGENTS.md の「Logging」セクションに追加情報を記載（必要に応じて）

## 検証

- [ ] 実際のopenspec changeをrun/TUIモードで実行し、ログが適切に出力されることを確認
- [ ] 並列実行モード（jj workspace使用）でVCSコマンドのログが出力されることを確認
- [ ] エラー発生時のトラブルシューティングが容易になったことを確認

## 完了条件

- すべてのCommand::new()呼び出し箇所でログ出力が追加されている
- ログレベルが適切に設定されている（user-facing: info, internal: debug）
- `cargo test` がすべて通過する
- `cargo clippy -- -D warnings` でwarningが出ない
- 実際の実行でログが期待通り出力される
