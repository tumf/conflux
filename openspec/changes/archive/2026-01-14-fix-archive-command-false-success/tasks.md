## 1. 仕様・挙動の整理
- [x] 1.1 `src/tui/orchestrator.rs` と `src/parallel/executor.rs` の archive 成功判定フロー（exit code → verify → retry）を確認する
- [x] 1.2 再試行ログが試行回数付きで出力される実装になっていることを確認する

## 2. 実装
- [x] 2.1 TUI: `archive_command` 成功後に verify が失敗した場合、`archive_command` を最大 N 回まで再実行する
- [x] 2.2 TUI: 再試行の各回で、試行回数と検証失敗をログに出す
- [x] 2.3 TUI: 再試行後も未アーカイブなら、現在のエラーメッセージで失敗扱いにする
- [x] 2.4 parallel: 同様の再試行ロジックを確認し、不足があれば実装する

## 3. テスト・検証
- [x] 3.1 単体テスト: verify 失敗→再試行→成功/失敗の分岐をテストで固定する
- [x] 3.2 `cargo test` を実行する

## Future work
- 手動検証: TUI で「exit 0 だが未アーカイブ」ケースが誤エラーにならないことを確認する
- OpenSpec: `npx @fission-ai/openspec@latest validate fix-archive-command-false-success --strict` を実行する
