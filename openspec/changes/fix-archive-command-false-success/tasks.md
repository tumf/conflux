## 1. 仕様・挙動の整理
- [ ] 1.1 現状の TUI/parallel の archive 成功判定フロー（exit code と verify の関係）を確認する
- [ ] 1.2 「exit 0 だが未アーカイブ」ケースの再現条件（ログ例）を整理する

## 2. 実装（予定）
- [ ] 2.1 TUI: `archive_command` 成功後に verify が失敗した場合、`archive_command` を最大 N 回まで再実行する
- [ ] 2.2 TUI: 再試行の各回で、試行回数と検証結果をログに出す
- [ ] 2.3 TUI: 再試行後も未アーカイブなら、現在のエラーメッセージで失敗扱いにする
- [ ] 2.4 parallel: 同様の再試行方針を適用するか判断し、適用する場合は同等の実装を行う

## 3. テスト・検証
- [ ] 3.1 単体テスト: verify 失敗→再試行→成功/失敗の分岐をテストで固定する
- [ ] 3.2 手動検証: TUI で「exit 0 だが未アーカイブ」ケースが誤エラーにならないことを確認する
- [ ] 3.3 `cargo test` を実行する

## 4. OpenSpec
- [ ] 4.1 `npx @fission-ai/openspec@latest validate fix-archive-command-false-success --strict` を通す
